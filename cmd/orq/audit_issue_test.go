package main

import (
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/terracenter/agent-orchestrator/internal/trace"
)

func TestCmdAuditIssueFromSession(t *testing.T) {
	tmp := t.TempDir()
	mgr := trace.NewManager(tmp)
	sess, err := mgr.Start(trace.TraceMetadata{Agent: "agy", Host: "local", Workspace: "/tmp", Metadata: map[string]string{"rtk_required": "true"}})
	if err != nil {
		t.Fatalf("start trace: %v", err)
	}
	if err := mgr.Record(sess.ID, trace.TraceEvent{EventType: trace.EventTypeCommand, Command: "git status", Timestamp: time.Now().UTC()}); err != nil {
		t.Fatalf("record trace: %v", err)
	}

	if err := cmdAudit([]string{"issue-from-session", "--path", tmp, "--session-id", sess.ID, "--format", "json"}); err != nil {
		t.Fatalf("cmd audit issue-from-session: %v", err)
	}
}

func TestCmdAuditIssueFromSessionFile(t *testing.T) {
	tmp := t.TempDir()
	file := filepath.Join(tmp, "events.jsonl")
	if err := os.WriteFile(file, []byte(`{"event_type":"command","command":"git status"}`+"\n"), 0o644); err != nil {
		t.Fatalf("write events: %v", err)
	}
	if err := cmdAudit([]string{"issue-from-session", "--file", file}); err != nil {
		t.Fatalf("cmd audit issue-from-session file: %v", err)
	}
}
