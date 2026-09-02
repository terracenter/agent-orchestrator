package main

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/terracenter/agent-orchestrator/internal/ledger"
)

func TestObserverEventFromLedgerHasStableID(t *testing.T) {
	event := ledger.Event{Timestamp: time.Date(2026, 8, 30, 12, 0, 0, 0, time.UTC), Task: "sync", Agent: "pi", Model: "gpt-5.5-minimal", Status: "completed"}
	first := observerEventFromLedger(event)
	second := observerEventFromLedger(event)
	if first.EventID == "" || first.EventID != second.EventID {
		t.Fatalf("event ID is not stable: first=%q second=%q", first.EventID, second.EventID)
	}
	if first.Agent != event.Agent || first.Model != event.Model || first.EventType != "orq_delegation" {
		t.Fatalf("unexpected observer event: %+v", first)
	}
	if first.TokensIn != 0 || first.TokensOut != 0 || first.CostEstimated != 0 {
		t.Fatalf("orq coordination events must not report model usage: %+v", first)
	}
}

func TestVerifyLastObserverLedgerEventReportsSyncedClaude(t *testing.T) {
	dir := t.TempDir()
	ledgerPath := filepath.Join(dir, "ledger.jsonl")
	statePath := filepath.Join(dir, "observer-sync.json")
	event := ledger.Event{Timestamp: time.Date(2026, 8, 30, 12, 0, 0, 0, time.UTC), Task: "validacion opus", Agent: "claude-code", Model: "claude-opus-4-1-20250805", Status: "completed"}
	if err := ledger.Append(ledgerPath, event); err != nil {
		t.Fatalf("append ledger: %v", err)
	}
	obsEvent := observerEventFromLedger(event)
	state := observerSyncState{Sent: map[string]time.Time{obsEvent.EventID: time.Now().UTC()}}
	data, err := json.Marshal(state)
	if err != nil {
		t.Fatalf("marshal state: %v", err)
	}
	if err := os.WriteFile(statePath, data, 0o600); err != nil {
		t.Fatalf("write state: %v", err)
	}

	report, err := verifyLastObserverLedgerEvent(ledgerPath, statePath, "claude-code")
	if err != nil {
		t.Fatalf("verify last: %v", err)
	}
	if !report.Found || !report.Synced || report.Agent != "claude-code" || report.Model != "claude-opus-4-1-20250805" {
		t.Fatalf("unexpected report: %+v", report)
	}
}

func TestVerifyLastObserverLedgerEventReportsUnsynced(t *testing.T) {
	dir := t.TempDir()
	ledgerPath := filepath.Join(dir, "ledger.jsonl")
	statePath := filepath.Join(dir, "observer-sync.json")
	event := ledger.Event{Timestamp: time.Date(2026, 8, 30, 12, 0, 0, 0, time.UTC), Task: "validacion opus", Agent: "claude-code", Model: "opus", Status: "timeout"}
	if err := ledger.Append(ledgerPath, event); err != nil {
		t.Fatalf("append ledger: %v", err)
	}

	report, err := verifyLastObserverLedgerEvent(ledgerPath, statePath, "claude-code")
	if err != nil {
		t.Fatalf("verify last: %v", err)
	}
	if !report.Found || report.Synced || report.Status != "timeout" {
		t.Fatalf("unexpected report: %+v", report)
	}
}

func TestSyncObserverLedgerDryRunHonorsState(t *testing.T) {
	t.Setenv("HOME", t.TempDir())
	t.Setenv("ORQ_OBSERVER_HOST_TOKEN", "")
	t.Setenv("ORQ_OBSERVER_HOST_TOKEN_FILE", "")
	t.Setenv("ORQ_OBSERVER_URL", "")

	dir := t.TempDir()
	ledgerPath := filepath.Join(dir, "ledger.jsonl")
	statePath := filepath.Join(dir, "observer-sync.json")
	event := ledger.Event{Timestamp: time.Date(2026, 8, 30, 12, 0, 0, 0, time.UTC), Task: "sync", Agent: "pi", Model: "gpt-5.5-minimal", Status: "completed"}
	if err := ledger.Append(ledgerPath, event); err != nil {
		t.Fatalf("append ledger: %v", err)
	}
	obsEvent := observerEventFromLedger(event)
	state := observerSyncState{Sent: map[string]time.Time{obsEvent.EventID: time.Now().UTC()}}
	data, err := json.Marshal(state)
	if err != nil {
		t.Fatalf("marshal state: %v", err)
	}
	if err := os.WriteFile(statePath, data, 0o600); err != nil {
		t.Fatalf("write state: %v", err)
	}

	report, err := syncObserverLedger(ledgerPath, statePath, true)
	if err != nil {
		t.Fatalf("sync dry-run: %v", err)
	}
	if report.Scanned != 1 || report.Pending != 0 || report.Sent != 0 || !report.DryRun {
		t.Fatalf("unexpected report: %+v", report)
	}
}
