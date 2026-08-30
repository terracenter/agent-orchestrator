package route

import "strings"

type Decision struct {
	Task                 string   `json:"task"`
	Category             string   `json:"category"`
	RecommendedLevel     int      `json:"recommended_level"`
	RecommendedAgent     string   `json:"recommended_agent"`
	RecommendedModel     string   `json:"recommended_model"`
	AllowedAgents        []string `json:"allowed_agents"`
	AvoidAgents          []string `json:"avoid_agents,omitempty"`
	RequiresConfirmation bool     `json:"requires_confirmation"`
	SecurityOverride     bool     `json:"security_override"`
	RtkRequired          bool     `json:"rtk_required"`
	Reason               string   `json:"reason"`
}

func Classify(task string) string {
	text := strings.ToLower(task)
	for _, word := range []string{"token", "credential", "credencial", ".env", "producción", "produccion", "sudo", "systemd", "push --force", "reset --hard"} {
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

func Decide(task string) Decision {
	category := Classify(task)
	decision := Decision{Task: task, Category: category, RtkRequired: true}
	switch category {
	case "seguridad":
		decision.RecommendedLevel = 3
		decision.RecommendedAgent = "claude-code"
		decision.RecommendedModel = "sonnet"
		decision.AllowedAgents = []string{"claude-code/sonnet", "claude-code/opus"}
		decision.RequiresConfirmation = true
		decision.SecurityOverride = true
		decision.Reason = "seguridad sobrescribe costo; Sonnet por defecto, Opus solo para arquitectura/auditoria critica"
	case "documentacion":
		decision.RecommendedLevel = 1
		decision.RecommendedAgent = "agy"
		decision.RecommendedModel = "gpt-oss-120b-medium"
		decision.AllowedAgents = []string{"nvidia-api/openai/gpt-oss-20b", "agy/gpt-oss-120b-medium", "agy/gemini-3.5-flash-low", "pi/cheap-or-fast", "claude-code/haiku"}
		decision.AvoidAgents = []string{"claude-code/opus", "claude-code/sonnet"}
		decision.Reason = "documentacion/vault: descubrir con vg/rtk y ejecutar con agente barato; escalar solo si hay conflicto o riesgo"
	case "codigo":
		decision.RecommendedLevel = 2
		decision.RecommendedAgent = "agy"
		decision.RecommendedModel = "gemini-3.7-flash-high"
		decision.AllowedAgents = []string{"agy/gemini-3.7-flash-high", "agy/gemini-3.5-flash-low", "pi/gpt-5.5"}
		decision.AvoidAgents = []string{"claude-opus"}
		decision.Reason = "codigo entra por agente de implementacion antes de escalar"
	default:
		decision.RecommendedLevel = 1
		decision.RecommendedAgent = "local-or-cheap"
		decision.RecommendedModel = "lowest-sufficient"
		decision.AllowedAgents = []string{"nvidia-api/openai/gpt-oss-20b", "agy/gpt-oss-120b-medium", "agy/gemini-3.5-flash-low", "pi/cheap-or-fast", "claude-code/haiku", "local"}
		decision.AvoidAgents = []string{"claude-code/opus", "claude-code/sonnet"}
		decision.Reason = "tarea mecanica: usar escalon mas barato suficiente"
	}
	return decision
}
