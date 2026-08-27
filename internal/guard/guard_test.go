package guard

import (
	"os"
	"path/filepath"
	"testing"
)

func TestAntiSplitBrainPassesWithoutEngramExport(t *testing.T) {
	results := AntiSplitBrain(t.TempDir())
	if !Passed(results) {
		t.Fatalf("Passed() = false, results=%+v", results)
	}
}

func TestAntiSplitBrainFailsWithEngramExport(t *testing.T) {
	vault := t.TempDir()
	if err := os.Mkdir(filepath.Join(vault, "engram"), 0o755); err != nil {
		t.Fatal(err)
	}
	results := AntiSplitBrain(vault)
	if Passed(results) {
		t.Fatalf("Passed() = true, want false, results=%+v", results)
	}
}
