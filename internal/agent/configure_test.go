package agent

import (
	"testing"
)

func TestConfigure_UnsupportedAgent(t *testing.T) {
	req := ConfigureRequest{
		Agent:   "qwen-code",
		DryRun:  false,
		AutoYes: true,
	}

	result, err := Configure(req)
	if err != nil {
		t.Fatalf("Configure returned error for unsupported agent: %v", err)
	}

	if result.Status != "unsupported" {
		t.Errorf("expected status=unsupported, got %s", result.Status)
	}

	if result.Notes == "" {
		t.Error("expected manual instructions in Notes for unsupported agent")
	}
}

func TestConfigure_DryRun(t *testing.T) {
	req := ConfigureRequest{
		Agent:   "openclaw",
		DryRun:  true,
		AutoYes: false,
	}

	result, err := Configure(req)
	if err != nil {
		t.Fatalf("Configure dry-run failed: %v", err)
	}

	if result.Status != "dry_run" {
		t.Errorf("expected status=dry_run, got %s", result.Status)
	}

	if !result.RTKRequired {
		t.Error("expected RTKRequired=true")
	}

	if len(result.Actions) == 0 {
		t.Error("expected actions list in dry-run mode")
	}
}

func TestConfigure_NeedsConfirmation(t *testing.T) {
	req := ConfigureRequest{
		Agent:   "agy",
		DryRun:  false,
		AutoYes: false,
	}

	result, err := Configure(req)
	if err != nil {
		t.Fatalf("Configure without --yes failed: %v", err)
	}

	if result.Status != "needs_confirmation" {
		t.Errorf("expected status=needs_confirmation, got %s", result.Status)
	}

	if len(result.Actions) == 0 {
		t.Error("expected actions list showing what would be done")
	}
}

func TestConfigure_Apply(t *testing.T) {
	t.Skip("Skipping test that requires mocking DetectAgents - test manually with real config")
}

func TestConfigure_WithBackup(t *testing.T) {
	t.Skip("Skipping test that requires mocking DetectAgents - test manually with real config")
}

func TestConfigureAll(t *testing.T) {
	req := ConfigureRequest{
		DryRun:  true,
		AutoYes: false,
	}

	results, err := ConfigureAll(req)
	if err != nil {
		t.Fatalf("ConfigureAll failed: %v", err)
	}

	if len(results) == 0 {
		t.Error("expected results for detected agents")
	}

	// Verificar que cada resultado tiene un agente
	for _, result := range results {
		if result.Agent == "" {
			t.Error("result should have agent name")
		}

		if result.Status == "" {
			t.Errorf("result for %s should have status", result.Agent)
		}
	}
}

func TestIsConfigurationSupported(t *testing.T) {
	tests := []struct {
		agent     string
		supported bool
	}{
		{"openclaw", true},
		{"agy", true},
		{"hermes", true},
		{"claude-code", true},
		{"qwen-code", false},
		{"pi", false},
		{"nvidia-api", false},
		{"codex", false},
		{"unknown", false},
	}

	for _, tt := range tests {
		result := isConfigurationSupported(tt.agent)
		if result != tt.supported {
			t.Errorf("isConfigurationSupported(%s) = %v, want %v", tt.agent, result, tt.supported)
		}
	}
}

func containsSubstring(s, substr string) bool {
	return len(s) >= len(substr) && (s == substr || len(s) > len(substr) && containsSubstringAt(s, substr))
}

func containsSubstringAt(s, substr string) bool {
	for i := 0; i <= len(s)-len(substr); i++ {
		if s[i:i+len(substr)] == substr {
			return true
		}
	}
	return false
}
