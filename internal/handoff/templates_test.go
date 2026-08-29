package handoff

import (
	"strings"
	"testing"

	"github.com/terracenter/agent-orchestrator/internal/task"
)

func TestDraftWithTemplateReviewer4R(t *testing.T) {
	item := task.Item{ID: "t1", Title: "revisar PR", State: task.Assigned, Agent: "agy", Model: "gemini-3.5-flash-low", Host: "minipc"}
	draft, err := DraftWithTemplate(item, "reviewer-4r")
	if err != nil {
		t.Fatal(err)
	}
	ordered := []string{"<contexto_estatico>", "<contexto_estable>", "<contexto_dinamico>", "<tarea>"}
	last := -1
	for _, marker := range ordered {
		idx := strings.Index(draft, marker)
		if idx <= last {
			t.Fatalf("marker %s is missing or out of order:\n%s", marker, draft)
		}
		last = idx
	}
	for _, want := range []string{"Agente: agy", "Provider:", "Modelo: gemini-3.5-flash-low", "No usar --dangerously-skip-permissions", "RDD:"} {
		if !strings.Contains(draft, want) {
			t.Fatalf("draft missing %q:\n%s", want, draft)
		}
	}
}

func TestDraftWithTemplateRejectsUnknown(t *testing.T) {
	_, err := DraftWithTemplate(task.Item{Title: "x"}, "unknown")
	if err == nil {
		t.Fatal("expected unknown template error")
	}
}
