package route

import "testing"

func TestTaskKindBridge(t *testing.T) {
	cases := map[string]string{
		"revision_critica": "deep_reasoning",
		"seguridad":        "simple_cybersecurity",
		"documentacion":    "documentation",
		"codigo":           "small_refactor",
		"mecanico":         "mechanical",
	}
	for category, want := range cases {
		if got := TaskKind(category); got != want {
			t.Errorf("TaskKind(%q) = %q, want %q", category, got, want)
		}
	}
}

func TestDecisionIncludesTaskKind(t *testing.T) {
	decision := Decide("ordenar informacion del vault")
	if decision.TaskKind != "documentation" {
		t.Fatalf("TaskKind = %q, want documentation", decision.TaskKind)
	}
}

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
	if decision.RecommendedAgent != "hermes" || decision.RecommendedModel != HermesFlashModel {
		t.Fatalf("recommended = %s/%s, want hermes/%s", decision.RecommendedAgent, decision.RecommendedModel, HermesFlashModel)
	}
	if !contains(decision.AllowedAgents, "hermes/"+HermesFlashModel) {
		t.Fatalf("AllowedAgents = %+v, want hermes/%s", decision.AllowedAgents, HermesFlashModel)
	}
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
	if decision.RecommendedAgent != "hermes" || decision.RecommendedModel != HermesFlashModel {
		t.Fatalf("recommended = %s/%s, want hermes/%s", decision.RecommendedAgent, decision.RecommendedModel, HermesFlashModel)
	}
	if !contains(decision.AllowedAgents, "hermes/"+HermesFlashModel) {
		t.Fatalf("AllowedAgents = %+v, want hermes/%s", decision.AllowedAgents, HermesFlashModel)
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

func TestClassifyDoesNotMatchGoAsSubstring(t *testing.T) {
	cases := []string{
		"revisar el pago del hosting",
		"renombrar algo en el servidor",
		"backup de agosto",
	}
	for _, task := range cases {
		if got := Classify(task); got == "codigo" {
			t.Errorf("Classify(%q) = %q, no debería clasificar como codigo por contener \"go\" como substring", task, got)
		}
	}

	goCases := []string{
		"revisar el código Go de este módulo",
		"escribir un script en go",
		"fix de bug en go",
	}
	for _, task := range goCases {
		if got := Classify(task); got != "codigo" {
			t.Errorf("Classify(%q) = %q, want \"codigo\"", task, got)
		}
	}
}

func TestClassifyDoesNotMatchCriticalSignalAsFilenameSubstring(t *testing.T) {
	// Reproducción literal del issue #200: el nombre de archivo contiene los substrings
	// "deploy" y "cwp" (señales de revision_critica), pero unidos por guiones a otras
	// palabras — no son la señal como palabra/frase independiente.
	task := "numerar con prefijo 0N. el archivo deploy-cwp-estandar.md de una carpeta del vault y corregir sus wikilinks"
	if got := Classify(task); got == "revision_critica" {
		t.Errorf("Classify(%q) = %q, no debería clasificar como revision_critica por \"deploy\"/\"cwp\" embebidos en un nombre de archivo", task, got)
	}
	if got := Classify(task); got != "documentacion" {
		t.Errorf("Classify(%q) = %q, want \"documentacion\"", task, got)
	}

	// Control negativo del issue: mismo tipo de tarea, sin el nombre de archivo problemático.
	control := "numerar con prefijo 0N. los archivos security-manager-go-dup.md y security-manager-ng.md de una carpeta del vault y corregir sus wikilinks"
	if got := Classify(control); got != Classify(task) {
		t.Errorf("Classify(%q) = %q, Classify(%q) = %q, ambas deberían clasificar igual", task, Classify(task), control, got)
	}

	// Otros substrings embebidos en nombres de archivo/rutas, sin espacio/puntuación real
	// alrededor de la señal, tampoco deben disparar revision_critica.
	filenameCases := []string{
		"revisar el script deploy-helper.sh del repo",
		"leer sshconfig-notes.md antes de continuar",
		"el archivo produccion-2026.csv ya fue importado",
	}
	for _, task := range filenameCases {
		if got := Classify(task); got == "revision_critica" {
			t.Errorf("Classify(%q) = %q, no debería clasificar como revision_critica por señal embebida en nombre de archivo", task, got)
		}
	}
}

func TestClassifyStillMatchesCriticalSignalAsStandaloneWord(t *testing.T) {
	// Coincidencias exactas y parciales válidas: la señal aparece como palabra o frase
	// independiente (separada por espacios/puntuación real), con y sin acentos, y en medio
	// o al final del texto — debe seguir disparando revision_critica tras el fix.
	cases := []string{
		"hay que hacer deploy a produccion esta noche",
		"revisar via ssh el servidor",
		"incidente en cwp",
		"posible falso positivo en el reporte",
		"revisión crítica de workflow antes de mergear",
		"falla en ci/cd del pipeline",
		"postmortem del incidente de ayer",
	}
	for _, task := range cases {
		if got := Classify(task); got != "revision_critica" {
			t.Errorf("Classify(%q) = %q, want \"revision_critica\"", task, got)
		}
	}
}
