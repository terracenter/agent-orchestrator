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
