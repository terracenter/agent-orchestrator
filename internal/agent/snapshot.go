package agent

import "time"

type CapabilitySnapshot struct {
	Agent        string             `json:"agent"`
	Provider     string             `json:"provider"`
	Model        string             `json:"model"`
	CostLevel    int                `json:"cost_level"`
	UseFor       string             `json:"use_for"`
	ReviewOnly   bool               `json:"review_only"`
	Verified     bool               `json:"verified"`
	Inputs       []string           `json:"inputs"`
	Outputs      []string           `json:"outputs"`
	Tools        []string           `json:"tools"`
	Modes        []string           `json:"modes"`
	Evidence     []CapabilitySource `json:"evidence"`
	CapturedAt   time.Time          `json:"captured_at"`
	SecurityNote string             `json:"security_note"`
}

type CapabilitySource struct {
	Kind        string `json:"kind"`
	Source      string `json:"source"`
	Description string `json:"description"`
}

func CapabilitySnapshots(capturedAt time.Time) []CapabilitySnapshot {
	snapshots := make([]CapabilitySnapshot, 0, len(DefaultProfiles))
	for _, profile := range DefaultProfiles {
		snapshots = append(snapshots, CapabilitySnapshot{
			Agent:        profile.Agent,
			Provider:     profile.Provider,
			Model:        profile.Model,
			CostLevel:    profile.CostLevel,
			UseFor:       profile.UseFor,
			ReviewOnly:   profile.ReviewOnly,
			Verified:     profile.Verified,
			Inputs:       inputsFor(profile.Agent),
			Outputs:      outputsFor(profile.Agent),
			Tools:        toolsFor(profile.Agent),
			Modes:        modesFor(profile),
			Evidence:     evidenceFor(profile),
			CapturedAt:   capturedAt,
			SecurityNote: "snapshot seguro: metadata operativa; no lee settings, tokens, .secrets ni archivos privados",
		})
	}
	return snapshots
}

func inputsFor(agent string) []string {
	switch agent {
	case "qwen-code", "claude-code", "agy", "pi", "openclaw", "hermes":
		return []string{"text", "files", "repo"}
	default:
		return []string{"text"}
	}
}

func outputsFor(agent string) []string {
	switch agent {
	case "qwen-code", "claude-code", "agy", "pi", "openclaw", "hermes":
		return []string{"text", "code", "patch", "markdown", "json"}
	default:
		return []string{"text", "json"}
	}
}

func toolsFor(agent string) []string {
	switch agent {
	case "qwen-code":
		return []string{"filesystem", "shell", "git", "github", "docker", "workspace-skills"}
	case "claude-code", "pi", "agy", "hermes", "openclaw":
		return []string{"filesystem", "shell", "git", "github", "workspace-skills"}
	default:
		return []string{}
	}
}

func modesFor(profile Profile) []string {
	if profile.ReviewOnly {
		return []string{"review", "debate", "architecture", "read-only"}
	}
	return []string{"chat", "agentic", "read-only", "edit", "review"}
}

func evidenceFor(profile Profile) []CapabilitySource {
	evidence := []CapabilitySource{{Kind: "registry", Source: "internal/agent/registry.go", Description: "perfil operativo declarado en Orq"}}
	if profile.Agent == "qwen-code" {
		evidence = append(evidence, CapabilitySource{Kind: "empirical", Source: "github issue #81", Description: "primer runtime Qwen Code reportado por usuario; qwen3.8-max verificado, otros modelos pendientes"})
	}
	if !profile.Verified {
		evidence = append(evidence, CapabilitySource{Kind: "status", Source: "orq", Description: "pendiente de validacion runtime/docs oficiales/evidencia empirica"})
	}
	return evidence
}
