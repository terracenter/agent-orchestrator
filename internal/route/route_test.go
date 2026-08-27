package route

import "testing"

func TestSecurityOverridesCost(t *testing.T) {
	decision := Decide("rotar token de producción")
	if !decision.SecurityOverride || !decision.RequiresConfirmation {
		t.Fatalf("decision = %+v, want security override with confirmation", decision)
	}
	if decision.RecommendedLevel < 3 {
		t.Fatalf("RecommendedLevel = %d, want >= 3", decision.RecommendedLevel)
	}
}

func TestMechanicalUsesLowLevel(t *testing.T) {
	decision := Decide("corregir referencia rota")
	if decision.RecommendedLevel > 1 {
		t.Fatalf("RecommendedLevel = %d, want <= 1", decision.RecommendedLevel)
	}
}
