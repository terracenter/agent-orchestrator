package budget

import "strings"

type CompactCapability struct {
	Agent       string `json:"agent"`
	Mode        string `json:"mode"`
	CanAuto     bool   `json:"can_auto_compact"`
	Instruction string `json:"instruction"`
}

func CompactCapabilityFor(agent string) CompactCapability {
	normalized := strings.ToLower(strings.TrimSpace(agent))
	if normalized == "" {
		normalized = "unknown"
	}
	capability := CompactCapability{
		Agent:       normalized,
		Mode:        "manual_user_required",
		CanAuto:     false,
		Instruction: "El orquestador no puede compactar automaticamente esta sesion/agente. Pide al usuario ejecutar /compact antes de continuar.",
	}
	switch normalized {
	case "orq":
		capability.Mode = "orchestrator_only"
		capability.CanAuto = true
		capability.Instruction = "Orq solo calcula la politica de compactacion; especifica --agent para una sesion real."
	case "pi", "pi-api", "claude", "claude-code", "codex", "hermes", "openclaw", "agy":
		capability.Instruction = "Ejecuta /compact en este agente antes de continuar; orq solo puede indicarlo, no compactar la sesion por ti."
	}
	return capability
}
