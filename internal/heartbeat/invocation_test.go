package heartbeat

import (
	"context"
	"errors"
	"sync"
	"testing"
	"time"
)

func TestRunInvocationSuccess(t *testing.T) {
	cfg := InvocationConfig{
		Timeout:           1 * time.Second,
		HeartbeatInterval: 100 * time.Millisecond,
	}

	res := RunInvocation(context.Background(), cfg, "clasificar tarea", "pi", "gpt-5.5", func(ctx context.Context) error {
		time.Sleep(10 * time.Millisecond)
		return nil
	})

	if res.Status != StatusOk {
		t.Fatalf("Status = %s, want %s", res.Status, StatusOk)
	}
	if res.Task != "clasificar tarea" || res.Agent != "pi" || res.Model != "gpt-5.5" {
		t.Fatalf("unexpected invocation metadata: %+v", res)
	}
	if res.StartedAt.IsZero() || res.FinishedAt.IsZero() {
		t.Fatalf("timestamps are zero: started=%v finished=%v", res.StartedAt, res.FinishedAt)
	}
	if res.DurationMs < 0 {
		t.Fatalf("DurationMs = %d, want >= 0", res.DurationMs)
	}

	ev := res.ToLedgerEvent()
	if ev.Task != res.Task || ev.Agent != res.Agent || ev.Status != StatusOk {
		t.Fatalf("unexpected ledger event: %+v", ev)
	}
}

func TestRunInvocationTimeout(t *testing.T) {
	cfg := InvocationConfig{
		Timeout:           30 * time.Millisecond,
		HeartbeatInterval: 10 * time.Millisecond,
	}

	res := RunInvocation(context.Background(), cfg, "tarea lenta", "claude-code", "claude-sonnet-4-5-20250929", func(ctx context.Context) error {
		select {
		case <-time.After(200 * time.Millisecond):
			return nil
		case <-ctx.Done():
			return ctx.Err()
		}
	})

	if res.Status != StatusTimeout {
		t.Fatalf("Status = %s, want %s", res.Status, StatusTimeout)
	}
	if res.Error == "" {
		t.Fatal("expected error message on timeout, got empty string")
	}
}

func TestRunInvocationHeartbeatPulses(t *testing.T) {
	var pulses []HeartbeatPulse
	var mu sync.Mutex

	cfg := InvocationConfig{
		Timeout:           500 * time.Millisecond,
		HeartbeatInterval: 25 * time.Millisecond,
		OnHeartbeat: func(pulse HeartbeatPulse) {
			mu.Lock()
			pulses = append(pulses, pulse)
			mu.Unlock()
		},
	}

	res := RunInvocation(context.Background(), cfg, "tarea monitoreada", "agy", "gemini-3.7-flash-high", func(ctx context.Context) error {
		time.Sleep(80 * time.Millisecond)
		return nil
	})

	if res.Status != StatusOk {
		t.Fatalf("Status = %s, want %s", res.Status, StatusOk)
	}

	mu.Lock()
	count := len(pulses)
	mu.Unlock()

	if count == 0 {
		t.Fatalf("expected at least 1 heartbeat pulse, got %d", count)
	}
}

func TestRunInvocationFallbackSuccess(t *testing.T) {
	cfg := InvocationConfig{
		Timeout:           50 * time.Millisecond,
		HeartbeatInterval: 10 * time.Millisecond,
		FallbackAgent:     "pi",
		FallbackModel:     "gpt-5.5",
		FallbackExecFn: func(ctx context.Context) error {
			time.Sleep(5 * time.Millisecond)
			return nil
		},
	}

	res := RunInvocation(context.Background(), cfg, "tarea con fallback", "agy", "gemini-3.7-flash-high", func(ctx context.Context) error {
		return errors.New("primary agent unavailable")
	})

	if !res.FallbackUsed {
		t.Fatal("expected FallbackUsed to be true")
	}
	if res.Status != StatusFallbackOk {
		t.Fatalf("Status = %s, want %s", res.Status, StatusFallbackOk)
	}
	if res.FallbackAgent != "pi" || res.FallbackModel != "gpt-5.5" {
		t.Fatalf("fallback info mismatch: %s/%s", res.FallbackAgent, res.FallbackModel)
	}
}

func TestRunInvocationFallbackFailure(t *testing.T) {
	cfg := InvocationConfig{
		Timeout:           50 * time.Millisecond,
		HeartbeatInterval: 10 * time.Millisecond,
		FallbackAgent:     "pi",
		FallbackModel:     "gpt-5.5",
		FallbackExecFn: func(ctx context.Context) error {
			return errors.New("fallback also failed")
		},
	}

	res := RunInvocation(context.Background(), cfg, "tarea con fallback fallido", "agy", "gemini-3.7-flash-high", func(ctx context.Context) error {
		return errors.New("primary failed")
	})

	if !res.FallbackUsed {
		t.Fatal("expected FallbackUsed to be true")
	}
	if res.Status != StatusFallbackFailed {
		t.Fatalf("Status = %s, want %s", res.Status, StatusFallbackFailed)
	}
	if res.FallbackError == "" {
		t.Fatal("expected FallbackError to be set")
	}
}
