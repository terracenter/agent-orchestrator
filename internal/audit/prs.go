package audit

import (
	"encoding/json"
	"fmt"
	"os/exec"
	"strings"
)

// PullRequest describes the read-only delivery status of an open PR.
type PullRequest struct {
	Number         int           `json:"number"`
	Title          string        `json:"title"`
	URL            string        `json:"url"`
	Mergeable      string        `json:"mergeable"`
	ReviewDecision string        `json:"review_decision"`
	Checks         []StatusCheck `json:"checks"`
	Blocked        bool          `json:"blocked"`
	Blockers       []string      `json:"blockers,omitempty"`
}

type StatusCheck struct {
	Name       string `json:"name"`
	Workflow   string `json:"workflow,omitempty"`
	Status     string `json:"status"`
	Conclusion string `json:"conclusion,omitempty"`
	Required   bool   `json:"required,omitempty"`
}

type PullRequestAudit struct {
	Repository string        `json:"repository"`
	Pulls      []PullRequest `json:"pulls"`
	Warnings   []string      `json:"warnings,omitempty"`
}

// AuditPullRequests reads PR state through gh CLI. It never approves, merges, or mutates.
func AuditPullRequests(repoPath string) (PullRequestAudit, error) {
	report := PullRequestAudit{Repository: repoPath}
	cmd := exec.Command("gh", "pr", "list", "--state", "open", "--json", "number,title,url,mergeable,reviewDecision,statusCheckRollup")
	cmd.Dir = repoPath
	out, err := cmd.Output()
	if err != nil {
		return report, fmt.Errorf("gh pr list failed: %w", err)
	}
	var raw []struct {
		Number            int             `json:"number"`
		Title             string          `json:"title"`
		URL               string          `json:"url"`
		Mergeable         string          `json:"mergeable"`
		ReviewDecision    string          `json:"reviewDecision"`
		StatusCheckRollup json.RawMessage `json:"statusCheckRollup"`
	}
	if err := json.Unmarshal(out, &raw); err != nil {
		return report, fmt.Errorf("parse gh pr list: %w", err)
	}
	for _, item := range raw {
		pr := PullRequest{Number: item.Number, Title: item.Title, URL: item.URL, Mergeable: item.Mergeable, ReviewDecision: item.ReviewDecision}
		pr.Checks = parseChecks(item.StatusCheckRollup)
		pr.Blockers = blockers(pr)
		pr.Blocked = len(pr.Blockers) > 0
		report.Pulls = append(report.Pulls, pr)
	}
	return report, nil
}

func blockers(pr PullRequest) []string {
	var out []string
	if strings.EqualFold(pr.ReviewDecision, "REVIEW_REQUIRED") {
		out = append(out, "review requerida por identidad distinta con permisos")
	}
	if pr.Mergeable != "MERGEABLE" {
		out = append(out, "mergeable="+pr.Mergeable)
	}
	for _, check := range pr.Checks {
		if check.Status != "COMPLETED" || (check.Conclusion != "" && check.Conclusion != "SUCCESS") {
			out = append(out, "check no exitoso: "+check.Name)
		}
	}
	return out
}

func parseChecks(raw json.RawMessage) []StatusCheck {
	var checks []StatusCheck
	var items []map[string]any
	if err := json.Unmarshal(raw, &items); err != nil {
		return checks
	}
	for _, item := range items {
		checks = append(checks, StatusCheck{
			Name:       stringField(item, "name"),
			Workflow:   stringField(item, "workflowName"),
			Status:     stringField(item, "status"),
			Conclusion: stringField(item, "conclusion"),
			Required:   boolField(item, "required"),
		})
	}
	return checks
}

func stringField(m map[string]any, key string) string {
	v, _ := m[key].(string)
	return v
}

func boolField(m map[string]any, key string) bool {
	v, _ := m[key].(bool)
	return v
}
