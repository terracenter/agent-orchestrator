package guard

import (
	"fmt"
	"os"
	"path/filepath"
)

type CheckResult struct {
	Name   string `json:"name"`
	Passed bool   `json:"passed"`
	Reason string `json:"reason,omitempty"`
}

func AntiSplitBrain(vaultPath string) []CheckResult {
	if vaultPath == "" {
		return []CheckResult{{Name: "vault-path", Passed: false, Reason: "vault path is required"}}
	}
	return []CheckResult{
		checkNoEngramExport(vaultPath),
	}
}

func checkNoEngramExport(vaultPath string) CheckResult {
	path := filepath.Join(vaultPath, "engram")
	_, err := os.Stat(path)
	if err == nil {
		return CheckResult{Name: "no-engram-obsidian-export", Passed: false, Reason: fmt.Sprintf("forbidden path exists: %s", path)}
	}
	if os.IsNotExist(err) {
		return CheckResult{Name: "no-engram-obsidian-export", Passed: true}
	}
	return CheckResult{Name: "no-engram-obsidian-export", Passed: false, Reason: err.Error()}
}

func Passed(results []CheckResult) bool {
	for _, result := range results {
		if !result.Passed {
			return false
		}
	}
	return true
}
