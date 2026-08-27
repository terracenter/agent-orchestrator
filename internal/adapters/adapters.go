package adapters

import "context"

// Shell executes commands. Implementations may wrap execution with rtk or use os/exec directly.
type Shell interface {
	Command(ctx context.Context, name string, args ...string) Result
}

type Result struct {
	Stdout   string
	Stderr   string
	ExitCode int
}

// Graph exposes optional project graph context. Implementations may call vg, Kuzu, or no-op.
type Graph interface {
	Available(ctx context.Context) bool
	Backlinks(ctx context.Context, path string) ([]string, error)
}

// Memory exposes optional agent memory. Implementations may call Engram MCP/HTTP or JSONL.
type Memory interface {
	Save(ctx context.Context, key string, content string) error
	Search(ctx context.Context, query string) ([]string, error)
}
