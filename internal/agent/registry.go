package agent

import "fmt"

type Profile struct {
	Agent      string `json:"agent"`
	Model      string `json:"model"`
	CostLevel  int    `json:"cost_level"`
	UseFor     string `json:"use_for"`
	ReviewOnly bool   `json:"review_only"`
}

var DefaultProfiles = []Profile{
	{Agent: "pi", Model: "gpt-5.5", CostLevel: 1, UseFor: "orquestacion, tareas mecanicas, documentacion, ejecucion local"},
	{Agent: "pi", Model: "cheap-or-fast", CostLevel: 1, UseFor: "alias de menor costo suficiente para tareas mecanicas/documentales"},
	{Agent: "haiku", Model: "haiku", CostLevel: 1, UseFor: "tareas mecanicas con instrucciones cerradas"},
	{Agent: "agy", Model: "gemini-flash-high", CostLevel: 2, UseFor: "implementacion de codigo y analisis tecnico medio"},
	{Agent: "claude-code", Model: "sonnet", CostLevel: 3, UseFor: "revision critica, seguridad, bloqueos y refutacion de decisiones", ReviewOnly: true},
	{Agent: "claude-code", Model: "opus", CostLevel: 4, UseFor: "arquitectura compleja o decision mayor", ReviewOnly: true},
}

func Find(agentName string, model string) (Profile, error) {
	for _, profile := range DefaultProfiles {
		if profile.Agent == agentName && profile.Model == model {
			return profile, nil
		}
	}
	return Profile{}, fmt.Errorf("unknown agent/model pair %s/%s", agentName, model)
}
