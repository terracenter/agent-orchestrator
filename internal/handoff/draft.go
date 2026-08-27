package handoff

import (
	"fmt"
	"strings"
	"time"

	"github.com/terracenter/agent-orchestrator/internal/delegate"
	"github.com/terracenter/agent-orchestrator/internal/route"
	"github.com/terracenter/agent-orchestrator/internal/task"
)

func Draft(item task.Item) string {
	decision := route.Decide(item.Title)
	if item.Agent != "" {
		decision.RecommendedAgent = item.Agent
	}
	if item.Model != "" {
		decision.RecommendedModel = item.Model
	}
	body := delegate.Prompt(item.Title, decision)
	return strings.TrimSpace(fmt.Sprintf(`# HANDOFF: %s

Fecha: %s
Task ID: %s
Estado: %s
Agente: %s
Modelo: %s
Host: %s

## Instrucciones

%s

## Entrega esperada

- Reportar estado final.
- Pegar comandos de validación.
- Si hubo bloqueo, marcar la tarea como blocked con evidencia.
`, item.Title, time.Now().UTC().Format(time.RFC3339), item.ID, item.State, value(item.Agent, decision.RecommendedAgent), value(item.Model, decision.RecommendedModel), item.Host, body)) + "\n"
}

func value(current string, fallback string) string {
	if current != "" {
		return current
	}
	return fallback
}
