package audit

import "testing"

func TestGenerateIssueDraftFromSessionAuditIncludesRequiredSections(t *testing.T) {
	report := SessionAuditReport{
		SessionID:   "sess-1",
		Status:      "BLOCKED",
		TotalEvents: 2,
		Findings: []SessionFinding{
			{Code: CodeRTKRequired, Severity: SeverityBlocker, Message: "comando sin rtk", Target: "git status", Remediation: "usar rtk"},
		},
	}

	draft := GenerateIssueDraftFromSessionAudit(IssueDraftInput{Report: report, Evidence: []string{"validacion: orq audit session --session-id sess-1"}})

	if draft.Title == "" {
		t.Fatal("expected title")
	}
	if !draft.RequiresHumanReview {
		t.Fatal("expected human review requirement")
	}
	if !draft.GuardrailRelated {
		t.Fatal("expected guardrail related draft")
	}
	for _, want := range []string{"## Comportamiento esperado", "## Comportamiento actual", "## Evidencia", "AUDIT_RTK_REQUIRED", "## Criterios de aceptación", "Revisado por una persona"} {
		if !contains(draft.Body, want) {
			t.Fatalf("expected body to contain %q; body=%s", want, draft.Body)
		}
	}
}

func TestGenerateIssueDraftFromSessionAuditAcceptsOverrides(t *testing.T) {
	draft := GenerateIssueDraftFromSessionAudit(IssueDraftInput{
		Title:              "Titulo custom",
		ExpectedBehavior:   "esperado custom",
		ActualBehavior:     "actual custom",
		AcceptanceCriteria: []string{"criterio custom"},
		Report:             SessionAuditReport{Status: "FAILED"},
	})

	if draft.Title != "Titulo custom" {
		t.Fatalf("unexpected title: %s", draft.Title)
	}
	for _, want := range []string{"esperado custom", "actual custom", "criterio custom"} {
		if !contains(draft.Body, want) {
			t.Fatalf("expected body to contain %q; body=%s", want, draft.Body)
		}
	}
}

func contains(s, substr string) bool {
	return len(substr) == 0 || (len(s) >= len(substr) && index(s, substr) >= 0)
}

func index(s, substr string) int {
	for i := 0; i+len(substr) <= len(s); i++ {
		if s[i:i+len(substr)] == substr {
			return i
		}
	}
	return -1
}
