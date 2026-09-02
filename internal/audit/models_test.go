package audit

import (
	"testing"

	agentpkg "github.com/terracenter/agent-orchestrator/internal/agent"
)

func TestAuditModelsMarksUnverifiedNotAssignable(t *testing.T) {
	profiles := []agentpkg.Profile{
		{Agent: "pi", Provider: "openai", Model: "gpt-5.5", CostLevel: 2, UseFor: "orquestacion", Verified: true},
		{Agent: "agy", Provider: "google", Model: "gemini-3.5-flash-low", CostLevel: 1, UseFor: "test", Verified: false},
		{Agent: "claude-code", Provider: "anthropic", Model: "claude-sonnet", CostLevel: 3, UseFor: "review", ReviewOnly: true, Verified: true},
	}
	report := AuditModels(profiles)
	var sawUnverified bool
	for _, model := range report.Models {
		if !model.Verified {
			sawUnverified = true
			if model.Assignable {
				t.Fatalf("unverified model should not be assignable: %+v", model)
			}
		}
		if model.ReviewOnly && model.Assignable {
			t.Fatalf("review-only model should not be assignable: %+v", model)
		}
	}
	if !sawUnverified || len(report.Findings) == 0 {
		t.Fatalf("expected unverified findings: %+v", report)
	}
}
