package receipt

import (
	"path/filepath"
	"testing"
)

func TestVerifyValidReceipt(t *testing.T) {
	r := New("tarea", "Pi", "openai", "gpt", "bajo", 12)
	r.Commands = []Command{{Cmd: "rtk go test ./...", Result: "passed"}}
	r.Evidence = []string{"PR #12"}
	r.Rollback = "revert PR #12"
	if findings := Verify(r); len(findings) != 0 {
		t.Fatalf("expected valid receipt, got %+v", findings)
	}
}

func TestVerifyAcceptsMultipleCommandResults(t *testing.T) {
	r := New("tarea", "Pi", "openai", "gpt", "bajo", 12)
	r.Commands = []Command{{Cmd: "rtk go test ./...", Result: "passed"}, {Cmd: "rtk go vet ./...", Result: "skipped"}}
	r.Evidence = []string{"logs"}
	r.Rollback = "revert PR #12"
	if findings := Verify(r); len(findings) != 0 {
		t.Fatalf("expected multiple commands to be valid, got %+v", findings)
	}
}

func TestVerifyAcceptsFailedCommandResult(t *testing.T) {
	r := New("tarea", "Pi", "openai", "gpt", "bajo", 12)
	r.Commands = []Command{{Cmd: "rtk go test ./...", Result: "failed"}}
	r.Evidence = []string{"log de fallo preservado"}
	r.Rollback = "revert PR #12"
	if findings := Verify(r); len(findings) != 0 {
		t.Fatalf("expected failed result to remain verifiable, got %+v", findings)
	}
}

func TestVerifyRejectsUnknownCommandResult(t *testing.T) {
	r := New("tarea", "Pi", "openai", "gpt", "bajo", 12)
	r.Commands = []Command{{Cmd: "rtk go test ./...", Result: "maybe"}}
	r.Evidence = []string{"log"}
	r.Rollback = "revert"
	if findings := Verify(r); len(findings) == 0 {
		t.Fatal("expected invalid command result finding")
	}
}

func TestVerifyAcceptsHumanEditsRequiredWithNotes(t *testing.T) {
	r := New("tarea", "Pi", "openai", "gpt", "bajo", 12)
	r.Commands = []Command{{Cmd: "rtk go test ./...", Result: "passed"}}
	r.Evidence = []string{"log"}
	r.Rollback = "revert"
	r.HumanEditsRequired = true
	r.CorreccionesHumanasRequeridas = true
	r.HumanEditsNotes = []string{"Freddy ajusto copy final"}
	if findings := Verify(r); len(findings) != 0 {
		t.Fatalf("expected valid receipt, got %+v", findings)
	}
}

func TestVerifyRejectsHumanEditsRequiredWithoutNotes(t *testing.T) {
	r := New("tarea", "Pi", "openai", "gpt", "bajo", 12)
	r.Commands = []Command{{Cmd: "rtk go test ./...", Result: "passed"}}
	r.Evidence = []string{"log"}
	r.Rollback = "revert"
	r.HumanEditsRequired = true
	r.CorreccionesHumanasRequeridas = true
	if findings := Verify(r); len(findings) == 0 {
		t.Fatal("expected human edits notes finding")
	}
}

func TestVerifyAcceptsHumanEditsRequiredValueUnknownOrInteger(t *testing.T) {
	for _, value := range []string{"unknown", "0", "3"} {
		r := New("tarea", "Pi", "openai", "gpt", "bajo", 12)
		r.Commands = []Command{{Cmd: "rtk go test ./...", Result: "passed"}}
		r.Evidence = []string{"log"}
		r.Rollback = "revert"
		r.HumanEditsRequiredValue = value
		if findings := Verify(r); len(findings) != 0 {
			t.Fatalf("expected %q valid, got %+v", value, findings)
		}
	}
}

func TestVerifyRejectsInvalidHumanEditsRequiredValue(t *testing.T) {
	r := New("tarea", "Pi", "openai", "gpt", "bajo", 12)
	r.Commands = []Command{{Cmd: "rtk go test ./...", Result: "passed"}}
	r.Evidence = []string{"log"}
	r.Rollback = "revert"
	r.HumanEditsRequiredValue = "true"
	if findings := Verify(r); len(findings) == 0 {
		t.Fatal("expected invalid human edits value finding")
	}
}

