package heartbeat

import (
	"context"
	"errors"
	"fmt"
	"sync"
	"time"

	"github.com/terracenter/agent-orchestrator/internal/ledger"
)

const (
	DefaultInvocationTimeout = 10 * time.Minute
	DefaultHeartbeatInterval = 30 * time.Second
	StatusOk                 = "ok"
	StatusTimeout            = "timeout"
	StatusFailed             = "failed"
	StatusFallbackOk         = "fallback_ok"
	StatusFallbackFailed     = "fallback_failed"
)

// HeartbeatPulse represents a periodic heartbeat notification during an invocation.
type HeartbeatPulse struct {
	InvocationID string        `json:"invocation_id"`
	Task         string        `json:"task"`
	Agent        string        `json:"agent"`
	Model        string        `json:"model"`
	Elapsed      time.Duration `json:"elapsed"`
	PulseNumber  int           `json:"pulse_number"`
	Timestamp    time.Time     `json:"timestamp"`
}

// InvocationConfig defines timeout, heartbeat rhythm, and fallback policy for an agent run.
type InvocationConfig struct {
	Timeout           time.Duration
	HeartbeatInterval time.Duration
	OnHeartbeat       func(pulse HeartbeatPulse)
	FallbackAgent     string
	FallbackModel     string
	FallbackExecFn    func(ctx context.Context) error
}

// InvocationResult captures the full execution lifecycle of an agent invocation.
type InvocationResult struct {
	Task            string    `json:"task"`
	Agent           string    `json:"agent"`
	Model           string    `json:"model"`
	Status          string    `json:"status"`
	StartedAt       time.Time `json:"started_at"`
	FinishedAt      time.Time `json:"finished_at"`
	DurationMs      int64     `json:"duration_ms"`
	HeartbeatsCount int       `json:"heartbeats_count"`
	Error           string    `json:"error,omitempty"`
	FallbackUsed    bool      `json:"fallback_used"`
	FallbackAgent   string    `json:"fallback_agent,omitempty"`
	FallbackModel   string    `json:"fallback_model,omitempty"`
	FallbackError   string    `json:"fallback_error,omitempty"`
}

// ToLedgerEvent converts an InvocationResult into a ledger.Event ready for persistence.
func (r InvocationResult) ToLedgerEvent() ledger.Event {
	ev := ledger.NewInvocationEvent(r.Task, r.Agent, r.Model, r.Status, r.StartedAt, r.FinishedAt, r.FallbackAgent, r.FallbackModel)
	if r.Error != "" {
		ev.Notes = r.Error
	}
	return ev
}

// RunInvocation executes an agent action under timeout and heartbeat supervision with optional fallback.
func RunInvocation(ctx context.Context, cfg InvocationConfig, task, agent, model string, execFn func(ctx context.Context) error) InvocationResult {
	startedAt := time.Now().UTC()
	timeout := cfg.Timeout
	if timeout <= 0 {
		timeout = DefaultInvocationTimeout
	}
	interval := cfg.HeartbeatInterval
	if interval <= 0 {
		interval = DefaultHeartbeatInterval
	}

	invCtx, cancel := context.WithTimeout(ctx, timeout)
	defer cancel()

	invocationID := fmt.Sprintf("%s-%d", agent, startedAt.UnixNano())
	pulseCount := 0
	var pulseMu sync.Mutex

	stopHeartbeat := make(chan struct{})
	ticker := time.NewTicker(interval)
	go func() {
		defer ticker.Stop()
		for {
			select {
			case <-ticker.C:
				pulseMu.Lock()
				pulseCount++
				currentCount := pulseCount
				pulseMu.Unlock()

				pulse := HeartbeatPulse{
					InvocationID: invocationID,
					Task:         task,
					Agent:        agent,
					Model:        model,
					Elapsed:      time.Since(startedAt),
					PulseNumber:  currentCount,
					Timestamp:    time.Now().UTC(),
				}
				if cfg.OnHeartbeat != nil {
					cfg.OnHeartbeat(pulse)
				}
			case <-stopHeartbeat:
				return
			case <-invCtx.Done():
				return
			}
		}
	}()

	err := execFn(invCtx)
	close(stopHeartbeat)
	finishedAt := time.Now().UTC()
	durationMs := finishedAt.Sub(startedAt).Milliseconds()
	if durationMs < 0 {
		durationMs = 0
	}

	pulseMu.Lock()
	finalPulses := pulseCount
	pulseMu.Unlock()

	res := InvocationResult{
		Task:            task,
		Agent:           agent,
		Model:           model,
		StartedAt:       startedAt,
		FinishedAt:      finishedAt,
		DurationMs:      durationMs,
		HeartbeatsCount: finalPulses,
		FallbackAgent:   cfg.FallbackAgent,
		FallbackModel:   cfg.FallbackModel,
	}

	if err == nil {
		res.Status = StatusOk
		return res
	}

	if errors.Is(invCtx.Err(), context.DeadlineExceeded) || errors.Is(err, context.DeadlineExceeded) {
		res.Status = StatusTimeout
		res.Error = fmt.Sprintf("invocation timed out after %v", timeout)
	} else {
		res.Status = StatusFailed
		res.Error = err.Error()
	}

	// Trigger fallback if provided and primary execution failed/timed out
	if cfg.FallbackExecFn != nil {
		res.FallbackUsed = true
		fallbackCtx, fallbackCancel := context.WithTimeout(ctx, timeout)
		defer fallbackCancel()

		fbErr := cfg.FallbackExecFn(fallbackCtx)
		fallbackFinished := time.Now().UTC()
		res.FinishedAt = fallbackFinished
		res.DurationMs = fallbackFinished.Sub(startedAt).Milliseconds()

		if fbErr == nil {
			res.Status = StatusFallbackOk
		} else {
			res.Status = StatusFallbackFailed
			res.FallbackError = fbErr.Error()
		}
	}

	return res
}
