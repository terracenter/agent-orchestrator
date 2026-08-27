package config

import (
	"testing"

	"github.com/terracenter/agent-orchestrator/internal/adapters"
)

func TestShellAdapterFactory(t *testing.T) {
	standard, err := ShellAdapter(Config{Shell: Adapter{Type: "standard"}})
	if err != nil {
		t.Fatalf("ShellAdapter(standard) error = %v", err)
	}
	if _, ok := standard.(adapters.StandardShell); !ok {
		t.Fatalf("standard adapter type = %T", standard)
	}
	rtk, err := ShellAdapter(Config{Shell: Adapter{Type: "rtk"}})
	if err != nil {
		t.Fatalf("ShellAdapter(rtk) error = %v", err)
	}
	if _, ok := rtk.(adapters.RTKShell); !ok {
		t.Fatalf("rtk adapter type = %T", rtk)
	}
}

func TestGraphAdapterFactory(t *testing.T) {
	shell := adapters.StandardShell{}
	noop, err := GraphAdapter(Config{Graph: Adapter{Type: "noop"}}, shell)
	if err != nil {
		t.Fatalf("GraphAdapter(noop) error = %v", err)
	}
	if _, ok := noop.(adapters.NoopGraph); !ok {
		t.Fatalf("noop graph type = %T", noop)
	}
	vg, err := GraphAdapter(Config{Graph: Adapter{Type: "vg"}}, shell)
	if err != nil {
		t.Fatalf("GraphAdapter(vg) error = %v", err)
	}
	if _, ok := vg.(adapters.VGGraph); !ok {
		t.Fatalf("vg graph type = %T", vg)
	}
}

func TestMemoryAdapterFactory(t *testing.T) {
	memory, err := MemoryAdapter(Config{Memory: Adapter{Type: "jsonl", Path: "memory.jsonl"}})
	if err != nil {
		t.Fatalf("MemoryAdapter(jsonl) error = %v", err)
	}
	if _, ok := memory.(adapters.JSONLMemory); !ok {
		t.Fatalf("jsonl memory type = %T", memory)
	}
}

func TestFactoriesRejectUnsupportedAdapters(t *testing.T) {
	if _, err := ShellAdapter(Config{Shell: Adapter{Type: "custom"}}); err == nil {
		t.Fatal("ShellAdapter(custom) error = nil, want error")
	}
	if _, err := GraphAdapter(Config{Graph: Adapter{Type: "custom"}}, adapters.StandardShell{}); err == nil {
		t.Fatal("GraphAdapter(custom) error = nil, want error")
	}
	if _, err := MemoryAdapter(Config{Memory: Adapter{Type: "custom"}}); err == nil {
		t.Fatal("MemoryAdapter(custom) error = nil, want error")
	}
}
