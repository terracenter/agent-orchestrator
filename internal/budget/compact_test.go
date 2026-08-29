package budget

import "testing"

func TestCompactCapabilityForPiRequiresUserInstruction(t *testing.T) {
	capability := CompactCapabilityFor("pi")
	if capability.CanAuto {
		t.Fatal("CanAuto = true, want false")
	}
	if capability.Mode != "manual_user_required" {
		t.Fatalf("Mode = %q, want manual_user_required", capability.Mode)
	}
	if capability.Instruction == "" {
		t.Fatal("Instruction empty")
	}
}

func TestDecideForAgentIncludesCompactInstruction(t *testing.T) {
	advice := DecideForAgent(10, 10, 0, "pi")
	if advice.CompactCapability.Agent != "pi" {
		t.Fatalf("Agent = %q, want pi", advice.CompactCapability.Agent)
	}
	if advice.CompactInstruction == "" {
		t.Fatal("CompactInstruction empty")
	}
}
