package audit

import agentpkg "github.com/terracenter/agent-orchestrator/internal/agent"

type ModelAudit struct {
	Models   []ModelStatus `json:"models"`
	Findings []string      `json:"findings,omitempty"`
}

type ModelStatus struct {
	Agent      string `json:"agent"`
	Provider   string `json:"provider"`
	Model      string `json:"model"`
	CostLevel  int    `json:"cost_level"`
	UseFor     string `json:"use_for"`
	ReviewOnly bool   `json:"review_only"`
	Verified   bool   `json:"verified"`
	Assignable bool   `json:"assignable"`
	Reason     string `json:"reason,omitempty"`
}

func AuditModels() ModelAudit {
	report := ModelAudit{}
	for _, profile := range agentpkg.DefaultProfiles {
		status := ModelStatus{Agent: profile.Agent, Provider: profile.Provider, Model: profile.Model, CostLevel: profile.CostLevel, UseFor: profile.UseFor, ReviewOnly: profile.ReviewOnly, Verified: profile.Verified, Assignable: profile.Verified && !profile.ReviewOnly}
		if !profile.Verified {
			status.Reason = "modelo no verificado; no asignable"
			report.Findings = append(report.Findings, profile.Agent+"/"+profile.Model+": no verificado")
		} else if profile.ReviewOnly {
			status.Reason = "review_only; no asignable para ejecucion"
		}
		report.Models = append(report.Models, status)
	}
	return report
}
