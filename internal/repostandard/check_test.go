package repostandard

import (
	"os"
	"path/filepath"
	"testing"
)

func TestCheckRepoFailsMissingRequiredFiles(t *testing.T) {
	dir := t.TempDir()
	report := CheckRepo(dir)
	if report.Passed {
		t.Fatal("expected missing required files to fail")
	}
}

func TestCheckRepoPassesWithRequiredFiles(t *testing.T) {
	dir := t.TempDir()
	files := []string{
		"README.md", "SECURITY.md", "CONTRIBUTING.md", "LICENSE", ".env.example", ".gitignore", ".github/pull_request_template.md", "docs/politica-branch-protection.md", "Makefile",
	}
	for _, file := range files {
		write(t, filepath.Join(dir, file))
	}
	for _, dirPath := range []string{".github/workflows", "docs/diagramas"} {
		if err := os.MkdirAll(filepath.Join(dir, dirPath), 0o755); err != nil {
			t.Fatal(err)
		}
	}
	report := CheckRepo(dir)
	if !report.Passed {
		t.Fatalf("expected pass: %+v", report.Checks)
	}
}

func write(t *testing.T, path string) {
	t.Helper()
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, []byte("ok\n"), 0o644); err != nil {
		t.Fatal(err)
	}
}
