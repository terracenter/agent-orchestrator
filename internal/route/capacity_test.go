package route

import "testing"

func TestApplyCapacityKeepsSecurityOverride(t *testing.T) {
	low := 1.0
	decision := Decide("deploy produccion por ssh")
	got := ApplyCapacity(decision, []CapacitySnapshot{{Agent: decision.RecommendedAgent, RemainingPercent: &low}})
	if got.RecommendedAgent != decision.RecommendedAgent || !got.SecurityOverride {
		t.Fatalf("security decision changed: %+v", got)
	}
}

func TestApplyCapacityMovesLowCapacityRecommendationToAllowedCandidate(t *testing.T) {
	low := 5.0
	decision := Decide("tarea mecanica simple")
	decision.RecommendedAgent = "agy"
	decision.RecommendedModel = "gemini-3.5-flash-low"
	decision.AllowedAgents = []string{"agy/gemini-3.5-flash-low", "pi/cheap-or-fast", "local"}
	got := ApplyCapacity(decision, []CapacitySnapshot{{Agent: "agy", RemainingPercent: &low}})
	if got.RecommendedAgent != "pi" {
		t.Fatalf("expected pi recommendation, got %+v", got)
	}
	if got.FallbackAgent != "agy" || got.FallbackModel != "gemini-3.5-flash-low" {
		t.Fatalf("fallback not preserved: %+v", got)
	}
}

func TestApplyCapacityKeepsHealthyRecommendation(t *testing.T) {
	healthy := 64.5
	decision := Decide("tarea mecanica simple")
	got := ApplyCapacity(decision, []CapacitySnapshot{{Agent: decision.RecommendedAgent, RemainingPercent: &healthy}})
	if got.RecommendedAgent != decision.RecommendedAgent {
		t.Fatalf("healthy decision changed: %+v", got)
	}
}
