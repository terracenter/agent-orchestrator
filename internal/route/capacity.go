package route

import (
	"fmt"
	"strings"
	"time"
)

const LowCapacityPercent = 15.0

type CapacitySnapshot struct {
	Agent            string     `json:"agent"`
	ProviderGroup    string     `json:"provider_group"`
	ModelGroup       string     `json:"model_group"`
	RemainingPercent *float64   `json:"remaining_percent,omitempty"`
	UsedPercent      *float64   `json:"used_percent,omitempty"`
	WindowLabel      string     `json:"window"`
	ResetsAt         *time.Time `json:"resets_at,omitempty"`
	Source           string     `json:"source"`
	CapturedAt       time.Time  `json:"captured_at"`
}

func ApplyCapacity(decision Decision, snapshots []CapacitySnapshot) Decision {
	if decision.SecurityOverride || len(snapshots) == 0 {
		return decision
	}
	current := lowestRemainingForAgent(snapshots, decision.RecommendedAgent)
	if current == nil || *current >= LowCapacityPercent {
		return decision
	}
	for _, candidate := range decision.AllowedAgents {
		agent, model := splitAgentModel(candidate)
		if agent == "" || agent == decision.RecommendedAgent || containsAgent(decision.AvoidAgents, agent) {
			continue
		}
		remaining := lowestRemainingForAgent(snapshots, agent)
		if remaining != nil && *remaining < LowCapacityPercent {
			continue
		}
		decision.FallbackAgent = decision.RecommendedAgent
		decision.FallbackModel = decision.RecommendedModel
		decision.RecommendedAgent = agent
		decision.RecommendedModel = model
		decision.Reason = fmt.Sprintf("%s; capacity: %s below %.0f%%, using %s", decision.Reason, decision.FallbackAgent, LowCapacityPercent, agent)
		return decision
	}
	decision.Reason = fmt.Sprintf("%s; capacity: %s below %.0f%% but no better allowed candidate found", decision.Reason, decision.RecommendedAgent, LowCapacityPercent)
	return decision
}

func lowestRemainingForAgent(snapshots []CapacitySnapshot, agent string) *float64 {
	needle := strings.ToLower(strings.TrimSpace(agent))
	var lowest *float64
	for _, snapshot := range snapshots {
		if strings.ToLower(strings.TrimSpace(snapshot.Agent)) != needle || snapshot.RemainingPercent == nil {
			continue
		}
		value := *snapshot.RemainingPercent
		if lowest == nil || value < *lowest {
			copy := value
			lowest = &copy
		}
	}
	return lowest
}

func splitAgentModel(candidate string) (string, string) {
	candidate = strings.TrimSpace(candidate)
	if candidate == "" {
		return "", ""
	}
	agent, model, ok := strings.Cut(candidate, "/")
	if !ok {
		return candidate, ""
	}
	return agent, model
}

func containsAgent(candidates []string, agent string) bool {
	for _, candidate := range candidates {
		candidateAgent, _ := splitAgentModel(candidate)
		if candidateAgent == agent {
			return true
		}
	}
	return false
}
