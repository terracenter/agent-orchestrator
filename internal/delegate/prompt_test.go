package delegate

import (
	"strings"
	"testing"

	"github.com/terracenter/agent-orchestrator/internal/route"
)

func TestPromptIncludesCheapRouting(t *testing.T) {
	decision := route.Decide("ordenar informacion del vault relacionada con GLPI")
	prompt := Prompt(decision.Task, decision)
	for _, want := range []string{"OBLIGATORIO", "prefijado con rtk", "vg", "categoria=documentacion", "Evita escalar"} {
		if !strings.Contains(prompt, want) {
			t.Fatalf("prompt missing %q:\n%s", want, prompt)
		}
	}
}
