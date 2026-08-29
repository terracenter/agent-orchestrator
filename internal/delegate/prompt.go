package delegate

import (
	"fmt"
	"strings"

	"github.com/terracenter/agent-orchestrator/internal/route"
)

func Prompt(task string, decision route.Decision) string {
	lines := []string{
		"OBLIGATORIO: Usa rtk; todo comando de terminal/git/filesystem debe ir prefijado con rtk. Si un comando no usa rtk, reportalo como BUG de orq y no lo ocultes.",
		"Usa vg para consultar el vault cuando aplique.",
		fmt.Sprintf("Tarea: %s", task),
		fmt.Sprintf("Routing orq: categoria=%s nivel=%d agente=%s modelo=%s", decision.Category, decision.RecommendedLevel, decision.RecommendedAgent, decision.RecommendedModel),
	}
	if len(decision.AvoidAgents) > 0 {
		lines = append(lines, "Evita escalar a: "+strings.Join(decision.AvoidAgents, ", ")+" salvo error o duda explícita.")
	}
	lines = append(lines,
		"Primero descubre el estado actual y presenta plan si la tarea puede mover o reordenar informacion.",
		"No ejecutes acciones destructivas sin validacion.",
		"Al terminar, reporta comandos de validacion y estado final.",
	)
	return strings.Join(lines, "\n")
}
