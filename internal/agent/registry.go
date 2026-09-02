package agent

import (
	"encoding/json"
	"fmt"
	"os"
	"strings"
)

type Profile struct {
	Agent      string `json:"agent"`
	Provider   string `json:"provider"`
	Model      string `json:"model"`
	CostLevel  int    `json:"cost_level"`
	UseFor     string `json:"use_for"`
	ReviewOnly bool   `json:"review_only"`
	Verified   bool   `json:"verified"`
}

const (
	DefaultProfilesPath = "config/agent-profiles.json"
	ProfilesEnv         = "ORQ_AGENT_PROFILES"
)

func LoadProfiles(path ...string) ([]Profile, error) {
	targetPath := ""
	if len(path) > 0 && strings.TrimSpace(path[0]) != "" {
		targetPath = path[0]
	} else if envPath := os.Getenv(ProfilesEnv); strings.TrimSpace(envPath) != "" {
		targetPath = envPath
	} else {
		targetPath = DefaultProfilesPath
	}

	data, err := os.ReadFile(targetPath)
	if err != nil {
		return nil, fmt.Errorf("failed to read agent profiles file %s: %w", targetPath, err)
	}

	var profiles []Profile
	if err := json.Unmarshal(data, &profiles); err != nil {
		return nil, fmt.Errorf("failed to unmarshal agent profiles JSON from %s: %w", targetPath, err)
	}

	for i, p := range profiles {
		if strings.TrimSpace(p.Agent) == "" ||
			strings.TrimSpace(p.Provider) == "" ||
			strings.TrimSpace(p.Model) == "" ||
			strings.TrimSpace(p.UseFor) == "" ||
			p.CostLevel < 0 {
			return nil, fmt.Errorf("invalid profile at index %d in %s: agent, provider, model, use_for must be non-empty and cost_level >= 0", i, targetPath)
		}
	}

	return profiles, nil
}

func Find(profiles []Profile, agentName string, model string) (Profile, error) {
	for _, profile := range profiles {
		if profile.Agent == agentName && profile.Model == model {
			return profile, nil
		}
	}
	return Profile{}, fmt.Errorf("unknown agent/model pair %s/%s", agentName, model)
}
