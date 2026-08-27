package adapters

import (
	"context"
	"path/filepath"
	"testing"
)

func TestJSONLMemorySaveSearch(t *testing.T) {
	path := filepath.Join(t.TempDir(), "memory.jsonl")
	memory := JSONLMemory{Path: path}
	ctx := context.Background()
	if err := memory.Save(ctx, "task-1", "clasificar tarea mecanica"); err != nil {
		t.Fatalf("Save() error = %v", err)
	}
	matches, err := memory.Search(ctx, "mecanica")
	if err != nil {
		t.Fatalf("Search() error = %v", err)
	}
	if len(matches) != 1 {
		t.Fatalf("matches = %#v", matches)
	}
}
