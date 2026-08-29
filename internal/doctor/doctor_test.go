package doctor

import (
	"context"
	"os"
	"path/filepath"
	"testing"
)

func TestRunBasicReport(t *testing.T) {
	ctx := context.Background()
	report := Run(ctx, Options{})

	if report.Summary.Total == 0 {
		t.Fatal("expected at least 1 tool check in report")
	}
	if len(report.Tools) != report.Summary.Total {
		t.Fatalf("expected tools count %d to match summary total %d", len(report.Tools), report.Summary.Total)
	}
}

func TestOpenClawSafeDetection(t *testing.T) {
	tempHome := t.TempDir()
	openclawDir := filepath.Join(tempHome, ".openclaw")
	if err := os.MkdirAll(openclawDir, 0o700); err != nil {
		t.Fatal(err)
	}
	// Add dummy binary in a bin dir
	binDir := filepath.Join(tempHome, ".local", "bin")
	if err := os.MkdirAll(binDir, 0o755); err != nil {
		t.Fatal(err)
	}
	binFile := filepath.Join(binDir, "openclaw")
	if err := os.WriteFile(binFile, []byte("#!/bin/sh\necho openclaw\n"), 0o755); err != nil {
		t.Fatal(err)
	}

	ctx := context.Background()
	check := checkOpenClaw(ctx, tempHome)
	if check.Status != StatusOK {
		t.Fatalf("expected openclaw status OK, got %s", check.Status)
	}
	if check.ConfigPath != openclawDir {
		t.Fatalf("expected config path %s, got %s", openclawDir, check.ConfigPath)
	}
	if check.Path != binFile {
		t.Fatalf("expected binary path %s, got %s", binFile, check.Path)
	}
}

func TestCheckToolMissing(t *testing.T) {
	emptyHome := t.TempDir()
	ctx := context.Background()
	check := checkOpenClaw(ctx, emptyHome)
	// If not in system PATH or tempHome, should be missing
	if check.Path == "" {
		if check.Status != StatusMissing {
			t.Fatalf("expected status missing when not installed, got %s", check.Status)
		}
	}
}

func TestCheckVG_ORQ_VG_PATH(t *testing.T) {
	tempDir := t.TempDir()
	customBin := filepath.Join(tempDir, "custom-vg")
	if err := os.WriteFile(customBin, []byte("#!/bin/sh\necho vg\n"), 0o755); err != nil {
		t.Fatal(err)
	}

	t.Setenv("ORQ_VG_PATH", customBin)
	ctx := context.Background()
	check := checkVG(ctx, t.TempDir())

	if check.Status != StatusOK {
		t.Fatalf("expected status OK, got %s", check.Status)
	}
	if check.Path != customBin {
		t.Fatalf("expected path %s, got %s", customBin, check.Path)
	}
	if check.Note != "detectado via ORQ_VG_PATH" {
		t.Fatalf("expected note 'detectado via ORQ_VG_PATH', got %s", check.Note)
	}
}

func TestCheckVG_LookPath(t *testing.T) {
	tempDir := t.TempDir()
	binDir := filepath.Join(tempDir, "bin")
	if err := os.MkdirAll(binDir, 0o755); err != nil {
		t.Fatal(err)
	}
	vgBin := filepath.Join(binDir, "vg")
	if err := os.WriteFile(vgBin, []byte("#!/bin/sh\necho vg\n"), 0o755); err != nil {
		t.Fatal(err)
	}

	t.Setenv("ORQ_VG_PATH", "")
	t.Setenv("PATH", binDir)

	ctx := context.Background()
	check := checkVG(ctx, t.TempDir())

	if check.Status != StatusOK {
		t.Fatalf("expected status OK, got %s", check.Status)
	}
	if check.Path != vgBin {
		t.Fatalf("expected path %s, got %s", vgBin, check.Path)
	}
	if check.Note != "" {
		t.Fatalf("expected empty note for PATH lookup, got %s", check.Note)
	}
}

