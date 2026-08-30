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

func TestCriticalDeployValidationPrioritizesOpus(t *testing.T) {
	decision := Decide("validar posible falso positivo en Deploy CWP falla: SSH exec request failed")
	if decision.Category != "revision_critica" {
		t.Fatalf("Category = %q, want revision_critica", decision.Category)
	}
	if decision.RecommendedAgent != "claude-code" || decision.RecommendedModel != AnthropicOpusCriticalModel {
		t.Fatalf("recommended = %s/%s, want claude-code/%s", decision.RecommendedAgent, decision.RecommendedModel, AnthropicOpusCriticalModel)
	}
	if !decision.RequiresConfirmation || !decision.SecurityOverride {
		t.Fatalf("decision = %+v, want confirmation and security override", decision)
	}
	if !contains(decision.AllowedAgents, "claude-code/"+AnthropicOpusCriticalModel) {
		t.Fatalf("AllowedAgents = %+v, want claude-code/%s", decision.AllowedAgents, AnthropicOpusCriticalModel)
	}
}

func TestProductionWorkflowValidationPrioritizesOpus(t *testing.T) {
	decision := Decide("revisión crítica de workflow GitHub Actions de producción antes de cambiar deploy")
	if decision.Category != "revision_critica" {
		t.Fatalf("Category = %q, want revision_critica", decision.Category)
	}
	if decision.RecommendedModel != AnthropicOpusCriticalModel {
		t.Fatalf("RecommendedModel = %q, want %s", decision.RecommendedModel, AnthropicOpusCriticalModel)
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
	if decision.RecommendedAgent != "claude-code" || decision.RecommendedModel != AnthropicSonnetReviewModel {
		t.Fatalf("recommended = %s/%s, want claude-code/%s", decision.RecommendedAgent, decision.RecommendedModel, AnthropicSonnetReviewModel)
	}
	if !contains(decision.AllowedAgents, "claude-code/"+AnthropicSonnetReviewModel) || !contains(decision.AllowedAgents, "claude-code/"+AnthropicOpusCriticalModel) {
		t.Fatalf("AllowedAgents = %+v, want exact sonnet and opus", decision.AllowedAgents)
	}
}

func TestMechanicalAllowsClaudeHaikuButAvoidsExpensiveClaude(t *testing.T) {
	decision := Decide("corregir typo simple")
	if !contains(decision.AllowedAgents, "claude-code/"+AnthropicHaikuCheapModel) {
		t.Fatalf("AllowedAgents = %+v, want claude-code/%s", decision.AllowedAgents, AnthropicHaikuCheapModel)
	}
	if !contains(decision.AvoidAgents, "claude-code/"+AnthropicSonnetReviewModel) || !contains(decision.AvoidAgents, "claude-code/"+AnthropicOpusCriticalModel) {
		t.Fatalf("AvoidAgents = %+v, want expensive exact Claude models", decision.AvoidAgents)
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

func TestDecideFallbackAssignment(t *testing.T) {
	cases := []struct {
		task          string
		wantFallbackA string
		wantFallbackM string
	}{
		{"revisión crítica de deploy", "claude-code", AnthropicSonnetReviewModel},
		{"rotar token de acceso y credenciales api", "claude-code", AnthropicOpusCriticalModel},
		{"ordenar notas de vault", "pi", "cheap-or-fast"},
		{"refactor de codigo go", "pi", "gpt-5.5"},
		{"corregir typo simple", "nvidia-api", "openai/gpt-oss-20b"},
	}

	for _, tc := range cases {
		decision := Decide(tc.task)
		if decision.FallbackAgent != tc.wantFallbackA || decision.FallbackModel != tc.wantFallbackM {
			t.Errorf("Decide(%q) fallback = %s/%s, want %s/%s", tc.task, decision.FallbackAgent, decision.FallbackModel, tc.wantFallbackA, tc.wantFallbackM)
		}
		fa, fm, ok := ResolveFallback(decision)
		if !ok || fa != tc.wantFallbackA || fm != tc.wantFallbackM {
			t.Errorf("ResolveFallback for %q = (%s, %s, %t), want (%s, %s, true)", tc.task, fa, fm, ok, tc.wantFallbackA, tc.wantFallbackM)
		}
	}
}
