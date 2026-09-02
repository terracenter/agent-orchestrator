package agent

import (
	"os"
	"path/filepath"
	"testing"
)

func TestLoadProfilesExplicitPath(t *testing.T) {
	tempDir := t.TempDir()
	jsonFile := filepath.Join(tempDir, "profiles.json")
	content := `[
		{
			"agent": "pi",
			"provider": "openai",
			"model": "gpt-5.5",
			"cost_level": 2,
			"use_for": "orquestacion principal",
			"verified": true
		}
	]`
	if err := os.WriteFile(jsonFile, []byte(content), 0644); err != nil {
		t.Fatalf("failed to write temp file: %v", err)
	}

	profiles, err := LoadProfiles(jsonFile)
	if err != nil {
		t.Fatalf("LoadProfiles() error = %v", err)
	}
	if len(profiles) != 1 {
		t.Fatalf("expected 1 profile, got %d", len(profiles))
	}
	if profiles[0].Agent != "pi" || profiles[0].Model != "gpt-5.5" {
		t.Fatalf("unexpected profile: %+v", profiles[0])
	}
}

func TestLoadProfilesEnvPriority(t *testing.T) {
	tempDir := t.TempDir()
	jsonFile := filepath.Join(tempDir, "env_profiles.json")
	content := `[
		{
			"agent": "agy",
			"provider": "google",
			"model": "gemini-3.5-flash-low",
			"cost_level": 1,
			"use_for": "tareas mecanicas",
			"verified": true
		}
	]`
	if err := os.WriteFile(jsonFile, []byte(content), 0644); err != nil {
		t.Fatalf("failed to write temp file: %v", err)
	}

	t.Setenv(ProfilesEnv, jsonFile)

	profiles, err := LoadProfiles()
	if err != nil {
		t.Fatalf("LoadProfiles() with env error = %v", err)
	}
	if len(profiles) != 1 || profiles[0].Agent != "agy" {
		t.Fatalf("unexpected profiles from env: %+v", profiles)
	}
}

func TestLoadProfilesInvalid(t *testing.T) {
	tempDir := t.TempDir()

	// 1. Missing file
	if _, err := LoadProfiles(filepath.Join(tempDir, "nonexistent.json")); err == nil {
		t.Fatal("expected error for missing file, got nil")
	}

	// 2. Malformed JSON
	badJSON := filepath.Join(tempDir, "bad.json")
	_ = os.WriteFile(badJSON, []byte("{invalid json}"), 0644)
	if _, err := LoadProfiles(badJSON); err == nil {
		t.Fatal("expected error for malformed JSON, got nil")
	}

	// 3. Validation failure (cost_level < 0)
	invalidProfile := filepath.Join(tempDir, "invalid.json")
	invalidContent := `[
		{
			"agent": "pi",
			"provider": "openai",
			"model": "gpt-5.5",
			"cost_level": -1,
			"use_for": "test"
		}
	]`
	_ = os.WriteFile(invalidProfile, []byte(invalidContent), 0644)
	if _, err := LoadProfiles(invalidProfile); err == nil {
		t.Fatal("expected error for invalid profile cost_level, got nil")
	}
}

func TestFindKnownProfile(t *testing.T) {
	profiles := []Profile{
		{Agent: "pi", Provider: "openai", Model: "gpt-5.5", CostLevel: 2, UseFor: "orquestacion", Verified: true},
	}
	profile, err := Find(profiles, "pi", "gpt-5.5")
	if err != nil {
		t.Fatalf("Find() error = %v", err)
	}
	if profile.CostLevel != 2 || !profile.Verified || profile.Provider != "openai" {
		t.Fatalf("profile = %+v", profile)
	}
}

func TestFindUnknownProfile(t *testing.T) {
	profiles := []Profile{
		{Agent: "pi", Provider: "openai", Model: "gpt-5.5", CostLevel: 2, UseFor: "orquestacion", Verified: true},
	}
	if _, err := Find(profiles, "pi", "unknown"); err == nil {
		t.Fatal("Find() error = nil, want error")
	}
}
