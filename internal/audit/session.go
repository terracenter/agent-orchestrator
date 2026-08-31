package audit

import (
	"bufio"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/terracenter/agent-orchestrator/internal/trace"
)

// Códigos estables para findings de auditoría de sesión.
const (
	CodeRTKRequired             = "AUDIT_RTK_REQUIRED"
	CodeExpensiveAgentExecution = "AUDIT_EXPENSIVE_AGENT_EXECUTION"
	CodeUnconfirmedMutation     = "AUDIT_UNCONFIRMED_MUTATION"
	CodeSessionNotFound         = "AUDIT_SESSION_NOT_FOUND"
	CodeSessionParseError       = "AUDIT_SESSION_PARSE_ERROR"
)

// Severidades estándar de findings.
const (
	SeverityBlocker  = "blocker"
	SeverityCritical = "critical"
	SeverityError    = "error"
	SeverityWarning  = "warning"
	SeverityInfo     = "info"
)

// SessionFinding representa un hallazgo individual durante la auditoría.
type SessionFinding struct {
	Code        string `json:"code"`
	Severity    string `json:"severity"` // blocker, critical, error, warning, info
	Message     string `json:"message"`
	Target      string `json:"target,omitempty"`      // comando, archivo, modelo o agente auditado
	Remediation string `json:"remediation,omitempty"` // recomendación para resolver el finding
}

// SessionAuditReport es el reporte generado al auditar una sesión.
type SessionAuditReport struct {
	SessionID   string           `json:"session_id,omitempty"`
	Agent       string           `json:"agent,omitempty"`
	Model       string           `json:"model,omitempty"`
	Host        string           `json:"host,omitempty"`
	Status      string           `json:"status"` // PASSED, WARNING, BLOCKED, FAILED
	TotalEvents int              `json:"total_events"`
	AuditedAt   time.Time        `json:"audited_at"`
	Findings    []SessionFinding `json:"findings"`
	Summary     string           `json:"summary"`
}

// SessionAuditOptions contiene parámetros para configurar la auditoría de sesión.
type SessionAuditOptions struct {
	SessionID       string
	TraceDir        string
	FilePath        string
	RTKRequired     *bool
	ExpensiveAgents []string
	RequireDryRun   *bool
}

// Herramientas comunes que requieren wrapper rtk en el workspace de orq.
var rtkProtectedBinaries = []string{
	"git", "curl", "ls", "grep", "rg", "find", "cat", "head", "tail",
	"sed", "awk", "go", "npm", "cargo", "docker", "act", "gh",
	"tree", "wc", "diff", "ps", "df", "du", "tar", "zip", "pytest",
}

// Comandos o patrones de mutación destructiva de alto riesgo.
var destructivePatterns = []string{
	"push --force",
	"push -f",
	"reset --hard",
	"clean -f",
	"branch -D",
	"rm -rf",
	"rm -r ",
	"drop table",
	"drop database",
	"delete from",
	"truncate table",
	"truncate ",
	"system prune -a",
	"volume prune",
	"mkfs",
	"dd if=",
}

// Agentes de alto costo o perfil de supervisión por defecto.
var defaultExpensiveAgents = []string{
	"pi",
	"claude-opus",
	"claude-sonnet",
	"claude-code/sonnet",
	"claude-code/opus",
	"openai/gpt-5.5",
	"gpt-5.5",
}

