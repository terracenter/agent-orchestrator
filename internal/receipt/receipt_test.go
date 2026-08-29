package receipt

import (
	"path/filepath"
	"testing"
)

func TestVerifyValidReceipt(t *testing.T) {
	r := New("tarea", "Pi", "openai", "gpt", "bajo", 12)
	r.Commands = []Command{{Cmd: "go test ./...", Result: "passed"}}
	r.Evidence = []string{"PR #12"}
	r.Rollback = "revert PR #12"
	if findings := Verify(r); len(findings) != 0 {
		t.Fatalf("expected valid receipt, got %+v", findings)
	}
}

func TestVerifyAcceptsFailedCommandResult(t *testing.T) {
	r := New("tarea", "Pi", "openai", "gpt", "bajo", 12)
	r.Commands = []Command{{Cmd: "go test ./...", Result: "failed"}}
	r.Evidence = []string{"log de fallo preservado"}
	r.Rollback = "revert PR #12"
	if findings := Verify(r); len(findings) != 0 {
		t.Fatalf("expected failed result to remain verifiable, got %+v", findings)
	}
}

func TestVerifyRejectsUnknownCommandResult(t *testing.T) {
	r := New("tarea", "Pi", "openai", "gpt", "bajo", 12)
	r.Commands = []Command{{Cmd: "go test ./...", Result: "maybe"}}
	r.Evidence = []string{"log"}
	r.Rollback = "revert"
	if findings := Verify(r); len(findings) == 0 {
		t.Fatal("expected invalid command result finding")
	}
}

func TestVerifyAcceptsHumanEditsRequiredWithNotes(t *testing.T) {
	r := New("tarea", "Pi", "openai", "gpt", "bajo", 12)
	r.Commands = []Command{{Cmd: "go test ./...", Result: "passed"}}
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
	r.Commands = []Command{{Cmd: "go test ./...", Result: "passed"}}
	r.Evidence = []string{"log"}
	r.Rollback = "revert"
	r.HumanEditsRequired = true
	r.CorreccionesHumanasRequeridas = true
	if findings := Verify(r); len(findings) == 0 {
		t.Fatal("expected human edits notes finding")
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
	r.Commands = []Command{{Cmd: "test", Result: "passed"}}
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
