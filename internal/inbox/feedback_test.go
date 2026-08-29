package inbox

import (
	"os"
	"path/filepath"
	"testing"
)

func TestScanFeedbacksParsesHermesFeedback(t *testing.T) {
	dir := t.TempDir()
	content := `# Feedback Hermes/orq

- Tarea: Integrar observer
- Task ID: observer-orq-integration
- Agente: Hermes

## Resultado
INTEGRACION_LOCAL_OK: evento recibido

## Siguiente paso para Pi/Claude
Nada.
`
	if err := os.WriteFile(filepath.Join(dir, "2026-feedback.md"), []byte(content), 0o644); err != nil {
		t.Fatal(err)
	}
	items, err := ScanFeedbacks(dir)
	if err != nil {
		t.Fatal(err)
	}
	if len(items) != 1 {
		t.Fatalf("expected 1 item, got %d", len(items))
	}
	item := items[0]
	if item.TaskID != "observer-orq-integration" || item.Agent != "Hermes" {
		t.Fatalf("unexpected parsed item: %+v", item)
	}
	if item.Result != "INTEGRACION_LOCAL_OK: evento recibido" {
		t.Fatalf("unexpected result: %q", item.Result)
	}
	if !item.NextForPi {
		t.Fatalf("expected next_for_pi")
	}
}

func TestNextFeedbackReturnsActionableItem(t *testing.T) {
	items := []FeedbackResume{{File: "info.md"}, {File: "pi.md", NextForPi: true}}
	item, ok := NextFeedback(items)
	if !ok || item.File != "pi.md" {
		t.Fatalf("unexpected next item: ok=%t item=%+v", ok, item)
	}
}

func TestNextUnseenFeedbackSkipsAcknowledgedItems(t *testing.T) {
	items := []FeedbackResume{{Path: "/new.md", NextForPi: true}, {Path: "/old.md", NeedsHuman: true}}
	item, ok := NextUnseenFeedback(items, SeenSet{"/new.md": true})
	if !ok || item.Path != "/old.md" {
		t.Fatalf("unexpected next unseen item: ok=%t item=%+v", ok, item)
	}
}

func TestMarkSeenIsIdempotent(t *testing.T) {
	seenFile := filepath.Join(t.TempDir(), "seen.txt")
	if err := MarkSeen(seenFile, "/tmp/feedback.md"); err != nil {
		t.Fatal(err)
	}
	if err := MarkSeen(seenFile, "/tmp/feedback.md"); err != nil {
		t.Fatal(err)
	}
	seen, err := LoadSeen(seenFile)
	if err != nil {
		t.Fatal(err)
	}
	if !seen["/tmp/feedback.md"] || len(seen) != 1 {
		t.Fatalf("unexpected seen set: %+v", seen)
	}
}

func TestScanFeedbacksMarksHumanBlockers(t *testing.T) {
	dir := t.TempDir()
	content := `- Task ID: x
BLOCKED_HUMAN_SECRET: falta token local
`
	if err := os.WriteFile(filepath.Join(dir, "blocked-feedback.md"), []byte(content), 0o644); err != nil {
		t.Fatal(err)
	}
	items, err := ScanFeedbacks(dir)
	if err != nil {
		t.Fatal(err)
	}
	if !items[0].NeedsHuman {
		t.Fatalf("expected needs_human")
	}
}
