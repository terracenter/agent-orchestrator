package score

import (
	"testing"

	"github.com/terracenter/agent-orchestrator/internal/ledger"
)

func TestFromLedgerPenalizesNotExecutedWithoutTreatingItAsSuccess(t *testing.T) {
	summaries := FromLedger([]ledger.Event{
		{Agent: "agy", Model: "flash", Status: "ok", Task: "doc"},
		{Agent: "agy", Model: "flash", Status: "not_executed", Task: "delegate"},
		{Agent: "qwen-code", Model: "qwen3.8-max", Status: "ok", Task: "code"},
	})

	if len(summaries) != 2 {
		t.Fatalf("expected 2 summaries, got %d", len(summaries))
	}
	if summaries[0].Agent != "qwen-code" || summaries[0].Score != 1 {
		t.Fatalf("expected qwen-code first with score 1, got %+v", summaries[0])
	}
	agy := summaries[1]
	if agy.NotExecuted != 1 {
		t.Fatalf("expected not_executed count, got %+v", agy)
	}
	if agy.Score >= 1 {
		t.Fatalf("not_executed must lower score, got %+v", agy)
	}
}

func TestFromLedgerTreatsUnknownStatusesAsFailures(t *testing.T) {
	summaries := FromLedger([]ledger.Event{{Agent: "pi", Model: "gpt", Status: "weird", Task: "x"}})
	if len(summaries) != 1 {
		t.Fatalf("expected one summary, got %d", len(summaries))
	}
	if summaries[0].Failures != 1 || summaries[0].Score != -1 {
		t.Fatalf("expected failure score, got %+v", summaries[0])
	}
}
