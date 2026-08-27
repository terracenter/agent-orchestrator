package adapters

import (
	"context"
	"strings"
	"testing"
)

type fakeShell struct {
	result Result
	calls  []string
}

func (shell *fakeShell) Command(_ context.Context, name string, args ...string) Result {
	shell.calls = append(shell.calls, strings.Join(append([]string{name}, args...), " "))
	return shell.result
}

func TestVGGraphBacklinks(t *testing.T) {
	shell := &fakeShell{result: Result{Stdout: `{"backlinks":[{"path":"a.md"},{"path":"b.md"}]}`, ExitCode: 0}}
	graph := VGGraph{Shell: shell}
	paths, err := graph.Backlinks(context.Background(), "target.md")
	if err != nil {
		t.Fatalf("Backlinks() error = %v", err)
	}
	if len(paths) != 2 || paths[0] != "a.md" || paths[1] != "b.md" {
		t.Fatalf("paths = %#v", paths)
	}
	if got := shell.calls[0]; got != "vg backlinks target.md --format json" {
		t.Fatalf("call = %q", got)
	}
}

func TestVGGraphBacklinksFailure(t *testing.T) {
	graph := VGGraph{Shell: &fakeShell{result: Result{Stderr: "boom", ExitCode: 1}}}
	if _, err := graph.Backlinks(context.Background(), "target.md"); err == nil {
		t.Fatal("Backlinks() error = nil, want error")
	}
}
