package delegate

import (
	"fmt"
	"strings"

	"github.com/terracenter/agent-orchestrator/internal/route"
)

type Result struct {
	Status                string         `json:"status"`
	Decision              route.Decision `json:"decision"`
	Prompt                string         `json:"prompt"`
	NextStep              string         `json:"next_step"`
	MustStopForDelegation bool           `json:"must_stop_for_delegation"`
	SupervisorOnly        bool           `json:"supervisor_only"`
	ExecutionAgentAllowed bool           `json:"execution_agent_allowed"`
}

func isPiAgent(agent string) bool {
	normalized := strings.ToLower(strings.TrimSpace(agent))
	return normalized == "pi" || normalized == "pi-api" || strings.HasPrefix(normalized, "pi/")
}

func Plan(task string, currentAgent string, executed bool) Result {
	decision := route.Decide(task)
	prompt := Prompt(task, decision)
	status := "not_executed"
	nextStep := "ejecutar este prompt en el agente/modelo recomendado y volver con recibo verificable antes de continuar desde Pi"
	mustStop := false
	supervisorOnly := false
	execAllowed := true

	if isPiAgent(currentAgent) {
		supervisorOnly = true
		if !executed {
			mustStop = true
			execAllowed = false
		}
	}

	if executed {
		status = "executed_unverified"
		nextStep = "adjuntar recibo verificable del agente externo antes de cerrar la tarea"
		mustStop = false
		execAllowed = true
	}

	return Result{
		Status:                status,
		Decision:              decision,
		Prompt:                prompt,
		NextStep:              nextStep,
		MustStopForDelegation: mustStop,
		SupervisorOnly:        supervisorOnly,
		ExecutionAgentAllowed: execAllowed,
	}
}

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
