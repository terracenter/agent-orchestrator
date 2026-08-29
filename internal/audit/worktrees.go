package audit

import (
	"bytes"
	"fmt"
	"os/exec"
	"strings"
)

// WorktreeReport summarizes local git worktrees for collision hygiene.
type WorktreeReport struct {
	Root      string           `json:"root"`
	Worktrees []WorktreeStatus `json:"worktrees"`
	Findings  []string         `json:"findings"`
}

// WorktreeStatus describes one git worktree.
type WorktreeStatus struct {
	Path     string `json:"path"`
	Branch   string `json:"branch,omitempty"`
	Commit   string `json:"commit,omitempty"`
	Detached bool   `json:"detached"`
	Bare     bool   `json:"bare"`
	Prunable bool   `json:"prunable"`
	Reason   string `json:"reason,omitempty"`
}

// AuditWorktrees reads git worktree metadata without modifying the repo.
func AuditWorktrees(path string) (WorktreeReport, error) {
	cmd := exec.Command("git", "-C", path, "worktree", "list", "--porcelain")
	out, err := cmd.Output()
	if err != nil {
		return WorktreeReport{}, fmt.Errorf("git worktree list failed: %w", err)
	}
	report := WorktreeReport{Root: path, Worktrees: ParseWorktrees(out)}
	for _, wt := range report.Worktrees {
		if wt.Prunable {
			report.Findings = append(report.Findings, fmt.Sprintf("worktree prunable: %s %s", wt.Path, wt.Reason))
		}
		if wt.Detached && !wt.Bare {
			report.Findings = append(report.Findings, "worktree detached: "+wt.Path)
		}
	}
	return report, nil
}

// ParseWorktrees parses `git worktree list --porcelain` output.
func ParseWorktrees(data []byte) []WorktreeStatus {
	blocks := bytes.Split(bytes.TrimSpace(data), []byte("\n\n"))
	var items []WorktreeStatus
	for _, block := range blocks {
		if len(bytes.TrimSpace(block)) == 0 {
			continue
		}
		var wt WorktreeStatus
		for _, line := range strings.Split(string(block), "\n") {
			key, value, ok := strings.Cut(line, " ")
			if !ok {
				key = line
			}
			switch key {
			case "worktree":
				wt.Path = value
			case "HEAD":
				wt.Commit = value
			case "branch":
				wt.Branch = strings.TrimPrefix(value, "refs/heads/")
			case "detached":
				wt.Detached = true
			case "bare":
				wt.Bare = true
			case "prunable":
				wt.Prunable = true
				wt.Reason = value
			}
		}
		items = append(items, wt)
	}
	return items
}
