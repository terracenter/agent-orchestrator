package handoff

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestBuildChain(t *testing.T) {
	body, err := BuildChain(ChainRequest{From: "a.md", To: "b.md", Task: "validar orquestador", NextAgent: "AGY"}, "resultado anterior")
	if err != nil {
		t.Fatal(err)
	}
	for _, want := range []string{"# HANDOFF CHAIN: validar orquestador", "Siguiente agente: AGY", "resultado anterior", "orq guard-collision", "No pedirle a Freddy"} {
		if !strings.Contains(body, want) {
			t.Fatalf("missing %q in:\n%s", want, body)
		}
	}
}

func TestBuildChainRequiresFields(t *testing.T) {
	if _, err := BuildChain(ChainRequest{}, "previo"); err == nil {
		t.Fatal("expected required fields error")
	}
}

func TestChainWritesWithoutOverwrite(t *testing.T) {
	dir := t.TempDir()
	from := filepath.Join(dir, "from.md")
	to := filepath.Join(dir, "nested", "to.md")
	if err := os.WriteFile(from, []byte("feedback anterior"), 0o644); err != nil {
		t.Fatal(err)
	}
	result, err := Chain(ChainRequest{From: from, To: to, Task: "seguir", NextAgent: "Pi"})
	if err != nil {
		t.Fatal(err)
	}
	if result.Bytes == 0 || result.To != to {
		t.Fatalf("unexpected result: %+v", result)
	}
	if _, err := Chain(ChainRequest{From: from, To: to, Task: "seguir", NextAgent: "Pi"}); err == nil {
		t.Fatal("expected overwrite protection")
	}
}
