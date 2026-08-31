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
	if err := os.WriteFile(filepath.Join(binDir, "qwen"), []byte("#!/bin/sh\n"), 0o755); err != nil {
		t.Fatal(err)
	}

	// Simulate config dirs without reading secret-bearing files.
	cfgDir := filepath.Join(tempHome, ".openclaw")
	if err := os.MkdirAll(cfgDir, 0o700); err != nil {
		t.Fatal(err)
	}
	qwenCfgDir := filepath.Join(tempHome, ".qwen")
	if err := os.MkdirAll(qwenCfgDir, 0o700); err != nil {
		t.Fatal(err)
	}

	detections := DetectAgentsWithHome(tempHome)
	if len(detections) == 0 {
		t.Fatal("expected agent detections")
	}

	var foundOpenClaw, foundAGY, foundQwen bool
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
		if d.Agent == "qwen-code" {
			foundQwen = true
			if !d.Installed {
				t.Fatalf("expected qwen-code to be installed")
			}
			if d.ConfigPath != qwenCfgDir {
				t.Fatalf("expected qwen config path %s, got %s", qwenCfgDir, d.ConfigPath)
			}
			if !d.Verified {
				t.Fatalf("expected qwen-code detection to be verified by runtime/config presence")
			}
		}
	}

	if !foundOpenClaw || !foundAGY || !foundQwen {
		t.Fatalf("expected to find openclaw, agy and qwen-code in detections")
	}
}

func TestDetectAgentsDefault(t *testing.T) {
	detections := DetectAgents()
	if len(detections) == 0 {
		t.Fatal("expected detections from default DetectAgents()")
	}
}
