package audit

import (
	"encoding/json"
	"fmt"
	"os/exec"
	"strings"
)

type IssueAudit struct {
	Repository string         `json:"repository"`
	Issues     []IssueSummary `json:"issues"`
	Findings   []string       `json:"findings,omitempty"`
}

type IssueSummary struct {
	Number int      `json:"number"`
	Title  string   `json:"title"`
	URL    string   `json:"url"`
	Labels []string `json:"labels,omitempty"`
}

func AuditIssues(repoPath string) (IssueAudit, error) {
	report := IssueAudit{Repository: repoPath}
	cmd := exec.Command("gh", "issue", "list", "--state", "open", "--limit", "100", "--json", "number,title,url,labels")
	cmd.Dir = repoPath
	out, err := cmd.Output()
	if err != nil {
		return report, fmt.Errorf("gh issue list failed: %w", err)
	}
	var raw []struct {
		Number int    `json:"number"`
		Title  string `json:"title"`
		URL    string `json:"url"`
		Labels []struct {
			Name string `json:"name"`
		} `json:"labels"`
	}
	if err := json.Unmarshal(out, &raw); err != nil {
		return report, fmt.Errorf("parse gh issue list: %w", err)
	}
	byLabel := map[string]int{}
	for _, item := range raw {
		issue := IssueSummary{Number: item.Number, Title: item.Title, URL: item.URL}
		for _, label := range item.Labels {
			issue.Labels = append(issue.Labels, label.Name)
			byLabel[strings.ToLower(label.Name)]++
		}
		report.Issues = append(report.Issues, issue)
	}
	if len(report.Issues) >= 10 {
		report.Findings = append(report.Findings, "10+ issues abiertos: sugerir auditoria arquitectonica")
	}
	for label, count := range byLabel {
		if count >= 3 {
			report.Findings = append(report.Findings, fmt.Sprintf("acumulacion por label %q: %d issues", label, count))
		}
	}
	return report, nil
}
