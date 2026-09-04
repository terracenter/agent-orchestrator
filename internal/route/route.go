package route

import "strings"

const (
	AnthropicOpusCriticalModel = "claude-opus-4-1-20250805"
	AnthropicSonnetReviewModel = "claude-sonnet-4-5-20250929"
	AnthropicHaikuCheapModel   = "claude-3-5-haiku-20241022"

	HermesFlashModel = "deepseek-v4-flash"
	HermesProModel   = "deepseek-v4-pro"
)

type Decision struct {
	Task                 string   `json:"task"`
	Category             string   `json:"category"`
	RecommendedLevel     int      `json:"recommended_level"`
	RecommendedAgent     string   `json:"recommended_agent"`
	RecommendedModel     string   `json:"recommended_model"`
	FallbackAgent        string   `json:"fallback_agent,omitempty"`
	FallbackModel        string   `json:"fallback_model,omitempty"`
	AllowedAgents        []string `json:"allowed_agents"`
	AvoidAgents          []string `json:"avoid_agents,omitempty"`
	RequiresConfirmation bool     `json:"requires_confirmation"`
	SecurityOverride     bool     `json:"security_override"`
	RtkRequired          bool     `json:"rtk_required"`
	Reason               string   `json:"reason"`
}

func Classify(task string) string {
	text := strings.ToLower(task)
	if isCriticalReview(text) {
		return "revision_critica"
	}
	for _, word := range []string{"token", "credential", "credencial", ".env", "sudo", "systemd", "push --force", "reset --hard"} {
		if strings.Contains(text, word) {
			return "seguridad"
		}
	}
	for _, word := range []string{"vault", "obsidian", "glpi", "ordenar", "indice", "índice", "documentacion", "documentación", "markdown", "moc"} {
		if strings.Contains(text, word) {
			return "documentacion"
		}
	}
	for _, word := range []string{"refactor", "código", "codigo", "go", "test", "bug"} {
		if strings.Contains(text, word) {
			return "codigo"
		}
	}
	return "mecanico"
}

func isCriticalReview(text string) bool {
	criticalSignals := []string{
		"validacion critica", "validación crítica", "revision critica", "revisión crítica",
		"posible falso positivo", "falso positivo", "diagnostico dudoso", "diagnóstico dudoso",
		"deploy", "despliegue", "produccion", "producción", "cwp", "workflow", "github actions", "ci/cd",
		"ssh", "exec request failed", "postmortem", "incidente",
	}
	for _, signal := range criticalSignals {
		if strings.Contains(text, signal) {
			return true
		}
	}
	return false
}

func Decide(task string) Decision {
	category := Classify(task)
	decision := Decision{Task: task, Category: category, RtkRequired: true}
	switch category {
	case "revision_critica":
		decision.RecommendedLevel = 4
		decision.RecommendedAgent = "claude-code"
		decision.RecommendedModel = AnthropicOpusCriticalModel
		decision.FallbackAgent = "claude-code"
		decision.FallbackModel = AnthropicSonnetReviewModel
		decision.AllowedAgents = []string{"claude-code/" + AnthropicOpusCriticalModel, "claude-code/" + AnthropicSonnetReviewModel}
		decision.RequiresConfirmation = true
		decision.SecurityOverride = true
		decision.Reason = "revision critica de produccion/deploy/CI o posible falso positivo: priorizar Opus como validador experto antes de actuar"
	case "seguridad":
		decision.RecommendedLevel = 3
		decision.RecommendedAgent = "claude-code"
		decision.RecommendedModel = AnthropicSonnetReviewModel
		decision.FallbackAgent = "claude-code"
		decision.FallbackModel = AnthropicOpusCriticalModel
		decision.AllowedAgents = []string{"claude-code/" + AnthropicSonnetReviewModel, "claude-code/" + AnthropicOpusCriticalModel}
		decision.RequiresConfirmation = true
		decision.SecurityOverride = true
		decision.Reason = "seguridad sobrescribe costo; Sonnet por defecto, Opus solo para arquitectura/auditoria critica"
	case "documentacion":
		decision.RecommendedLevel = 1
		decision.RecommendedAgent = "hermes"
		decision.RecommendedModel = HermesFlashModel
		decision.FallbackAgent = "pi"
		decision.FallbackModel = "cheap-or-fast"
		decision.AllowedAgents = []string{
			"hermes/" + HermesFlashModel,
			"hermes/" + HermesProModel,
			"nvidia-api/openai/gpt-oss-20b",
			"agy/gpt-oss-120b-medium",
			"agy/gemini-3.5-flash-low",
			"pi/cheap-or-fast",
			"claude-code/" + AnthropicHaikuCheapModel,
		}
		decision.AvoidAgents = []string{"claude-code/" + AnthropicOpusCriticalModel, "claude-code/" + AnthropicSonnetReviewModel}
		decision.Reason = "documentacion/vault: descubrir con vg/rtk y ejecutar con agente barato; escalar solo si hay conflicto o riesgo"
	case "codigo":
		decision.RecommendedLevel = 2
		decision.RecommendedAgent = "agy"
		decision.RecommendedModel = "gemini-3.7-flash-high"
		decision.FallbackAgent = "pi"
		decision.FallbackModel = "gpt-5.5"
		decision.AllowedAgents = []string{"agy/gemini-3.7-flash-high", "agy/gemini-3.5-flash-low", "hermes/" + HermesProModel, "pi/gpt-5.5"}
		decision.AvoidAgents = []string{"claude-code/" + AnthropicOpusCriticalModel}
		decision.Reason = "codigo entra por agente de implementacion antes de escalar"
	default:
		decision.RecommendedLevel = 1
		decision.RecommendedAgent = "hermes"
		decision.RecommendedModel = HermesFlashModel
		decision.FallbackAgent = "nvidia-api"
		decision.FallbackModel = "openai/gpt-oss-20b"
		decision.AllowedAgents = []string{
			"hermes/" + HermesFlashModel,
			"hermes/" + HermesProModel,
			"nvidia-api/openai/gpt-oss-20b",
			"agy/gpt-oss-120b-medium",
			"agy/gemini-3.5-flash-low",
			"pi/cheap-or-fast",
			"claude-code/" + AnthropicHaikuCheapModel,
			"local",
		}
		decision.AvoidAgents = []string{"claude-code/" + AnthropicOpusCriticalModel, "claude-code/" + AnthropicSonnetReviewModel}
		decision.Reason = "tarea mecanica: usar escalon mas barato suficiente"
	}
	return decision
}

// ResolveFallback checks if a decision provides a configured fallback agent and model.
func ResolveFallback(decision Decision) (fallbackAgent, fallbackModel string, hasFallback bool) {
	if decision.FallbackAgent != "" && decision.FallbackModel != "" {
		return decision.FallbackAgent, decision.FallbackModel, true
	}
	return "", "", false
}
