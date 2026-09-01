package main

import (
	"encoding/json"
	"io"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestCmdRunDefaultsToDryRun(t *testing.T) {
	output := captureStdout(t, func() {
		if err := cmdRun([]string{"--format", "json", "hacer tarea mecanica"}); err != nil {
			t.Fatalf("cmdRun returned error: %v", err)
		}
	})

	var result struct {
		Executed bool `json:"executed"`
		DryRun   bool `json:"dry_run"`
		Receipt  any  `json:"receipt"`
	}
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("decode result: %v\n%s", err, output)
	}
	if result.Executed {
		t.Fatal("expected dry-run to not execute")
	}
	if !result.DryRun {
		t.Fatal("expected dry_run=true by default")
	}
}

func TestCmdRunExecuteParsesSucceededReceipt(t *testing.T) {
	runner := fakeOrqAgent(t, `#!/usr/bin/env bash
printf '{"schema_version":1,"correlation_id":"test","agent":"qwen-code","model":"qwen3.8-max","command":["fake"],"status":"succeeded","policy_reason":"allowed","started_at_unix":1,"duration_ms":2,"timeout_seconds":5,"exit_code":0,"stdout_tail":"ok","stderr_tail":"","secrets_read":false}\n'
`)

	output := captureStdout(t, func() {
		if err := cmdRun([]string{"--execute", "--agent", "qwen-code", "--model", "qwen3.8-max", "--timeout", "5", "--orq-agent-bin", runner, "--format", "json", "hacer smoke"}); err != nil {
			t.Fatalf("cmdRun returned error: %v", err)
		}
	})

	var result struct {
		Executed bool             `json:"executed"`
		DryRun   bool             `json:"dry_run"`
		Receipt  *OrqAgentReceipt `json:"receipt"`
	}
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("decode result: %v\n%s", err, output)
	}
	if !result.Executed {
		t.Fatal("expected executed=true for succeeded receipt")
	}
	if result.DryRun {
		t.Fatal("expected dry_run=false with --execute")
	}
	if result.Receipt == nil || result.Receipt.Status != "succeeded" {
		t.Fatalf("unexpected receipt: %+v", result.Receipt)
	}
}

func TestCmdRunExecuteFailedReceiptIsNotExecuted(t *testing.T) {
	runner := fakeOrqAgent(t, `#!/usr/bin/env bash
printf '{"schema_version":1,"correlation_id":"test","agent":"qwen-code","model":"qwen3.8-max","command":["fake"],"status":"timed_out","policy_reason":"allowed","started_at_unix":1,"duration_ms":2,"timeout_seconds":5,"exit_code":null,"stdout_tail":"partial","stderr_tail":"timed out","secrets_read":false}\n'
`)

	output := captureStdout(t, func() {
		if err := cmdRun([]string{"--execute", "--agent", "qwen-code", "--model", "qwen3.8-max", "--orq-agent-bin", runner, "--format", "json", "hacer timeout"}); err != nil {
			t.Fatalf("cmdRun returned error: %v", err)
		}
	})

	if !strings.Contains(output, `"executed":false`) || !strings.Contains(output, `"status":"timed_out"`) {
		t.Fatalf("expected not executed timed_out receipt, got: %s", output)
	}
}

func TestCmdRunExecuteRejectsInvalidTimeout(t *testing.T) {
	err := cmdRun([]string{"--execute", "--timeout", "0", "hacer algo"})
	if err == nil || !strings.Contains(err.Error(), "--timeout must be > 0") {
		t.Fatalf("expected timeout error, got %v", err)
	}
}

func fakeOrqAgent(t *testing.T, body string) string {
	t.Helper()
	path := filepath.Join(t.TempDir(), "orq-agent-fake")
	if err := os.WriteFile(path, []byte(body), 0o755); err != nil {
		t.Fatalf("write fake runner: %v", err)
	}
	return path
}

func captureStdout(t *testing.T, fn func()) string {
	t.Helper()
	old := os.Stdout
	read, write, err := os.Pipe()
	if err != nil {
		t.Fatalf("pipe: %v", err)
	}
	os.Stdout = write
	fn()
	write.Close()
	os.Stdout = old
	data, err := io.ReadAll(read)
	if err != nil {
		t.Fatalf("read stdout: %v", err)
	}
	return string(data)
}