// AuditSession audita una sesión de trace y sus eventos en memoria.
func AuditSession(session *trace.TraceSession, events []trace.TraceEvent, opts SessionAuditOptions) SessionAuditReport {
	now := time.Now().UTC()
	report := SessionAuditReport{
		AuditedAt:   now,
		Findings:    []SessionFinding{},
		TotalEvents: len(events),
	}

	if session != nil {
		report.SessionID = session.ID
		report.Agent = session.Agent
		report.Model = session.Model
		report.Host = session.Host
	} else if opts.SessionID != "" {
		report.SessionID = opts.SessionID
	}

	// 1. Determinar si rtk_required aplica (por flag, por metadata de sesión, o default true)
	rtkRequired := true
	if opts.RTKRequired != nil {
		rtkRequired = *opts.RTKRequired
	} else if session != nil && session.Metadata != nil {
		if val, exists := session.Metadata["rtk_required"]; exists {
			rtkRequired = strings.EqualFold(val, "true") || val == "1"
		}
	}

	// 2. Determinar si el agente es considerado caro / supervisor que debió delegar
	expensiveAgents := defaultExpensiveAgents
	if len(opts.ExpensiveAgents) > 0 {
		expensiveAgents = opts.ExpensiveAgents
	}

	isExpensiveAgent := false
	if session != nil {
		agentIdentifier := strings.ToLower(strings.TrimSpace(session.Agent))
		modelIdentifier := strings.ToLower(strings.TrimSpace(session.Model))
		fullIdentifier := agentIdentifier
		if modelIdentifier != "" {
			fullIdentifier = agentIdentifier + "/" + modelIdentifier
		}

		for _, exp := range expensiveAgents {
			expLower := strings.ToLower(strings.TrimSpace(exp))
			if agentIdentifier == expLower || strings.Contains(fullIdentifier, expLower) || (modelIdentifier != "" && strings.Contains(modelIdentifier, expLower)) {
				isExpensiveAgent = true
				break
			}
		}

		if session.Metadata != nil {
			if strings.EqualFold(session.Metadata["supervisor_only"], "true") ||
				strings.EqualFold(session.Metadata["must_stop_for_delegation"], "true") ||
				strings.EqualFold(session.Metadata["role"], "supervisor") {
				isExpensiveAgent = true
			}
		}
	}

	// Evaluamos eventos
	commandCount := 0
	technicalExecutionDetected := false

	for _, ev := range events {
		cmdStr := strings.TrimSpace(ev.Command)

		if ev.EventType == trace.EventTypeCommand || cmdStr != "" {
			commandCount++

			// Check 1: Detectar comandos sin rtk cuando rtk_required=true
			if rtkRequired && cmdStr != "" {
				if isProtectedWithoutRTK(cmdStr) {
					report.Findings = append(report.Findings, SessionFinding{
						Code:        CodeRTKRequired,
						Severity:    SeverityBlocker,
						Message:     fmt.Sprintf("comando ejecutado sin el wrapper rtk obligatorio: %q", cmdStr),
						Target:      cmdStr,
						Remediation: "anteponer el wrapper rtk al comando (ej. 'rtk " + cmdStr + "')",
					})
				}
			}

			// Check 3: Detectar mutaciones destructivas sin dry-run ni confirmación
			if isDestructiveCommand(cmdStr) {
				hasDryRun := strings.Contains(cmdStr, "--dry-run") || strings.Contains(cmdStr, "--dryrun")
				hasApproval := false
				if ev.Details != nil {
					if strings.EqualFold(ev.Details["human_approval"], "true") ||
						strings.EqualFold(ev.Details["dry_run"], "true") ||
						strings.EqualFold(ev.Details["confirmed"], "true") {
						hasApproval = true
					}
				}
				if session != nil && session.Metadata != nil {
					if strings.EqualFold(session.Metadata["human_approval"], "true") {
						hasApproval = true
					}
				}

				if !hasDryRun && !hasApproval {
					report.Findings = append(report.Findings, SessionFinding{
						Code:        CodeUnconfirmedMutation,
						Severity:    SeverityBlocker,
						Message:     fmt.Sprintf("mutación destructiva ejecutada sin flag --dry-run ni confirmación humana: %q", cmdStr),
						Target:      cmdStr,
						Remediation: "ejecutar primero con simulación --dry-run o solicitar autorización humana explícita",
					})
				}
			}

			technicalExecutionDetected = true
		}

		// Validar mutaciones directas de archivos (delete sin aprobación)
		if ev.EventType == trace.EventTypeFile && ev.FileOperation == trace.FileOpDelete {
			hasApproval := false
			if ev.Details != nil && (strings.EqualFold(ev.Details["human_approval"], "true") || strings.EqualFold(ev.Details["confirmed"], "true")) {
				hasApproval = true
			}
			if session != nil && session.Metadata != nil && strings.EqualFold(session.Metadata["human_approval"], "true") {
				hasApproval = true
			}
			if !hasApproval {
				report.Findings = append(report.Findings, SessionFinding{
					Code:        CodeUnconfirmedMutation,
					Severity:    SeverityBlocker,
					Message:     fmt.Sprintf("eliminación de archivo sin confirmación humana previa: %s", ev.FilePath),
					Target:      ev.FilePath,
					Remediation: "solicitar confirmación humana antes de eliminar archivos en la sesión",
				})
			}
		}
	}

	// Check 2: Detectar ejecución en agente caro cuando la política exigía delegación
	if isExpensiveAgent && (commandCount > 0 || technicalExecutionDetected) {
		report.Findings = append(report.Findings, SessionFinding{
			Code:        CodeExpensiveAgentExecution,
			Severity:    SeverityBlocker,
			Message:     fmt.Sprintf("agente supervisor/caro (%s) ejecutó %d comandos técnicos directamente en lugar de delegar a runner de bajo costo", report.Agent, commandCount),
			Target:      report.Agent,
			Remediation: "utilizar 'orq delegate' para traspasar la ejecución a agy, hermes, codex o modelo local y detener el supervisor",
		})
	}

	// Calcular estado global del reporte
	hasBlocker := false
	hasError := false
	hasWarning := false

	for _, f := range report.Findings {
		switch f.Severity {
		case SeverityBlocker, SeverityCritical:
			hasBlocker = true
		case SeverityError:
			hasError = true
		case SeverityWarning:
			hasWarning = true
		}
	}

	if hasBlocker {
		report.Status = "BLOCKED"
	} else if hasError {
		report.Status = "FAILED"
	} else if hasWarning {
		report.Status = "WARNING"
	} else {
		report.Status = "PASSED"
	}

	report.Summary = fmt.Sprintf("status=%s events=%d findings=%d", report.Status, report.TotalEvents, len(report.Findings))
	return report
}

