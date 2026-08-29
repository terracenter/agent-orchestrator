package audit

import "testing"

func TestAuditModelsMarksUnverifiedNotAssignable(t *testing.T) {
	report := AuditModels()
	var sawUnverified bool
	for _, model := range report.Models {
		if !model.Verified {
			sawUnverified = true
			if model.Assignable {
				t.Fatalf("unverified model should not be assignable: %+v", model)
			}
		}
		if model.ReviewOnly && model.Assignable {
			t.Fatalf("review-only model should not be assignable: %+v", model)
		}
	}
	if !sawUnverified || len(report.Findings) == 0 {
		t.Fatalf("expected unverified findings: %+v", report)
	}
}
