package audit

import (
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/terracenter/agent-orchestrator/internal/trace"
)

func TestAuditSession_RTKMissingDetection(t *testing.T) {
	session := &trace.TraceSession{
		ID:        "sess-rtk-test",
		Agent:     "agy",
		Model:     "gemini-3.5-flash-low",
		Host:      "hermes-contabo",
		StartedAt: time.Now().UTC(),
		Metadata: map[string]string{
			"rtk_required": "true",
		},
	}

	events := []trace.TraceEvent{
		{
			SessionID: "sess-rtk-test",
			EventType: trace.EventTypeCommand,
			Command:   "git status", // Sin rtk -> debe fallar
		},
		{
			SessionID: "sess-rtk-test",
			EventType: trace.EventTypeCommand,
			Command:   "rtk git status", // Con rtk -> válido
		},
		{
			SessionID: "sess-rtk-test",
			EventType: trace.EventTypeCommand,
			Command:   "ls -la", // Sin rtk -> debe fallar
		},
		{
			SessionID: "sess-rtk-test",
			EventType: trace.EventTypeCommand,
			Command:   "vg query", // vg es excepción autorizada -> válido
		},
		{
			SessionID: "sess-rtk-test",
			EventType: trace.EventTypeCommand,
			Command:   "orq status", // orq es excepción autorizada -> válido
		},
	}

	report := AuditSession(session, events, SessionAuditOptions{})

	if report.Status != "FAILED" && report.Status != "BLOCKED" {
		t.Fatalf("expected status FAILED or BLOCKED, got %s", report.Status)
	}

	rtkMissingCount := 0
	for _, f := range report.Findings {
		if f.Code == CodeRTKRequired {
			rtkMissingCount++
			if f.Severity != SeverityBlocker {
				t.Errorf("expected severity %s for CodeRTKRequired, got %s", SeverityBlocker, f.Severity)
			}
		}
	}

	if rtkMissingCount != 2 {
		t.Errorf("expected 2 RTKMissing findings (git status and ls -la), got %d", rtkMissingCount)
	}
}

func TestAuditSession_ExpensiveAgentExecutionDetection(t *testing.T) {
	session := &trace.TraceSession{
		ID:        "sess-expensive-test",
		Agent:     "pi",
		Model:     "gpt-5.5",
		Host:      "pi",
		StartedAt: time.Now().UTC(),
		Metadata: map[string]string{
			"supervisor_only": "true",
			"rtk_required":    "true",
		},
	}

	events := []trace.TraceEvent{
		{
			SessionID: "sess-expensive-test",
			EventType: trace.EventTypeCommand,
			Command:   "rtk go test ./...",
		},
	}

	report := AuditSession(session, events, SessionAuditOptions{})

	if report.Status != "FAILED" && report.Status != "BLOCKED" {
		t.Fatalf("expected status FAILED or BLOCKED, got %s", report.Status)
	}

	foundExpensiveFinding := false
	for _, f := range report.Findings {
		if f.Code == CodeExpensiveAgentExecution {
			foundExpensiveFinding = true
			if f.Severity != SeverityBlocker {
				t.Errorf("expected severity %s for CodeExpensiveAgentExecution, got %s", SeverityBlocker, f.Severity)
			}
		}
	}

	if !foundExpensiveFinding {
		t.Errorf("expected finding with code %s", CodeExpensiveAgentExecution)
	}
}

func TestAuditSession_DestructiveMutationWithoutDryRun(t *testing.T) {
	session := &trace.TraceSession{
		ID:        "sess-destructive-test",
		Agent:     "agy",
		Model:     "gemini-3.5-flash-low",
		Host:      "hermes-contabo",
		StartedAt: time.Now().UTC(),
	}

	events := []trace.TraceEvent{
		{
			SessionID: "sess-destructive-test",
			EventType: trace.EventTypeCommand,
			Command:   "rtk git push --force origin main", // destructivo sin dry-run ni aprobación
		},
		{
			SessionID: "sess-destructive-test",
			EventType: trace.EventTypeCommand,
			Command:   "rtk git clean -f --dry-run", // destructivo con dry-run -> permitido
		},
		{
			SessionID:     "sess-destructive-test",
			EventType:     trace.EventTypeFile,
			FilePath:      "/tmp/critical.db",
			FileOperation: trace.FileOpDelete, // delete sin human_approval -> bloqueado
		},
	}

	report := AuditSession(session, events, SessionAuditOptions{})

	if report.Status != "BLOCKED" {
		t.Fatalf("expected status BLOCKED, got %s", report.Status)
	}

	mutationFindings := 0
	for _, f := range report.Findings {
		if f.Code == CodeUnconfirmedMutation {
			mutationFindings++
			if f.Severity != SeverityBlocker {
				t.Errorf("expected severity %s for CodeUnconfirmedMutation, got %s", SeverityBlocker, f.Severity)
			}
		}
	}

	if mutationFindings != 2 {
		t.Errorf("expected 2 CodeUnconfirmedMutation findings, got %d", mutationFindings)
	}
}

