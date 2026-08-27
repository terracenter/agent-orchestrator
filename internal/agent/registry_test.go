package agent

import "testing"

func TestFindKnownProfile(t *testing.T) {
	profile, err := Find("pi", "gpt-5.5")
	if err != nil {
		t.Fatalf("Find() error = %v", err)
	}
	if profile.CostLevel != 1 {
		t.Fatalf("CostLevel = %d, want 1", profile.CostLevel)
	}
}

func TestFindUnknownProfile(t *testing.T) {
	if _, err := Find("pi", "unknown"); err == nil {
		t.Fatal("Find() error = nil, want error")
	}
}
