package agent

import (
	"testing"
	"time"
)

func TestCapabilitySnapshotsPreserveProfileIdentityAndEvidence(t *testing.T) {
	capturedAt := time.Date(2026, 8, 31, 16, 0, 0, 0, time.UTC)
	testProfiles := []Profile{
		{Agent: "pi", Provider: "openai", Model: "gpt-5.5", CostLevel: 2, UseFor: "orquestacion", Verified: true},
		{Agent: "hermes", Provider: "deepseek", Model: "deepseek-v4-pro", CostLevel: 1, UseFor: "codigo", Verified: true},
	}

	snapshots := CapabilitySnapshots(testProfiles, capturedAt)
	if len(snapshots) != len(testProfiles) {
		t.Fatalf("expected %d snapshots, got %d", len(testProfiles), len(snapshots))
	}

	var foundHermes bool
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
		if snapshot.Agent == "hermes" && snapshot.Model == "deepseek-v4-pro" {
			foundHermes = true
			if !snapshot.Verified {
				t.Fatalf("deepseek-v4-pro should remain verified")
			}
			if !contains(snapshot.Tools, "shell") {
				t.Fatalf("expected hermes tools to include shell: %v", snapshot.Tools)
			}
			if hasEvidenceKind(snapshot.Evidence, "empirical") {
				t.Fatalf("hermes must not carry empirical evidence from the registry alone: %v", snapshot.Evidence)
			}
		}
	}
	if !foundHermes {
		t.Fatal("expected hermes/deepseek-v4-pro snapshot")
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
