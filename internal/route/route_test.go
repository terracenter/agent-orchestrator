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

func TestRouteAlwaysRequiresRTK(t *testing.T) {
	decision := Decide("cualquier tarea con comandos")
	if !decision.RtkRequired {
		t.Fatal("RtkRequired = false, want true")
	}
}

func TestSecurityAllowsClaudeSonnetAndOpus(t *testing.T) {
	decision := Decide("auditoria de seguridad de credenciales")
	if decision.RecommendedAgent != "claude-code" || decision.RecommendedModel != "sonnet" {
		t.Fatalf("recommended = %s/%s, want claude-code/sonnet", decision.RecommendedAgent, decision.RecommendedModel)
	}
	if !contains(decision.AllowedAgents, "claude-code/sonnet") || !contains(decision.AllowedAgents, "claude-code/opus") {
		t.Fatalf("AllowedAgents = %+v, want sonnet and opus", decision.AllowedAgents)
	}
}

func TestMechanicalAllowsClaudeHaikuButAvoidsExpensiveClaude(t *testing.T) {
	decision := Decide("corregir typo simple")
	if !contains(decision.AllowedAgents, "claude-code/haiku") {
		t.Fatalf("AllowedAgents = %+v, want claude-code/haiku", decision.AllowedAgents)
	}
	if !contains(decision.AvoidAgents, "claude-code/sonnet") || !contains(decision.AvoidAgents, "claude-code/opus") {
		t.Fatalf("AvoidAgents = %+v, want expensive Claude models", decision.AvoidAgents)
	}
}

func contains(values []string, want string) bool {
	for _, value := range values {
		if value == want {
			return true
		}
	}
	return false
}

func TestVaultDocumentationUsesCheapAgents(t *testing.T) {
	decision := Decide("ordenar informacion del vault relacionada con GLPI")
	if decision.Category != "documentacion" {
		t.Fatalf("Category = %q, want documentacion", decision.Category)
	}
	if decision.RecommendedLevel != 1 {
		t.Fatalf("RecommendedLevel = %d, want 1", decision.RecommendedLevel)
	}
	if len(decision.AvoidAgents) == 0 {
		t.Fatalf("AvoidAgents empty: %+v", decision)
	}
}
