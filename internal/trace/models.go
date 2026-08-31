package trace

import (
	"time"
)

// TraceEventType enum para tipos de evento en trace.
type TraceEventType string

const (
	EventTypeCommand   TraceEventType = "command"
	EventTypeFile      TraceEventType = "file"
	EventTypeTest      TraceEventType = "test"
	EventTypeCommit    TraceEventType = "commit"
	EventTypePR        TraceEventType = "pr"
	EventTypeIssue     TraceEventType = "issue"
	EventTypeDiscovery TraceEventType = "discovery"
)

// FileOperation enum para operación sobre archivo.
type FileOperation string

const (
	FileOpRead   FileOperation = "read"
	FileOpWrite  FileOperation = "write"
	FileOpDelete FileOperation = "delete"
)

// TestResult enum para resultado de test.
type TestResult string

const (
	TestResultPassed  TestResult = "passed"
	TestResultFailed  TestResult = "failed"
	TestResultSkipped TestResult = "skipped"
)

// TraceEvent es un evento atomic en una sesión de trace.
type TraceEvent struct {
	Timestamp time.Time      `json:"ts"`
	SessionID string         `json:"session_id"`
	EventType TraceEventType `json:"event_type"`

	// Común
	Success bool              `json:"success,omitempty"`
	Error   string            `json:"error,omitempty"`
	Details map[string]string `json:"details,omitempty"`

	// Command
	Command        string `json:"command,omitempty"`
	CommandPath    string `json:"command_path,omitempty"`
	CommandResult  string `json:"command_result,omitempty"`
	CommandExitCode int    `json:"command_exit_code,omitempty"`

	// File
	FilePath      string        `json:"file_path,omitempty"`
	FileOperation FileOperation `json:"file_operation,omitempty"`
	FileSizeBytes int64         `json:"file_size_bytes,omitempty"`
	FileHash      string        `json:"file_hash,omitempty"`

	// Test
	TestName   string     `json:"test_name,omitempty"`
	TestPath   string     `json:"test_path,omitempty"`
	TestResult TestResult `json:"test_result,omitempty"`
	TestDurationMs int64  `json:"test_duration_ms,omitempty"`

	// Commit
	CommitHash string `json:"commit_hash,omitempty"`
	CommitMsg  string `json:"commit_msg,omitempty"`
	CommitRepo string `json:"commit_repo,omitempty"`

	// PR
	PRID    int    `json:"pr_id,omitempty"`
	PRTitle string `json:"pr_title,omitempty"`
	PRRepo  string `json:"pr_repo,omitempty"`

	// Issue
	IssueID    int    `json:"issue_id,omitempty"`
	IssueTitle string `json:"issue_title,omitempty"`
	IssueRepo  string `json:"issue_repo,omitempty"`

	// Discovery
	DiscoveryType string `json:"discovery_type,omitempty"` // "memory", "config", "dependency", etc.
	DiscoveryData string `json:"discovery_data,omitempty"` // JSON string o texto
}

// TraceSession representa una sesión de tracing en progreso.
type TraceSession struct {
	ID           string    `json:"id"`
	Agent        string    `json:"agent"`
	Host         string    `json:"host"`
	Workspace    string    `json:"workspace"`
	Model        string    `json:"model,omitempty"`
	Status       string    `json:"status"` // "active", "stopped", "error"
	StartedAt    time.Time `json:"started_at"`
	StoppedAt    *time.Time `json:"stopped_at,omitempty"`
	EventCount   int       `json:"event_count"`
	Description  string    `json:"description,omitempty"`
	Metadata     map[string]string `json:"metadata,omitempty"`
}

// TraceMetadata info de la sesión para almacenamiento.
type TraceMetadata struct {
	SessionID   string
	Agent       string
	Host        string
	Workspace   string
	Model       string
	Description string
	Metadata    map[string]string
}
