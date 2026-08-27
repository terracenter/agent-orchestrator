package task

import "testing"

func TestTransitions(t *testing.T) {
	valid := [][2]State{{Planned, Assigned}, {Assigned, Running}, {Running, Done}, {Done, Verified}, {Verified, Merged}, {Blocked, Assigned}}
	for _, pair := range valid {
		if err := ValidateTransition(pair[0], pair[1]); err != nil {
			t.Fatalf("ValidateTransition(%q,%q) error = %v", pair[0], pair[1], err)
		}
	}
	if err := ValidateTransition(Planned, Merged); err == nil {
		t.Fatal("ValidateTransition(planned, merged) error = nil, want error")
	}
}
