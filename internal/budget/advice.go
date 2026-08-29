package budget

import "fmt"

type Advice struct {
	ContextPercent           float64           `json:"context_percent"`
	Codex5hPercent           float64           `json:"codex_5h_percent"`
	WeeklyPercent            float64           `json:"weekly_percent"`
	Action                   string            `json:"action"`
	UseAgents                []string          `json:"use_agents"`
	AvoidAgents              []string          `json:"avoid_agents"`
	CompactPrompt            string            `json:"compact_prompt,omitempty"`
	PreflightCompactRequired bool              `json:"preflight_compact_required"`
	CompactCapability        CompactCapability `json:"compact_capability"`
	CompactInstruction       string            `json:"compact_instruction"`
	ManualCompactStop        bool              `json:"manual_compact_stop"`
	CompactApplied           bool              `json:"compact_applied"`
	Reason                   string            `json:"reason"`
}

func Decide(contextPercent, codex5hPercent, weeklyPercent float64) Advice {
	return DecideForAgent(contextPercent, codex5hPercent, weeklyPercent, "orq")
}

func DecideForAgent(contextPercent, codex5hPercent, weeklyPercent float64, agent string) Advice {
	return DecideForAgentWithCompactApplied(contextPercent, codex5hPercent, weeklyPercent, agent, false)
}

func DecideForAgentWithCompactApplied(contextPercent, codex5hPercent, weeklyPercent float64, agent string, compactApplied bool) Advice {
	capability := CompactCapabilityFor(agent)
	advice := Advice{
		ContextPercent: contextPercent,
		Codex5hPercent: codex5hPercent,
		WeeklyPercent:  weeklyPercent,
		UseAgents: []string{
			"nvidia-api/openai/gpt-oss-20b",
			"nvidia-api/openai/gpt-oss-120b",
			"agy/gpt-oss-120b-medium",
			"agy/gemini-3.5-flash-low",
		},
		AvoidAgents:              []string{"pi/openai/gpt-5.5", "claude-opus", "claude-sonnet"},
		CompactPrompt:            CompactPrompt(),
		PreflightCompactRequired: true,
		CompactCapability:        capability,
		CompactInstruction:       capability.Instruction,
		ManualCompactStop:        !capability.CanAuto && !compactApplied,
		CompactApplied:           compactApplied,
	}

	switch {
	case codex5hPercent >= 95:
		advice.Action = "pausar"
		advice.Reason = "limite corto Codex casi agotado; esperar reset antes de consumir Pi/OpenAI"
	case contextPercent >= 65:
		advice.Action = "compactar"
		advice.Reason = "contexto alto; compactar antes de continuar"
		advice.CompactPrompt = CompactPrompt()
	case codex5hPercent >= 80 || weeklyPercent >= 75:
		advice.Action = "delegar_barato"
		advice.Reason = "presupuesto OpenAI tensionado; usar NVIDIA/AGY y evitar Pi/OpenAI"
	case contextPercent >= 40:
		advice.Action = "compactar_pronto"
		advice.Reason = "contexto medio; preparar compactacion y reducir lecturas largas"
		advice.CompactPrompt = CompactPrompt()
	default:
		advice.Action = "continuar"
		advice.Reason = "presupuesto aceptable; continuar priorizando modelos baratos"
	}
	if advice.ManualCompactStop && advice.Action != "pausar" {
		advice.Action = "compactar_manual"
		advice.Reason = "compactacion preflight obligatoria; este agente no puede compactar automaticamente"
	} else if compactApplied && !capability.CanAuto && advice.Action == "continuar" {
		advice.Reason = "compactacion manual declarada por el usuario; continuar con bloque pequeno y verificar presupuesto al terminar"
	}
	return advice
}

func CompactPrompt() string {
	return "/compact Compacta agresivamente. Conserva solo: objetivo actual, decisiones vigentes, archivos modificados, comandos validados, issues activos, modelos baratos verificados y próximos pasos. Elimina outputs largos e historial narrativo."
}

func ValidatePercent(name string, value float64) error {
	if value < 0 || value > 100 {
		return fmt.Errorf("%s must be between 0 and 100", name)
	}
	return nil
}