func TestCheckVG_KnownPaths(t *testing.T) {
	cases := []struct {
		name       string
		relPath    string
		expectNote string
	}{
		{
			name:       "Obsidian 10.Tooling",
			relPath:    filepath.Join("Workspace", "Obsidian", "10.Tooling", "vault-graph", "vg"),
			expectNote: "detectado en ruta conocida del workspace",
		},
		{
			name:       "Obsidian Tooling scripts",
			relPath:    filepath.Join("Workspace", "Obsidian", "Tooling", "vault-graph", "scripts", "vg"),
			expectNote: "detectado en ruta conocida del workspace",
		},
		{
			name:       "Workspace Tooling scripts",
			relPath:    filepath.Join("Workspace", "Tooling", "vault-graph", "scripts", "vg"),
			expectNote: "detectado en ruta conocida del workspace",
		},
		{
			name:       "local bin",
			relPath:    filepath.Join(".local", "bin", "vg"),
			expectNote: "detectado en ruta conocida del workspace",
		},
	}

	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			tempHome := t.TempDir()
			targetPath := filepath.Join(tempHome, tc.relPath)
			if err := os.MkdirAll(filepath.Dir(targetPath), 0o755); err != nil {
				t.Fatal(err)
			}
			if err := os.WriteFile(targetPath, []byte("#!/bin/sh\necho vg\n"), 0o755); err != nil {
				t.Fatal(err)
			}

			t.Setenv("ORQ_VG_PATH", "")
			t.Setenv("PATH", "")

			ctx := context.Background()
			check := checkVG(ctx, tempHome)

			if check.Status != StatusOK {
				t.Fatalf("expected status OK, got %s", check.Status)
			}
			if check.Path != targetPath {
				t.Fatalf("expected path %s, got %s", targetPath, check.Path)
			}
			if check.Note != tc.expectNote {
				t.Fatalf("expected note %q, got %q", tc.expectNote, check.Note)
			}
		})
	}
}

func TestCheckVG_Missing(t *testing.T) {
	emptyHome := t.TempDir()
	t.Setenv("ORQ_VG_PATH", "")
	t.Setenv("PATH", "")

	ctx := context.Background()
	check := checkVG(ctx, emptyHome)

	if check.Status != StatusMissing {
		t.Fatalf("expected status missing, got %s", check.Status)
	}
	if check.Path != "" {
		t.Fatalf("expected empty path, got %s", check.Path)
	}
	if check.Recommendation == "" {
		t.Fatal("expected non-empty recommendation")
	}
}

func TestCheckVG_Precedence(t *testing.T) {
	tempHome := t.TempDir()
	knownBin := filepath.Join(tempHome, "Workspace", "Obsidian", "10.Tooling", "vault-graph", "vg")
	if err := os.MkdirAll(filepath.Dir(knownBin), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(knownBin, []byte("#!/bin/sh\necho vg-known\n"), 0o755); err != nil {
		t.Fatal(err)
	}

	pathDir := filepath.Join(tempHome, "bin")
	if err := os.MkdirAll(pathDir, 0o755); err != nil {
		t.Fatal(err)
	}
	pathBin := filepath.Join(pathDir, "vg")
	if err := os.WriteFile(pathBin, []byte("#!/bin/sh\necho vg-path\n"), 0o755); err != nil {
		t.Fatal(err)
	}

	envBin := filepath.Join(tempHome, "env-vg")
	if err := os.WriteFile(envBin, []byte("#!/bin/sh\necho vg-env\n"), 0o755); err != nil {
		t.Fatal(err)
	}

	ctx := context.Background()

	// 1. When ORQ_VG_PATH is set, it wins
	t.Setenv("ORQ_VG_PATH", envBin)
	t.Setenv("PATH", pathDir)
	check1 := checkVG(ctx, tempHome)
	if check1.Path != envBin {
		t.Fatalf("expected envBin %s to win, got %s", envBin, check1.Path)
	}

	// 2. When ORQ_VG_PATH is unset, PATH wins over known paths
	t.Setenv("ORQ_VG_PATH", "")
	t.Setenv("PATH", pathDir)
	check2 := checkVG(ctx, tempHome)
	if check2.Path != pathBin {
		t.Fatalf("expected pathBin %s to win, got %s", pathBin, check2.Path)
	}

	// 3. When PATH is unset, known paths win
	t.Setenv("ORQ_VG_PATH", "")
	t.Setenv("PATH", "")
	check3 := checkVG(ctx, tempHome)
	if check3.Path != knownBin {
		t.Fatalf("expected knownBin %s to win, got %s", knownBin, check3.Path)
	}
}