// AuditSessionByID busca una sesión en el trace manager por su ID y la audita.
func AuditSessionByID(stateDir string, sessionID string, opts SessionAuditOptions) (SessionAuditReport, error) {
	mgr := trace.NewManager(stateDir)
	session, events, err := mgr.Status(sessionID)
	if err != nil {
		return SessionAuditReport{
			SessionID: sessionID,
			Status:    "FAILED",
			Findings: []SessionFinding{
				{
					Code:        CodeSessionNotFound,
					Severity:    SeverityBlocker,
					Message:     fmt.Sprintf("no se pudo cargar la sesión de trace %q: %v", sessionID, err),
					Target:      sessionID,
					Remediation: "verificar que el session_id exista en el directorio de traces de orq",
				},
			},
			Summary: fmt.Sprintf("status=FAILED session_id=%s not found", sessionID),
		}, err
	}

	opts.SessionID = sessionID
	return AuditSession(session, events, opts), nil
}

// AuditSessionFile lee un archivo de sesión (.json) o archivo JSONL de eventos y genera la auditoría.
func AuditSessionFile(filePath string, opts SessionAuditOptions) (SessionAuditReport, error) {
	data, err := os.ReadFile(filePath)
	if err != nil {
		return SessionAuditReport{}, fmt.Errorf("leer archivo de sesión %s: %w", filePath, err)
	}

	// Intentar deserializar como TraceSession primero
	var session trace.TraceSession
	if err := json.Unmarshal(data, &session); err == nil && session.ID != "" {
		// Buscar archivo de eventos complementario con el mismo base name
		dir := filepath.Dir(filePath)
		base := strings.TrimSuffix(filepath.Base(filePath), ".session.json")
		eventsPath := filepath.Join(dir, base+".jsonl")
		events, _ := readEventsFromFile(eventsPath)
		return AuditSession(&session, events, opts), nil
	}

	// Intentar deserializar como lista de eventos o líneas JSONL
	events, err := readEventsFromFile(filePath)
	if err == nil && len(events) > 0 {
		return AuditSession(nil, events, opts), nil
	}

	// Si es un objeto de reporte directo o estructura simple
	return SessionAuditReport{
		Status: "FAILED",
		Findings: []SessionFinding{
			{
				Code:        CodeSessionParseError,
				Severity:    SeverityBlocker,
				Message:     fmt.Sprintf("el archivo %s no contiene una sesión o eventos de trace válidos", filePath),
				Target:      filePath,
				Remediation: "proporcionar un archivo .session.json o .jsonl generado por orq trace",
			},
		},
		Summary: fmt.Sprintf("status=FAILED invalid file %s", filePath),
	}, fmt.Errorf("formato no reconocido en %s", filePath)
}

