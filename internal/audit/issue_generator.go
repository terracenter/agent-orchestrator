package audit

import (
	"fmt"
	"strings"
	"time"
)

// IssueDraftInput contiene los datos auditados para generar un issue revisable.
type IssueDraftInput struct {
	Title              string
	Repository         string
	Report             SessionAuditReport
	ExpectedBehavior   string
	ActualBehavior     string
	Evidence           []string
	AcceptanceCriteria []string
}

// IssueDraft es un borrador de issue. No crea issues remotos por diseño.
type IssueDraft struct {
	Title               string    `json:"title"`
	Body                string    `json:"body"`
	RequiresHumanReview bool      `json:"requires_human_review"`
	GuardrailRelated    bool      `json:"guardrail_related"`
	GeneratedAt         time.Time `json:"generated_at"`
}

// GenerateIssueDraftFromSessionAudit convierte findings de auditoría en un issue verificable.
func GenerateIssueDraftFromSessionAudit(input IssueDraftInput) IssueDraft {
	title := strings.TrimSpace(input.Title)
	if title == "" {
		title = "Corregir findings de auditoría de sesión"
		if input.Report.SessionID != "" {
			title = fmt.Sprintf("Corregir findings de auditoría de sesión %s", input.Report.SessionID)
		}
	}

	expected := strings.TrimSpace(input.ExpectedBehavior)
	if expected == "" {
		expected = "La sesión debe cumplir las políticas de Orq: usar rtk cuando sea obligatorio, delegar ejecución cara/supervisora y confirmar o simular mutaciones destructivas."
	}
	actual := strings.TrimSpace(input.ActualBehavior)
	if actual == "" {
		actual = fmt.Sprintf("La auditoría devolvió status=%s con %d finding(s).", input.Report.Status, len(input.Report.Findings))
	}

	criteria := input.AcceptanceCriteria
	if len(criteria) == 0 {
		criteria = []string{
			"Dado un reporte de auditoría con los mismos eventos, cuando se reejecute la auditoría, entonces no debe aparecer el finding corregido.",
			"Dado que el cambio modifica guardrails o política de ejecución, cuando exista un PR, entonces debe requerir revisión humana antes de merge.",
		}
	}

	guardrail := false
	var body strings.Builder
	body.WriteString("## Comportamiento esperado\n\n")
	body.WriteString(expected + "\n\n")
	body.WriteString("## Comportamiento actual\n\n")
	body.WriteString(actual + "\n\n")
	body.WriteString("## Evidencia\n\n")
	body.WriteString(fmt.Sprintf("- session_id: `%s`\n", input.Report.SessionID))
	body.WriteString(fmt.Sprintf("- status: `%s`\n", input.Report.Status))
	body.WriteString(fmt.Sprintf("- total_events: `%d`\n", input.Report.TotalEvents))
	for _, ev := range input.Evidence {
		body.WriteString("- " + strings.TrimSpace(ev) + "\n")
	}
	for _, finding := range input.Report.Findings {
		if isGuardrailFinding(finding.Code) {
			guardrail = true
		}
		body.WriteString(fmt.Sprintf("- `%s` severity=`%s`: %s\n", finding.Code, finding.Severity, finding.Message))
		if finding.Target != "" {
			body.WriteString(fmt.Sprintf("  - target: `%s`\n", finding.Target))
		}
		if finding.Remediation != "" {
			body.WriteString(fmt.Sprintf("  - remediation: %s\n", finding.Remediation))
		}
	}
	body.WriteString("\n## Criterios de aceptación\n\n")
	for _, criterion := range criteria {
		body.WriteString("- " + strings.TrimSpace(criterion) + "\n")
	}
	body.WriteString("\n## Revisión humana\n\n")
	body.WriteString("- [ ] Revisado por una persona antes de aplicar o mergear cambios de guardrails.\n")

	return IssueDraft{Title: title, Body: body.String(), RequiresHumanReview: true, GuardrailRelated: guardrail, GeneratedAt: time.Now().UTC()}
}

func isGuardrailFinding(code string) bool {
	switch code {
	case CodeRTKRequired, CodeExpensiveAgentExecution, CodeUnconfirmedMutation:
		return true
	default:
		return false
	}
}
