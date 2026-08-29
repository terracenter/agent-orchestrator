package agent

import (
	"os"
	"path/filepath"
	"testing"
)

func TestDetectAgentsWithHome(t *testing.T) {
	tempHome := t.TempDir()

	// Simulate openclaw and agy bin
	binDir := filepath.Join(tempHome, ".local", "bin")
	if err := os.MkdirAll(binDir, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(binDir, "openclaw"), []byte("#!/bin/sh\n"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(binDir, "agy"), []byte("#!/bin/sh\n"), 0o755); err != nil {
		t.Fatal(err)
	}

	// Simulate openclaw config dir
	cfgDir := filepath.Join(tempHome, ".openclaw")
	if err := os.MkdirAll(cfgDir, 0o700); err != nil {
		t.Fatal(err)
	}

	detections := DetectAgentsWithHome(tempHome)
	if len(detections) == 0 {
		t.Fatal("expected agent detections")
	}

	var foundOpenClaw, foundAGY bool
	for _, d := range detections {
		if d.Agent == "openclaw" {
			foundOpenClaw = true
			if !d.Installed {
				t.Fatalf("expected openclaw to be installed")
			}
			if d.ConfigPath != cfgDir {
				t.Fatalf("expected config path %s, got %s", cfgDir, d.ConfigPath)
			}
		}
		if d.Agent == "agy" {
			foundAGY = true
			if !d.Installed {
				t.Fatalf("expected agy to be installed")
			}
		}
	}

	if !foundOpenClaw || !foundAGY {
		t.Fatalf("expected to find openclaw and agy in detections")
	}
}

func TestDetectAgentsDefault(t *testing.T) {
	detections := DetectAgents()
	if len(detections) == 0 {
		t.Fatal("expected detections from default DetectAgents()")
	}
}
