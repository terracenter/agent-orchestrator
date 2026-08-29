package mobile

import "testing"

func TestAuthorizedStatusIsReadOnly(t *testing.T) {
	r := NewRouter([]int64{42})
	resp := r.Handle(Message{UserID: 42, Text: "/status"})
	if resp.Rejected || !resp.ReadOnly || resp.RequiresConfirmation {
		t.Fatalf("unexpected response: %+v", resp)
	}
	if resp.Audit.Result != "allowed_read_only" || !resp.Audit.Authorized {
		t.Fatalf("missing audit: %+v", resp.Audit)
	}
}

func TestUnauthorizedUserRejectedAndAudited(t *testing.T) {
	r := NewRouter([]int64{42})
	resp := r.Handle(Message{UserID: 7, Text: "/status"})
	if !resp.Rejected || resp.Audit.Result != "rejected_unauthorized" || resp.Audit.Authorized {
		t.Fatalf("expected unauthorized rejection audit, got %+v", resp)
	}
}

func TestWriteCommandRequiresConfirmation(t *testing.T) {
	r := NewRouter([]int64{42})
	resp := r.Handle(Message{UserID: 42, Text: "/task_create ordenar docs"})
	if !resp.RequiresConfirmation || resp.Rejected || !resp.Audit.RequiresWrite {
		t.Fatalf("expected confirmation gate, got %+v", resp)
	}
	if resp.Audit.ModelAssigned == "" || resp.Audit.Result != "confirmation_required" {
		t.Fatalf("missing routing audit: %+v", resp.Audit)
	}
}

func TestUnknownCommandRejected(t *testing.T) {
	r := NewRouter([]int64{42})
	resp := r.Handle(Message{UserID: 42, Text: "/shell rm -rf"})
	if !resp.Rejected || resp.Audit.Result != "rejected_unknown_command" {
		t.Fatalf("expected safe rejection, got %+v", resp)
	}
}
