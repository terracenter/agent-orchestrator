package session

import "testing"

func TestValidatePassesCompleteSession(t *testing.T) {
	report := Validate(Input{RepoCheck: "OK", SafetyCheck: "OK", GuardCollision: "should_stop=false", Tests: "PASS", Receipt: "OK"})
	if !report.Valid || len(report.Findings) != 0 {
		t.Fatalf("expected valid report: %+v", report)
	}
}

func TestValidateBlocksMissingEvidence(t *testing.T) {
	report := Validate(Input{RepoCheck: "OK"})
	if report.Valid {
		t.Fatalf("expected invalid report: %+v", report)
	}
	if len(report.Findings) == 0 {
		t.Fatal("expected findings")
	}
}

func TestValidateDangerousRequiresHumanApproval(t *testing.T) {
	base := Input{RepoCheck: "OK", SafetyCheck: "OK", GuardCollision: "OK", Tests: "OK", Receipt: "OK", TouchesDangerous: true}
	if Validate(base).Valid {
		t.Fatal("expected dangerous session without approval to fail")
	}
	base.HumanApproval = true
	if !Validate(base).Valid {
		t.Fatal("expected dangerous session with explicit approval to pass")
	}
}
