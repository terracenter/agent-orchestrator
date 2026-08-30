package ledger

import (
	"bufio"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"time"
)

// Event is an append-only fact produced by the orchestrator.
type Event struct {
	Timestamp     time.Time  `json:"ts"`
	Task          string     `json:"task"`
	Agent         string     `json:"agent"`
	Model         string     `json:"model"`
	Status        string     `json:"status"`
	StartedAt     *time.Time `json:"started_at,omitempty"`
	FinishedAt    *time.Time `json:"finished_at,omitempty"`
	DurationMs    int64      `json:"duration_ms,omitempty"`
	FallbackAgent string     `json:"fallback_agent,omitempty"`
	FallbackModel string     `json:"fallback_model,omitempty"`
	TokensIn      int64      `json:"tokens_in,omitempty"`
	TokensOut     int64      `json:"tokens_out,omitempty"`
	Notes         string     `json:"notes,omitempty"`
}

// NewInvocationEvent creates an Event recording the start, finish, duration, and status of an agent invocation.
func NewInvocationEvent(task, agent, model, status string, started, finished time.Time, fallbackAgent, fallbackModel string) Event {
	var durationMs int64
	if !started.IsZero() && !finished.IsZero() {
		durationMs = finished.Sub(started).Milliseconds()
		if durationMs < 0 {
			durationMs = 0
		}
	}
	ts := finished
	if ts.IsZero() {
		ts = started
	}
	if ts.IsZero() {
		ts = time.Now().UTC()
	}

	ev := Event{
		Timestamp:     ts,
		Task:          task,
		Agent:         agent,
		Model:         model,
		Status:        status,
		DurationMs:    durationMs,
		FallbackAgent: fallbackAgent,
		FallbackModel: fallbackModel,
	}
	if !started.IsZero() {
		ev.StartedAt = &started
	}
	if !finished.IsZero() {
		ev.FinishedAt = &finished
	}
	return ev
}

func DefaultPath() string {
	state := os.Getenv("XDG_STATE_HOME")
	if state == "" {
		home, err := os.UserHomeDir()
		if err != nil || home == "" {
			return filepath.Join(".", "orq", "ledger.jsonl")
		}
		state = filepath.Join(home, ".local", "state")
	}
	return filepath.Join(state, "orq", "ledger.jsonl")
}

func Append(path string, event Event) error {
	if event.StartedAt != nil && event.FinishedAt != nil && event.DurationMs == 0 {
		diff := event.FinishedAt.Sub(*event.StartedAt).Milliseconds()
		if diff >= 0 {
			event.DurationMs = diff
		}
	}
	if event.Timestamp.IsZero() {
		if event.FinishedAt != nil && !event.FinishedAt.IsZero() {
			event.Timestamp = *event.FinishedAt
		} else if event.StartedAt != nil && !event.StartedAt.IsZero() {
			event.Timestamp = *event.StartedAt
		} else {
			event.Timestamp = time.Now().UTC()
		}
	}
	if event.Task == "" {
		return errors.New("task is required")
	}
	if event.Agent == "" {
		return errors.New("agent is required")
	}
	if event.Model == "" {
		return errors.New("model is required")
	}
	if event.Status == "" {
		return errors.New("status is required")
	}
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return fmt.Errorf("create ledger dir: %w", err)
	}
	file, err := os.OpenFile(path, os.O_CREATE|os.O_WRONLY|os.O_APPEND, 0o644)
	if err != nil {
		return fmt.Errorf("open ledger: %w", err)
	}
	defer file.Close()

	line, err := json.Marshal(event)
	if err != nil {
		return fmt.Errorf("marshal event: %w", err)
	}
	if _, err := file.Write(append(line, '\n')); err != nil {
		return fmt.Errorf("write event: %w", err)
	}
	return nil
}

func ReadAll(path string) ([]Event, error) {
	file, err := os.Open(path)
	if errors.Is(err, os.ErrNotExist) {
		return nil, nil
	}
	if err != nil {
		return nil, fmt.Errorf("open ledger: %w", err)
	}
	defer file.Close()

	var events []Event
	scanner := bufio.NewScanner(file)
	for scanner.Scan() {
		var event Event
		if err := json.Unmarshal(scanner.Bytes(), &event); err != nil {
			return nil, fmt.Errorf("decode ledger line %d: %w", len(events)+1, err)
		}
		events = append(events, event)
	}
	if err := scanner.Err(); err != nil {
		return nil, fmt.Errorf("scan ledger: %w", err)
	}
	return events, nil
}
