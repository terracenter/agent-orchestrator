package adapters

import (
	"context"
	"encoding/json"
	"fmt"
)

type VGGraph struct {
	Shell Shell
}

func (graph VGGraph) Available(ctx context.Context) bool {
	result := graph.shell().Command(ctx, "vg", "stats", "--format", "json")
	return result.ExitCode == 0
}

func (graph VGGraph) Backlinks(ctx context.Context, path string) ([]string, error) {
	result := graph.shell().Command(ctx, "vg", "backlinks", path, "--format", "json")
	if result.ExitCode != 0 {
		return nil, fmt.Errorf("vg backlinks failed: %s", result.Stderr)
	}
	var payload struct {
		Backlinks []struct {
			Path string `json:"path"`
		} `json:"backlinks"`
	}
	if err := json.Unmarshal([]byte(result.Stdout), &payload); err != nil {
		return nil, fmt.Errorf("decode vg backlinks: %w", err)
	}
	paths := make([]string, 0, len(payload.Backlinks))
	for _, backlink := range payload.Backlinks {
		paths = append(paths, backlink.Path)
	}
	return paths, nil
}

func (graph VGGraph) shell() Shell {
	if graph.Shell != nil {
		return graph.Shell
	}
	return StandardShell{}
}
