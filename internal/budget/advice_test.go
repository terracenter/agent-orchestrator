package budget

import "testing"

func TestDecidePausesWhenCodexLimitIsAlmostExhausted(t *testing.T) {
	advice := Decide(10, 99, 10)
	if advice.Action != "pausar" {
		t.Fatalf("Action = %q, want pausar", advice.Action)
	}
}

func TestDecideCompactsWhenContextIsHigh(t *testing.T) {
	advice := Decide(70, 10, 10)
	if advice.Action != "compactar" {
		t.Fatalf("Action = %q, want compactar", advice.Action)
	}
	if advice.CompactPrompt == "" {
		t.Fatal("CompactPrompt empty")
	}
}

func TestDecideDelegatesWhenBudgetIsTight(t *testing.T) {
	advice := Decide(20, 85, 10)
	if advice.Action != "delegar_barato" {
		t.Fatalf("Action = %q, want delegar_barato", advice.Action)
	}
	if len(advice.UseAgents) == 0 || advice.UseAgents[0] != "nvidia-api/openai/gpt-oss-20b" {
		t.Fatalf("UseAgents = %+v", advice.UseAgents)
	}
}

func TestValidatePercent(t *testing.T) {
	if err := ValidatePercent("context", 101); err == nil {
		t.Fatal("expected error")
	}
	if err := ValidatePercent("context", 50); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
}
