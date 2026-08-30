package ledger

import (
	"path/filepath"
	"testing"
	"time"
)

func TestAppendAndReadAll(t *testing.T) {
	path := filepath.Join(t.TempDir(), "ledger.jsonl")
	event := Event{Task: "test", Agent: "pi", Model: "gpt-5.5", Status: "ok"}
	if err := Append(path, event); err != nil {
		t.Fatalf("Append() error = %v", err)
	}
	events, err := ReadAll(path)
	if err != nil {
		t.Fatalf("ReadAll() error = %v", err)
	}
	if len(events) != 1 {
		t.Fatalf("len(events) = %d, want 1", len(events))
	}
	got := events[0]
	if got.Agent != "pi" || got.Model != "gpt-5.5" || got.Status != "ok" {
		t.Fatalf("event = %+v", got)
	}
}

func TestAppendRequiresFields(t *testing.T) {
	path := filepath.Join(t.TempDir(), "ledger.jsonl")
	if err := Append(path, Event{}); err == nil {
		t.Fatal("Append() error = nil, want error")
	}
}

func TestAppendWithDurationAndTimestamps(t *testing.T) {
	path := filepath.Join(t.TempDir(), "ledger.jsonl")
	started := time.Date(2026, 8, 30, 10, 0, 0, 0, time.UTC)
	finished := time.Date(2026, 8, 30, 10, 0, 15, 500000000, time.UTC)

	event := NewInvocationEvent("refactor auth", "agy", "gemini-3.7-flash-high", "ok", started, finished, "pi", "gpt-5.5")
	event.TokensIn = 1200
	event.TokensOut = 450
	event.Notes = "ejecucion exitosa"

	if err := Append(path, event); err != nil {
		t.Fatalf("Append() error = %v", err)
	}

	events, err := ReadAll(path)
	if err != nil {
		t.Fatalf("ReadAll() error = %v", err)
	}
	if len(events) != 1 {
		t.Fatalf("len(events) = %d, want 1", len(events))
	}
	got := events[0]
	if got.Task != "refactor auth" || got.Agent != "agy" || got.Model != "gemini-3.7-flash-high" || got.Status != "ok" {
		t.Fatalf("unexpected event base fields: %+v", got)
	}
	if got.StartedAt == nil || !got.StartedAt.Equal(started) {
		t.Fatalf("StartedAt = %v, want %v", got.StartedAt, started)
	}
	if got.FinishedAt == nil || !got.FinishedAt.Equal(finished) {
		t.Fatalf("FinishedAt = %v, want %v", got.FinishedAt, finished)
	}
	if got.DurationMs != 15500 {
		t.Fatalf("DurationMs = %d, want 15500", got.DurationMs)
	}
	if got.FallbackAgent != "pi" || got.FallbackModel != "gpt-5.5" {
		t.Fatalf("fallback fields = %s/%s, want pi/gpt-5.5", got.FallbackAgent, got.FallbackModel)
	}
	if got.TokensIn != 1200 || got.TokensOut != 450 {
		t.Fatalf("tokens in/out = %d/%d, want 1200/450", got.TokensIn, got.TokensOut)
	}
}

func TestAppendCalculateDurationWhenOmitted(t *testing.T) {
	path := filepath.Join(t.TempDir(), "ledger.jsonl")
	started := time.Now().Add(-5 * time.Second)
	finished := time.Now()

	event := Event{
		Task:       "fix bug",
		Agent:      "claude-code",
		Model:      "claude-sonnet-4-5-20250929",
		Status:     "ok",
		StartedAt:  &started,
		FinishedAt: &finished,
	}

	if err := Append(path, event); err != nil {
		t.Fatalf("Append() error = %v", err)
	}

	events, err := ReadAll(path)
	if err != nil {
		t.Fatalf("ReadAll() error = %v", err)
	}
	if len(events) != 1 {
		t.Fatalf("len(events) = %d, want 1", len(events))
	}
	if events[0].DurationMs <= 0 {
		t.Fatalf("DurationMs = %d, want > 0", events[0].DurationMs)
	}
}
