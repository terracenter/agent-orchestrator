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

func TestVerifyRequiresEvidence(t *testing.T) {
	r := New("tarea", "Pi", "openai", "gpt", "bajo", 0)
	findings := Verify(r)
	if len(findings) == 0 {
		t.Fatal("expected findings")
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
