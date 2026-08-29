package budget

import "testing"

func TestDecidePausesWhenCodexLimitIsAlmostExhausted(t *testing.T) {
	advice := Decide(10, 99, 10)
	if advice.Action != "pausar" {
		t.Fatalf("Action = %q, want pausar", advice.Action)
	}
}

func TestDecideAlwaysRequiresPreflightCompact(t *testing.T) {
	advice := Decide(10, 10, 10)
	if !advice.PreflightCompactRequired {
		t.Fatal("PreflightCompactRequired = false, want true")
	}
	if advice.CompactPrompt == "" {
		t.Fatal("CompactPrompt empty")
	}
	if advice.CompactInstruction == "" {
		t.Fatal("CompactInstruction empty")
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

func TestDecideForPiUnderTightBudgetRequiresDelegationStop(t *testing.T) {
	advice := DecideForAgentWithCompactApplied(20, 85, 10, "pi", false)
	if !advice.MustStopForDelegation {
		t.Fatal("MustStopForDelegation = false, want true")
	}
	if !advice.SupervisorOnly {
		t.Fatal("SupervisorOnly = false, want true")
	}
	if advice.ExecutionAgentAllowed {
		t.Fatal("ExecutionAgentAllowed = true, want false")
	}

	// When compact is already applied, Pi action is delegar_barato and must still stop
	adviceApplied := DecideForAgentWithCompactApplied(20, 85, 10, "pi", true)
	if adviceApplied.Action != "delegar_barato" {
		t.Fatalf("Action = %q, want delegar_barato", adviceApplied.Action)
	}
	if !adviceApplied.MustStopForDelegation {
		t.Fatal("MustStopForDelegation = false, want true when delegating")
	}
	if !adviceApplied.SupervisorOnly {
		t.Fatal("SupervisorOnly = false, want true")
	}
	if adviceApplied.ExecutionAgentAllowed {
		t.Fatal("ExecutionAgentAllowed = true, want false")
	}
}

func TestDecideForNonPiAllowsExecutionAgent(t *testing.T) {
	advice := DecideForAgentWithCompactApplied(20, 85, 10, "agy", true)
	if advice.Action != "delegar_barato" {
		t.Fatalf("Action = %q, want delegar_barato", advice.Action)
	}
	if advice.MustStopForDelegation {
		t.Fatal("MustStopForDelegation = true, want false for agy")
	}
	if advice.SupervisorOnly {
		t.Fatal("SupervisorOnly = true, want false for agy")
	}
	if !advice.ExecutionAgentAllowed {
		t.Fatal("ExecutionAgentAllowed = false, want true for agy")
	}
}
