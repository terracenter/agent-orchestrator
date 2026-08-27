package route

import "strings"

type Decision struct {
	Task                 string `json:"task"`
	Category             string `json:"category"`
	RecommendedLevel     int    `json:"recommended_level"`
	RecommendedAgent     string `json:"recommended_agent"`
	RecommendedModel     string `json:"recommended_model"`
	RequiresConfirmation bool   `json:"requires_confirmation"`
	SecurityOverride     bool   `json:"security_override"`
	Reason               string `json:"reason"`
}

func Classify(task string) string {
	text := strings.ToLower(task)
	for _, word := range []string{"token", "credential", "credencial", ".env", "producción", "produccion", "sudo", "systemd", "push --force", "reset --hard"} {
		if strings.Contains(text, word) {
			return "seguridad"
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
	decision := Decision{Task: task, Category: category}
	switch category {
	case "seguridad":
		decision.RecommendedLevel = 3
		decision.RecommendedAgent = "claude-code"
		decision.RecommendedModel = "sonnet"
		decision.RequiresConfirmation = true
		decision.SecurityOverride = true
		decision.Reason = "seguridad sobrescribe costo"
	case "codigo":
		decision.RecommendedLevel = 2
		decision.RecommendedAgent = "agy"
		decision.RecommendedModel = "gemini-flash-high"
		decision.Reason = "codigo entra por agente de implementacion antes de escalar"
	default:
		decision.RecommendedLevel = 1
		decision.RecommendedAgent = "local-or-cheap"
		decision.RecommendedModel = "lowest-sufficient"
		decision.Reason = "tarea mecanica: usar escalon mas barato suficiente"
	}
	return decision
}
