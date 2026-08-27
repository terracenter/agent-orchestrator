package handoff

import (
	"strings"
	"testing"
	"time"

	"github.com/terracenter/agent-orchestrator/internal/task"
)

func TestDraft(t *testing.T) {
	item := task.Item{ID: "t1", Title: "ordenar vault GLPI", State: task.Assigned, Agent: "pi", Model: "cheap-or-fast", Host: "minipc", CreatedAt: time.Now(), UpdatedAt: time.Now()}
	draft := Draft(item)
	for _, want := range []string{"# HANDOFF: ordenar vault GLPI", "Task ID: t1", "Agente: pi", "Usa rtk", "vg"} {
		if !strings.Contains(draft, want) {
			t.Fatalf("draft missing %q:\n%s", want, draft)
		}
	}
}
