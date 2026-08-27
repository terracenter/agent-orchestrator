package ledger

import (
	"path/filepath"
	"testing"
)

func TestAppendAndReadAll(t *testing.T) {
	path := filepath.Join(t.TempDir(), "ledger.jsonl")
	event := Event{Task: "test", Agent: "pi", Model: "gpt-5.5", Status: "ok"}
	if err := Append(path, event); err != nil {
		t.Fatalf("Append() error = %v", err)
	}
	events, err := ReadAll(path)
	if err != nil {
		t.Fatalf("ReadAll() error = %v", err)
	}
	if len(events) != 1 {
		t.Fatalf("len(events) = %d, want 1", len(events))
	}
	got := events[0]
	if got.Agent != "pi" || got.Model != "gpt-5.5" || got.Status != "ok" {
		t.Fatalf("event = %+v", got)
	}
}

func TestAppendRequiresFields(t *testing.T) {
	path := filepath.Join(t.TempDir(), "ledger.jsonl")
	if err := Append(path, Event{}); err == nil {
		t.Fatal("Append() error = nil, want error")
	}
}
