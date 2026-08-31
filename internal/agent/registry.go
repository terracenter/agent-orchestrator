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
	{Agent: "nvidia-api", Provider: "nvidia", Model: "openai/gpt-oss-20b", CostLevel: 0, UseFor: "smoke tests, clasificacion barata y tareas mecanicas con API NVIDIA", Verified: true},
	{Agent: "nvidia-api", Provider: "nvidia", Model: "openai/gpt-oss-120b", CostLevel: 0, UseFor: "validacion barata y razonamiento hospedado con API NVIDIA", Verified: true},
	{Agent: "pi", Provider: "nvidia", Model: "free-or-low-cost", CostLevel: 0, UseFor: "tareas mecanicas, resumen y clasificacion cuando el provider este disponible", Verified: false},
	{Agent: "claude-code", Provider: "anthropic", Model: "claude-3-5-haiku-20241022", CostLevel: 1, UseFor: "tareas mecanicas con instrucciones cerradas cuando se quiere usar Claude barato; modelo exacto Anthropic", Verified: true},
	{Agent: "claude-code", Provider: "anthropic", Model: "haiku", CostLevel: 1, UseFor: "alias humano; no usar en validaciones criticas", Verified: true},
	{Agent: "agy", Provider: "google", Model: "gemini-3.5-flash-low", CostLevel: 1, UseFor: "tareas mecanicas, clasificacion y validaciones baratas", Verified: true},
	{Agent: "agy", Provider: "google", Model: "gemini-3.5-flash-medium", CostLevel: 1, UseFor: "tareas rapidas, resumenes y documentacion", Verified: false},
	{Agent: "agy", Provider: "google", Model: "gemini-3.7-flash-high", CostLevel: 1, UseFor: "implementacion de codigo y analisis tecnico medio", Verified: false},
	{Agent: "agy", Provider: "google", Model: "gemini-3.1-pro-low", CostLevel: 2, UseFor: "analisis tecnico fuerte y refutacion antes de escalar a Claude", Verified: false},
	{Agent: "agy", Provider: "open-model", Model: "gpt-oss-120b-medium", CostLevel: 0, UseFor: "validacion barata de prompts, resumenes y tareas mecanicas cuando AGY lo expone", Verified: true},
	{Agent: "agy", Provider: "nvidia", Model: "free-or-low-cost", CostLevel: 0, UseFor: "tareas mecanicas y validaciones baratas si AGY lo expone", Verified: false},
	{Agent: "qwen-code", Provider: "bailian", Model: "qwen3.8-max", CostLevel: 1, UseFor: "codigo, busqueda en repos, shell/git/docker y tareas tecnicas medianas bajo plan Standard reportado por runtime", Verified: true},
	{Agent: "qwen-code", Provider: "bailian", Model: "qwen3.5", CostLevel: 1, UseFor: "modelo disponible reportado por Qwen Code; validar empiricamente antes de asignacion critica", Verified: false},
	{Agent: "qwen-code", Provider: "bailian", Model: "qwen3.6", CostLevel: 1, UseFor: "modelo disponible reportado por Qwen Code; validar empiricamente antes de asignacion critica", Verified: false},
	{Agent: "qwen-code", Provider: "bailian", Model: "qwen3.7-plus", CostLevel: 1, UseFor: "modelo disponible reportado por Qwen Code; validar empiricamente antes de asignacion critica", Verified: false},
	{Agent: "claude-code", Provider: "anthropic", Model: "claude-sonnet-4-5-20250929", CostLevel: 3, UseFor: "codigo, revision critica, seguridad, bloqueos y refutacion de decisiones; modelo exacto Anthropic", ReviewOnly: true, Verified: true},
	{Agent: "claude-code", Provider: "anthropic", Model: "sonnet", CostLevel: 3, UseFor: "alias humano; no usar en validaciones criticas", ReviewOnly: true, Verified: true},
	{Agent: "claude-code", Provider: "anthropic", Model: "claude-opus-4-1-20250805", CostLevel: 4, UseFor: "arquitectura compleja, auditoria critica o decision mayor; modelo exacto Anthropic", ReviewOnly: true, Verified: true},
	{Agent: "claude-code", Provider: "anthropic", Model: "opus", CostLevel: 4, UseFor: "alias humano; no usar en validaciones criticas", ReviewOnly: true, Verified: true},
	{Agent: "claude-code", Provider: "anthropic", Model: "fable", CostLevel: 2, UseFor: "modelo Claude pendiente de clasificar; usar solo si la CLI lo expone y hay confirmacion operativa", ReviewOnly: true, Verified: false},
}

func Find(agentName string, model string) (Profile, error) {
	for _, profile := range DefaultProfiles {
		if profile.Agent == agentName && profile.Model == model {
			return profile, nil
		}
	}
	return Profile{}, fmt.Errorf("unknown agent/model pair %s/%s", agentName, model)
}
