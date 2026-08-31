package main

import (
	"testing"

	"github.com/terracenter/agent-orchestrator/internal/budget"
)

func TestBudgetLedgerEventCapturesDecision(t *testing.T) {
	advice := budget.DecideForAgentWithCompactApplied(42, 81, 10, "pi", true)
	event := budgetLedgerEvent("pi", advice)

	if event.Task != "budget decision" {
		t.Fatalf("unexpected task: %q", event.Task)
	}
	if event.Agent != "pi" {
		t.Fatalf("unexpected agent: %q", event.Agent)
	}
	if event.Model != "budget-policy" {
		t.Fatalf("unexpected model: %q", event.Model)
	}
	if event.Status != "budget_"+advice.Action {
		t.Fatalf("unexpected status: %q", event.Status)
	}
	if event.Notes == "" {
		t.Fatal("expected notes with budget decision details")
	}
}