func TestAuditSession_PassedCleanSession(t *testing.T) {
	session := &trace.TraceSession{
		ID:        "sess-clean-test",
		Agent:     "agy",
		Model:     "gemini-3.5-flash-low",
		Host:      "hermes-contabo",
		StartedAt: time.Now().UTC(),
		Metadata: map[string]string{
			"rtk_required": "true",
		},
	}

	events := []trace.TraceEvent{
		{
			SessionID: "sess-clean-test",
			EventType: trace.EventTypeCommand,
			Command:   "rtk git status",
		},
		{
			SessionID: "sess-clean-test",
			EventType: trace.EventTypeCommand,
			Command:   "rtk go test ./...",
		},
		{
			SessionID: "sess-clean-test",
			EventType: trace.EventTypeCommand,
			Command:   "vg sync",
		},
	}

	report := AuditSession(session, events, SessionAuditOptions{})

	if report.Status != "PASSED" {
		t.Fatalf("expected status PASSED, got %s (findings: %v)", report.Status, report.Findings)
	}
	if len(report.Findings) != 0 {
		t.Errorf("expected 0 findings, got %d", len(report.Findings))
	}
}

func TestAuditSession_TraceManagerIntegration(t *testing.T) {
	tempDir := t.TempDir()
	mgr := trace.NewManager(tempDir)

	sess, err := mgr.Start(trace.TraceMetadata{
		Agent:     "agy",
		Host:      "hermes-contabo",
		Workspace: "/workspace",
		Model:     "gemini-3.5-flash-low",
		Metadata: map[string]string{
			"rtk_required": "true",
		},
	})
	if err != nil {
		t.Fatalf("start trace: %v", err)
	}

	err = mgr.Record(sess.ID, trace.TraceEvent{
		EventType: trace.EventTypeCommand,
		Command:   "cat README.md", // sin rtk
	})
	if err != nil {
		t.Fatalf("record trace: %v", err)
	}

	report, err := AuditSessionByID(tempDir, sess.ID, SessionAuditOptions{})
	if err != nil {
		t.Fatalf("audit session by id: %v", err)
	}

	if report.Status != "BLOCKED" {
		t.Errorf("expected status BLOCKED, got %s", report.Status)
	}
	if len(report.Findings) != 1 {
		t.Fatalf("expected 1 finding, got %d", len(report.Findings))
	}
	if report.Findings[0].Code != CodeRTKRequired {
		t.Errorf("expected code %s, got %s", CodeRTKRequired, report.Findings[0].Code)
	}

	// Probar AuditLatestSession
	latestReport, err := AuditLatestSession(tempDir, SessionAuditOptions{})
	if err != nil {
		t.Fatalf("audit latest session: %v", err)
	}
	if latestReport.SessionID != sess.ID {
		t.Errorf("expected session ID %s, got %s", sess.ID, latestReport.SessionID)
	}

	// Probar AuditSessionFile
	sessFile := filepath.Join(tempDir, sess.ID+".session.json")
	fileReport, err := AuditSessionFile(sessFile, SessionAuditOptions{})
	if err != nil {
		t.Fatalf("audit session file: %v", err)
	}
	if fileReport.SessionID != sess.ID {
		t.Errorf("expected session ID %s from file audit, got %s", sess.ID, fileReport.SessionID)
	}
}

func TestAuditSession_NotFound(t *testing.T) {
	tempDir := t.TempDir()
	report, err := AuditSessionByID(tempDir, "non-existent-id", SessionAuditOptions{})
	if err == nil {
		t.Fatalf("expected error for non-existent session, got nil")
	}
	if report.Status != "FAILED" {
		t.Errorf("expected status FAILED, got %s", report.Status)
	}
	if len(report.Findings) == 0 || report.Findings[0].Code != CodeSessionNotFound {
		t.Errorf("expected finding code %s", CodeSessionNotFound)
	}
}

func TestAuditSessionFile_Invalid(t *testing.T) {
	tempFile := filepath.Join(t.TempDir(), "invalid.txt")
	if err := os.WriteFile(tempFile, []byte("plain text content"), 0o644); err != nil {
		t.Fatalf("write invalid file: %v", err)
	}

	report, err := AuditSessionFile(tempFile, SessionAuditOptions{})
	if err == nil {
		t.Fatalf("expected error for invalid file format, got nil")
	}
	if report.Status != "FAILED" {
		t.Errorf("expected status FAILED, got %s", report.Status)
	}
}
