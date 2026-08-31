package trace

import (
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestStart(t *testing.T) {
	tmpdir := t.TempDir()
	m := NewManager(tmpdir)

	metadata := TraceMetadata{
		Agent:       "test-agent",
		Host:        "test-host",
		Workspace:   "/test/workspace",
		Model:       "test-model",
		Description: "test trace session",
	}

	session, err := m.Start(metadata)
	if err != nil {
		t.Fatalf("Start failed: %v", err)
	}

	if session.Agent != "test-agent" {
		t.Errorf("expected agent=test-agent, got %s", session.Agent)
	}
	if session.Status != "active" {
		t.Errorf("expected status=active, got %s", session.Status)
	}
	if session.ID == "" {
		t.Error("session ID is empty")
	}

	// Verificar archivo creado
	sessionPath := filepath.Join(tmpdir, session.ID+".session.json")
	if _, err := os.Stat(sessionPath); err != nil {
		t.Errorf("session file not created: %v", err)
	}
}

func TestRecord(t *testing.T) {
	tmpdir := t.TempDir()
	m := NewManager(tmpdir)

	metadata := TraceMetadata{
		Agent:     "test-agent",
		Host:      "test-host",
		Workspace: "/test/workspace",
	}

	session, err := m.Start(metadata)
	if err != nil {
		t.Fatalf("Start failed: %v", err)
	}

	event := TraceEvent{
		EventType:   EventTypeCommand,
		Command:     "go test ./...",
		CommandPath: "/test/workspace",
		Success:     true,
	}

	if err := m.Record(session.ID, event); err != nil {
		t.Fatalf("Record failed: %v", err)
	}

	// Verificar evento fue grabado
	eventsPath := filepath.Join(tmpdir, session.ID+".jsonl")
	if _, err := os.Stat(eventsPath); err != nil {
		t.Errorf("events file not created: %v", err)
	}
}

func TestStatus(t *testing.T) {
	tmpdir := t.TempDir()
	m := NewManager(tmpdir)

	metadata := TraceMetadata{
		Agent:     "test-agent",
		Host:      "test-host",
		Workspace: "/test/workspace",
	}

	session, err := m.Start(metadata)
	if err != nil {
		t.Fatalf("Start failed: %v", err)
	}

	// Grabar eventos
	for i := 0; i < 3; i++ {
		event := TraceEvent{
			EventType: EventTypeCommand,
			Command:   "echo test",
			Success:   true,
		}
		if err := m.Record(session.ID, event); err != nil {
			t.Fatalf("Record failed: %v", err)
		}
	}

	// Obtener status
	sess, events, err := m.Status(session.ID)
	if err != nil {
		t.Fatalf("Status failed: %v", err)
	}

	if sess.EventCount != 3 {
		t.Errorf("expected 3 events, got %d", sess.EventCount)
	}

	if len(events) != 3 {
		t.Errorf("expected 3 events in list, got %d", len(events))
	}
}

func TestStop(t *testing.T) {
	tmpdir := t.TempDir()
	m := NewManager(tmpdir)

	metadata := TraceMetadata{
		Agent:     "test-agent",
		Host:      "test-host",
		Workspace: "/test/workspace",
	}

	session, err := m.Start(metadata)
	if err != nil {
		t.Fatalf("Start failed: %v", err)
	}

	if err := m.Stop(session.ID); err != nil {
		t.Fatalf("Stop failed: %v", err)
	}

	// Verificar status cambió
	sess, _, err := m.Status(session.ID)
	if err != nil {
		t.Fatalf("Status failed: %v", err)
	}

	if sess.Status != "stopped" {
		t.Errorf("expected status=stopped, got %s", sess.Status)
	}

	if sess.StoppedAt == nil {
		t.Error("StoppedAt is nil")
	}
}

func TestList(t *testing.T) {
	tmpdir := t.TempDir()
	m := NewManager(tmpdir)

	// Crear 2 sesiones
	for i := 0; i < 2; i++ {
		metadata := TraceMetadata{
			Agent:     "test-agent",
			Host:      "test-host",
			Workspace: "/test/workspace",
		}
		if _, err := m.Start(metadata); err != nil {
			t.Fatalf("Start failed: %v", err)
		}
	}

	sessions, err := m.List()
	if err != nil {
		t.Fatalf("List failed: %v", err)
	}

	if len(sessions) != 2 {
		t.Errorf("expected 2 sessions, got %d", len(sessions))
	}
}

func TestEventTimestamp(t *testing.T) {
	tmpdir := t.TempDir()
	m := NewManager(tmpdir)

	metadata := TraceMetadata{
		Agent:     "test-agent",
		Host:      "test-host",
		Workspace: "/test/workspace",
	}

	session, err := m.Start(metadata)
	if err != nil {
		t.Fatalf("Start failed: %v", err)
	}

	before := time.Now().UTC()
	event := TraceEvent{
		EventType: EventTypeCommand,
		Command:   "echo test",
		Success:   true,
	}
	if err := m.Record(session.ID, event); err != nil {
		t.Fatalf("Record failed: %v", err)
	}
	after := time.Now().UTC()

	// Obtener evento grabado
	_, events, err := m.Status(session.ID)
	if err != nil {
		t.Fatalf("Status failed: %v", err)
	}

	if len(events) != 1 {
		t.Fatalf("expected 1 event, got %d", len(events))
	}

	ts := events[0].Timestamp
	if ts.Before(before) || ts.After(after) {
		t.Errorf("timestamp out of range: before=%v, ts=%v, after=%v", before, ts, after)
	}
}
