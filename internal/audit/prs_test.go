package audit

import "testing"

func TestBlockersDetectReviewRequired(t *testing.T) {
	pr := PullRequest{Mergeable: "MERGEABLE", ReviewDecision: "REVIEW_REQUIRED", Checks: []StatusCheck{{Name: "go-test", Status: "COMPLETED", Conclusion: "SUCCESS"}}}
	got := blockers(pr)
	if len(got) != 1 || got[0] != "review requerida por identidad distinta con permisos" {
		t.Fatalf("unexpected blockers: %#v", got)
	}
}

func TestBlockersDetectFailingCheck(t *testing.T) {
	pr := PullRequest{Mergeable: "MERGEABLE", ReviewDecision: "APPROVED", Checks: []StatusCheck{{Name: "go-test", Status: "COMPLETED", Conclusion: "FAILURE"}}}
	got := blockers(pr)
	if len(got) != 1 || got[0] != "check no exitoso: go-test" {
		t.Fatalf("unexpected blockers: %#v", got)
	}
}

func TestParseChecks(t *testing.T) {
	raw := []byte(`[{"name":"go-test","workflowName":"Go test","status":"COMPLETED","conclusion":"SUCCESS","required":true}]`)
	got := parseChecks(raw)
	if len(got) != 1 {
		t.Fatalf("expected 1 check, got %d", len(got))
	}
	if got[0].Name != "go-test" || got[0].Workflow != "Go test" || !got[0].Required {
		t.Fatalf("unexpected check: %#v", got[0])
	}
}