func TestVerifyRequiresEvidence(t *testing.T) {
	r := New("tarea", "Pi", "openai", "gpt", "bajo", 0)
	findings := Verify(r)
	if len(findings) == 0 {
		t.Fatal("expected findings")
	}
}

func TestFromPRBuildsVerifiableReceipt(t *testing.T) {
	r := FromPR(PRInfo{Number: 36, Title: "feat: recibos", URL: "https://example.test/pr/36", MergeCommit: "abc123", Files: []string{"cmd/orq/main.go"}, Checks: []string{"go-test SUCCESS"}}, "Pi", "openai", "gpt", "bajo")
	if r.PR != 36 || r.Task != "feat: recibos" {
		t.Fatalf("unexpected receipt: %+v", r)
	}
	if len(r.FilesChanged) != 1 || len(r.Commands) != 1 || len(r.Evidence) != 3 {
		t.Fatalf("missing PR evidence: %+v", r)
	}
	if findings := Verify(r); len(findings) != 0 {
		t.Fatalf("expected verifiable receipt, got %+v", findings)
	}
}

func TestSaveLoad(t *testing.T) {
	path := filepath.Join(t.TempDir(), "receipt.json")
	r := New("tarea", "Pi", "openai", "gpt", "bajo", 1)
	r.Commands = []Command{{Cmd: "rtk test", Result: "passed"}}
	r.Evidence = []string{"commit abc"}
	r.Rollback = "revert"
	if err := Save(path, r); err != nil {
		t.Fatal(err)
	}
	loaded, err := Load(path)
	if err != nil {
		t.Fatal(err)
	}
	if loaded.Task != r.Task || loaded.PR != r.PR {
		t.Fatalf("unexpected loaded receipt: %+v", loaded)
	}
}

func TestVerifyDetectsRtkViolations(t *testing.T) {
	r := New("tarea", "Pi", "openai", "gpt", "bajo", 12)
	r.Commands = []Command{
		{Cmd: "rtk go test ./...", Result: "passed"},
		{Cmd: "go vet ./...", Result: "skipped"},
	}
	r.Evidence = []string{"logs"}
	r.Rollback = "revert PR #12"

	findings := Verify(r)
	if len(findings) == 0 {
		t.Fatal("esperaba encontrar hallazgos de violacion de rtk")
	}

	r.RtkViolations = []string{"go vet ./..."}
	findingsAfter := Verify(r)
	if len(findingsAfter) != 0 {
		t.Fatalf("esperaba que el recibo fuera valido despues de declarar la violacion, hallazgos: %+v", findingsAfter)
	}
}

func TestVerifyGhDirectViolatesRtk(t *testing.T) {
	r := New("tarea", "Pi", "openai", "gpt", "bajo", 12)
	r.Commands = []Command{
		{Cmd: "gh pr view 1", Result: "passed"},
	}
	r.Evidence = []string{"logs"}
	r.Rollback = "revert PR #12"

	findings := Verify(r)
	if len(findings) == 0 {
		t.Fatal("esperaba encontrar hallazgos de violacion de rtk para command 'gh pr view 1' directo")
	}

	// rtk gh pr view 1 debe pasar
	r2 := New("tarea", "Pi", "openai", "gpt", "bajo", 12)
	r2.Commands = []Command{
		{Cmd: "rtk gh pr view 1", Result: "passed"},
	}
	r2.Evidence = []string{"logs"}
	r2.Rollback = "revert PR #12"

	findings2 := Verify(r2)
	if len(findings2) != 0 {
		t.Fatalf("esperaba que 'rtk gh pr view 1' fuera valido, hallazgos: %+v", findings2)
	}
}

func TestVerifyCdDirectAllowed(t *testing.T) {
	r := New("tarea", "Pi", "openai", "gpt", "bajo", 12)
	r.Commands = []Command{
		{Cmd: "cd /home/freddy/Workspace/Desarrollo/agent-orchestrator", Result: "passed"},
	}
	r.Evidence = []string{"logs"}
	r.Rollback = "revert PR #12"

	findings := Verify(r)
	if len(findings) != 0 {
		t.Fatalf("esperaba que 'cd ...' directo fuera permitido, hallazgos: %+v", findings)
	}
}

