package agent

import (
	"testing"
	"time"
)

func TestCapabilitySnapshotsPreserveProfileIdentityAndEvidence(t *testing.T) {
	capturedAt := time.Date(2026, 8, 31, 16, 0, 0, 0, time.UTC)
	testProfiles := []Profile{
		{Agent: "pi", Provider: "openai", Model: "gpt-5.5", CostLevel: 2, UseFor: "orquestacion", Verified: true},
		{Agent: "qwen-code", Provider: "bailian", Model: "qwen3.8-max", CostLevel: 1, UseFor: "codigo", Verified: true},
	}

	snapshots := CapabilitySnapshots(testProfiles, capturedAt)
	if len(snapshots) != len(testProfiles) {
		t.Fatalf("expected %d snapshots, got %d", len(testProfiles), len(snapshots))
	}

	var foundQwen bool
	for _, snapshot := range snapshots {
		if snapshot.Agent == "" || snapshot.Provider == "" || snapshot.Model == "" {
			t.Fatalf("snapshot identity must be complete: %+v", snapshot)
		}
		if !snapshot.CapturedAt.Equal(capturedAt) {
			t.Fatalf("expected captured_at %s, got %s", capturedAt, snapshot.CapturedAt)
		}
		if len(snapshot.Evidence) == 0 {
			t.Fatalf("expected evidence for %+v", snapshot)
		}
		if snapshot.Evidence[0].Source != "config/agent-profiles.json" {
			t.Fatalf("expected evidence source config/agent-profiles.json, got %s", snapshot.Evidence[0].Source)
		}
		if snapshot.SecurityNote == "" {
			t.Fatalf("expected security note for %+v", snapshot)
		}
		if snapshot.Agent == "qwen-code" && snapshot.Model == "qwen3.8-max" {
			foundQwen = true
			if !snapshot.Verified {
				t.Fatalf("qwen3.8-max should remain verified")
			}
			if !contains(snapshot.Tools, "docker") {
				t.Fatalf("expected qwen-code tools to include docker: %v", snapshot.Tools)
			}
			if !hasEvidenceKind(snapshot.Evidence, "empirical") {
				t.Fatalf("expected qwen-code empirical evidence: %v", snapshot.Evidence)
			}
		}
	}
	if !foundQwen {
		t.Fatal("expected qwen-code/qwen3.8-max snapshot")
	}
}

func contains(values []string, needle string) bool {
	for _, value := range values {
		if value == needle {
			return true
		}
	}
	return false
}

func hasEvidenceKind(values []CapabilitySource, kind string) bool {
	for _, value := range values {
		if value.Kind == kind {
			return true
		}
	}
	return false
}
