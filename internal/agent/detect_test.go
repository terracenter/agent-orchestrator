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
	if err := os.WriteFile(filepath.Join(binDir, "hermes"), []byte("#!/bin/sh\n"), 0o755); err != nil {
		t.Fatal(err)
	}

	// Simulate config dirs without reading secret-bearing files.
	cfgDir := filepath.Join(tempHome, ".openclaw")
	if err := os.MkdirAll(cfgDir, 0o700); err != nil {
		t.Fatal(err)
	}
	hermesCfgDir := filepath.Join(tempHome, ".hermes")
	if err := os.MkdirAll(hermesCfgDir, 0o700); err != nil {
		t.Fatal(err)
	}

	detections := DetectAgentsWithHome(tempHome)
	if len(detections) == 0 {
		t.Fatal("expected agent detections")
	}

	var foundOpenClaw, foundAGY, foundHermes bool
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
		if d.Agent == "hermes" {
			foundHermes = true
			if !d.Installed {
				t.Fatalf("expected hermes to be installed")
			}
			if d.ConfigPath != hermesCfgDir {
				t.Fatalf("expected hermes config path %s, got %s", hermesCfgDir, d.ConfigPath)
			}
			if !d.Verified {
				t.Fatalf("expected hermes detection to be verified by runtime/config presence")
			}
		}
	}

	if !foundOpenClaw || !foundAGY || !foundHermes {
		t.Fatalf("expected to find openclaw, agy and hermes in detections")
	}
}

func TestDetectAgentsDefault(t *testing.T) {
	detections := DetectAgents()
	if len(detections) == 0 {
		t.Fatal("expected detections from default DetectAgents()")
	}
}
