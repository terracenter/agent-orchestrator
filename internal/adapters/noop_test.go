package adapters

import (
	"context"
	"testing"
)

func TestNoopAdapters(t *testing.T) {
	ctx := context.Background()
	graph := NoopGraph{}
	if graph.Available(ctx) {
		t.Fatal("NoopGraph.Available() = true, want false")
	}
	memory := NoopMemory{}
	if err := memory.Save(ctx, "key", "content"); err != nil {
		t.Fatalf("NoopMemory.Save() error = %v", err)
	}
}
