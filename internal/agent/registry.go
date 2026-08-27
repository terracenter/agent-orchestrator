package agent

import "fmt"

type Profile struct {
	Agent      string `json:"agent"`
	Provider   string `json:"provider"`
	Model      string `json:"model"`
	CostLevel  int    `json:"cost_level"`
	UseFor     string `json:"use_for"`
	ReviewOnly bool   `json:"review_only"`
	Verified   bool   `json:"verified"`
}

var DefaultProfiles = []Profile{
	{Agent: "pi", Provider: "openai", Model: "gpt-5.5", CostLevel: 2, UseFor: "orquestacion principal y sintesis de decisiones", Verified: true},
	{Agent: "pi", Provider: "openai", Model: "cheap-or-fast", CostLevel: 1, UseFor: "alias de menor costo suficiente para tareas mecanicas/documentales", Verified: true},
	{Agent: "pi", Provider: "nvidia", Model: "free-or-low-cost", CostLevel: 0, UseFor: "tareas mecanicas, resumen y clasificacion cuando el provider este disponible", Verified: false},
	{Agent: "haiku", Provider: "anthropic", Model: "haiku", CostLevel: 1, UseFor: "tareas mecanicas con instrucciones cerradas", Verified: true},
	{Agent: "agy", Provider: "google", Model: "gemini-flash-high", CostLevel: 1, UseFor: "implementacion de codigo y analisis tecnico medio", Verified: true},
	{Agent: "agy", Provider: "google", Model: "gemini-pro", CostLevel: 2, UseFor: "analisis tecnico fuerte y refutacion antes de escalar a Claude", Verified: false},
	{Agent: "agy", Provider: "nvidia", Model: "free-or-low-cost", CostLevel: 0, UseFor: "tareas mecanicas y validaciones baratas si AGY lo expone", Verified: false},
	{Agent: "claude-code", Provider: "anthropic", Model: "sonnet", CostLevel: 3, UseFor: "revision critica, seguridad, bloqueos y refutacion de decisiones", ReviewOnly: true, Verified: true},
	{Agent: "claude-code", Provider: "anthropic", Model: "opus", CostLevel: 4, UseFor: "arquitectura compleja o decision mayor", ReviewOnly: true, Verified: true},
}

func Find(agentName string, model string) (Profile, error) {
	for _, profile := range DefaultProfiles {
		if profile.Agent == agentName && profile.Model == model {
			return profile, nil
		}
	}
	return Profile{}, fmt.Errorf("unknown agent/model pair %s/%s", agentName, model)
}
