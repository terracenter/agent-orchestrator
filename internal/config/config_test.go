package config

import (
	"strings"
	"testing"
)

func TestDecodeConfigExample(t *testing.T) {
	input := `[project]
name = "agent-orchestrator"

[ssot]
type = "local-dir"
path = "./docs"

[graph]
type = "vg"

[memory]
type = "jsonl"
path = "~/.local/state/orq/ledger.jsonl"

[shell]
type = "rtk"

[policy]
default_mode = "dry-run"
require_confirmation_for_security = true
`
	cfg, err := Decode(strings.NewReader(input))
	if err != nil {
		t.Fatalf("Decode() error = %v", err)
	}
	if cfg.Project.Name != "agent-orchestrator" {
		t.Fatalf("Project.Name = %q", cfg.Project.Name)
	}
	if cfg.Graph.Type != "vg" || cfg.Shell.Type != "rtk" {
		t.Fatalf("adapters = graph:%q shell:%q", cfg.Graph.Type, cfg.Shell.Type)
	}
	if !cfg.Policy.RequireConfirmationForSecurity {
		t.Fatal("RequireConfirmationForSecurity = false")
	}
}

func TestDecodeDefaults(t *testing.T) {
	cfg, err := Decode(strings.NewReader(`[project]
name = "custom"
`))
	if err != nil {
		t.Fatalf("Decode() error = %v", err)
	}
	if cfg.Project.Name != "custom" {
		t.Fatalf("Project.Name = %q", cfg.Project.Name)
	}
	if cfg.Graph.Type != "noop" || cfg.Policy.DefaultMode != "dry-run" {
		t.Fatalf("defaults not preserved: %#v", cfg)
	}
}

func TestDecodeRejectsUnknownSetting(t *testing.T) {
	_, err := Decode(strings.NewReader(`[graph]
unknown = "x"
`))
	if err == nil {
		t.Fatal("Decode() error = nil, want error")
	}
}
