package vaultorder

import (
	"os"
	"path/filepath"
	"testing"
)

func TestBuildPlan(t *testing.T) {
	vault := t.TempDir()
	write(t, filepath.Join(vault, "Planes", "GLPI", "plan.md"), "Integracion GLPI")
	write(t, filepath.Join(vault, "Planes", "GLPI", "00-index.md"), "Indice GLPI")
	write(t, filepath.Join(vault, "Notas", "incidente.md"), "fallo glpi")

	plan, err := Build(vault, "glpi")
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	if len(plan.Matches) != 3 {
		t.Fatalf("matches = %#v", plan.Matches)
	}
	if len(plan.Actions) != 3 {
		t.Fatalf("actions = %#v", plan.Actions)
	}
	if plan.Actions[0].Type != "create-index" || plan.Actions[1].Type != "consider-rename" || plan.Actions[2].Type != "consider-rename" {
		t.Fatalf("actions = %#v", plan.Actions)
	}
}

func write(t *testing.T, path string, content string) {
	t.Helper()
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, []byte(content), 0o644); err != nil {
		t.Fatal(err)
	}
}