// AuditLatestSession busca la sesión más reciente en el traceDir y la audita.
func AuditLatestSession(stateDir string, opts SessionAuditOptions) (SessionAuditReport, error) {
	mgr := trace.NewManager(stateDir)
	sessions, err := mgr.List()
	if err != nil {
		return SessionAuditReport{}, fmt.Errorf("listar sesiones: %w", err)
	}
	if len(sessions) == 0 {
		return SessionAuditReport{
			Status:  "PASSED",
			Summary: "status=PASSED no sessions found in trace store",
		}, nil
	}

	// Obtener la sesión más reciente
	var latest trace.TraceSession
	for _, s := range sessions {
		if latest.ID == "" || s.StartedAt.After(latest.StartedAt) {
			latest = s
		}
	}

	return AuditSessionByID(stateDir, latest.ID, opts)
}

// isProtectedWithoutRTK verifica si un comando invoca binarios protegidos sin el prefijo rtk.
func isProtectedWithoutRTK(cmd string) bool {
	trimmed := strings.TrimSpace(cmd)
	if trimmed == "" {
		return false
	}

	// Wrappers válidos autorizados
	if strings.HasPrefix(trimmed, "rtk ") || strings.HasPrefix(trimmed, "rtk\t") || trimmed == "rtk" {
		return false
	}
	if strings.HasPrefix(trimmed, "vg ") || strings.HasPrefix(trimmed, "vg\t") || trimmed == "vg" {
		return false
	}
	if strings.HasPrefix(trimmed, "orq ") || strings.HasPrefix(trimmed, "orq\t") || strings.HasPrefix(trimmed, "./orq ") || strings.HasPrefix(trimmed, "./orq\t") {
		return false
	}

	fields := strings.Fields(trimmed)
	if len(fields) == 0 {
		return false
	}

	firstWord := filepath.Base(fields[0])
	for _, protected := range rtkProtectedBinaries {
		if firstWord == protected {
			return true
		}
	}

	return false
}

// isDestructiveCommand detecta si un comando contiene patrones destructivos de alto riesgo.
func isDestructiveCommand(cmd string) bool {
	cmdLower := strings.ToLower(cmd)
	for _, pat := range destructivePatterns {
		if strings.Contains(cmdLower, pat) {
			return true
		}
	}
	return false
}

func readEventsFromFile(path string) ([]trace.TraceEvent, error) {
	f, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer f.Close()

	var events []trace.TraceEvent
	scanner := bufio.NewScanner(f)
	for scanner.Scan() {
		line := strings.TrimSpace(scanner.Text())
		if line == "" {
			continue
		}
		var ev trace.TraceEvent
		if err := json.Unmarshal([]byte(line), &ev); err != nil {
			continue
		}
		events = append(events, ev)
	}
	return events, scanner.Err()
}
