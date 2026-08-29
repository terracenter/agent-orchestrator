package config

import (
	"fmt"

	"github.com/terracenter/agent-orchestrator/internal/adapters"
)

func ShellAdapter(cfg Config) (adapters.Shell, error) {
	switch cfg.Shell.Type {
	case "", "standard":
		return adapters.StandardShell{}, nil
	case "rtk":
		return adapters.RTKShell{}, nil
	default:
		return nil, fmt.Errorf("unsupported shell adapter %q", cfg.Shell.Type)
	}
}

func GraphAdapter(cfg Config, shell adapters.Shell) (adapters.Graph, error) {
	switch cfg.Graph.Type {
	case "", "noop":
		return adapters.NoopGraph{}, nil
	case "vg":
		if _, ok := shell.(adapters.RTKShell); !ok {
			return nil, fmt.Errorf("graph adapter %q requires shell.type=rtk (got %T): bare vg calls are forbidden (BUG-RTK-VG-001)", "vg", shell)
		}
		return adapters.VGGraph{Shell: shell}, nil
	default:
		return nil, fmt.Errorf("unsupported graph adapter %q", cfg.Graph.Type)
	}
}

func MemoryAdapter(cfg Config) (adapters.Memory, error) {
	switch cfg.Memory.Type {
	case "", "noop":
		return adapters.NoopMemory{}, nil
	case "jsonl":
		return adapters.JSONLMemory{Path: cfg.Memory.Path}, nil
	default:
		return nil, fmt.Errorf("unsupported memory adapter %q", cfg.Memory.Type)
	}
}
