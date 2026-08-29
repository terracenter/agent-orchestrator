package heartbeat

import (
	"os"
	"path/filepath"
	"testing"
)

func TestRunDetectsManifests(t *testing.T) {
	root := t.TempDir()
	project := filepath.Join(root, "app")
	if err := os.MkdirAll(project, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(project, "go.mod"), []byte("module example.com/app\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(project, "package.json"), []byte("{}\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	report, err := Run(root)
	if err != nil {
		t.Fatal(err)
	}
	if len(report.Projects) != 1 {
		t.Fatalf("expected 1 project, got %d", len(report.Projects))
	}
	if len(report.Projects[0].Manifests) != 2 {
		t.Fatalf("expected 2 manifests, got %+v", report.Projects[0].Manifests)
	}
	if len(report.Sources) == 0 || len(report.Actions) == 0 {
		t.Fatalf("expected sources and actions: %+v", report)
	}
}

func TestRunSkipsHeavyDirectories(t *testing.T) {
	root := t.TempDir()
	skipped := filepath.Join(root, "node_modules", "dep")
	if err := os.MkdirAll(skipped, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(skipped, "package.json"), []byte("{}\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	report, err := Run(root)
	if err != nil {
		t.Fatal(err)
	}
	if len(report.Projects) != 0 {
		t.Fatalf("expected skipped projects, got %+v", report.Projects)
	}
}
