package agent

import "testing"

func TestFindKnownProfile(t *testing.T) {
	profile, err := Find("pi", "gpt-5.5")
	if err != nil {
		t.Fatalf("Find() error = %v", err)
	}
	if profile.CostLevel != 2 || !profile.Verified || profile.Provider != "openai" {
		t.Fatalf("profile = %+v", profile)
	}
}

func TestFindUnknownProfile(t *testing.T) {
	if _, err := Find("pi", "unknown"); err == nil {
		t.Fatal("Find() error = nil, want error")
	}
}
