package delegate

import (
	"fmt"
	"strings"

	"github.com/terracenter/agent-orchestrator/internal/route"
)

type PlanOptions struct {
	Task        string `json:"task"`
	Agent       string `json:"agent"`
	Executed    bool   `json:"executed"`
	HandoffPath string `json:"handoff_path,omitempty"`
	RepoPath    string `json:"repo_path,omitempty"`
	AgentsDir   string `json:"agents_dir,omitempty"`
	Workspace   string `json:"workspace,omitempty"`
	Model       string `json:"model,omitempty"`
}

type Result struct {
	Status                string         `json:"status"`
	Decision              route.Decision `json:"decision"`
	Prompt                string         `json:"prompt"`
	AutonomousCommand     string         `json:"autonomous_command,omitempty"`
	Command               string         `json:"command,omitempty"`
	NextStep              string         `json:"next_step"`
	MustStopForDelegation bool           `json:"must_stop_for_delegation"`
	SupervisorOnly        bool           `json:"supervisor_only"`
	ExecutionAgentAllowed bool           `json:"execution_agent_allowed"`
}

func isPiAgent(agent string) bool {
	normalized := strings.ToLower(strings.TrimSpace(agent))
	return normalized == "pi" || normalized == "pi-api" || strings.HasPrefix(normalized, "pi/")
}

func isAGYAgent(agent string) bool {
	normalized := strings.ToLower(strings.TrimSpace(agent))
	return normalized == "agy" || normalized == "agy-cli" || normalized == "antigravity" || normalized == "antigravity-cli" || strings.HasPrefix(normalized, "agy/")
}

func BuildAGYCommand(opts PlanOptions, decision route.Decision) string {
	workspace := opts.Workspace
	if workspace == "" {
		workspace = "/home/freddy/Workspace"
	}

	repo := opts.RepoPath
	if repo == "" {
		repo = "/home/freddy/Workspace/Desarrollo/agent-orchestrator"
	}

	agentsDir := opts.AgentsDir
	if agentsDir == "" {
		agentsDir = "/home/freddy/Workspace/.agents"
	}

	model := opts.Model
	if model == "" {
		if isAGYAgent(decision.RecommendedAgent) && decision.RecommendedModel != "" {
			model = decision.RecommendedModel
		} else {
			model = "gemini-3.7-flash-high"
		}
	}

	var printInstruction string
	if opts.HandoffPath != "" {
		printInstruction = fmt.Sprintf("Olvida el historial anterior. Lee y ejecuta %s", opts.HandoffPath)
	} else if strings.HasSuffix(strings.TrimSpace(opts.Task), ".md") || strings.Contains(opts.Task, "handoffs/") {
		printInstruction = fmt.Sprintf("Olvida el historial anterior. Lee y ejecuta %s", strings.TrimSpace(opts.Task))
	} else if strings.TrimSpace(opts.Task) != "" {
		printInstruction = fmt.Sprintf("Olvida el historial anterior. %s", strings.TrimSpace(opts.Task))
	} else {
		printInstruction = "Olvida el historial anterior."
	}

	return fmt.Sprintf("cd %s\nrtk agy --model %s --dangerously-skip-permissions --add-dir %s --add-dir %s --print=%q",
		workspace,
		model,
		repo,
		agentsDir,
		printInstruction,
	)
}

func PlanWithOptions(opts PlanOptions) Result {
	task := opts.Task
	if task == "" && opts.HandoffPath != "" {
		task = fmt.Sprintf("ejecutar handoff %s", opts.HandoffPath)
	}
	decision := route.Decide(task)
	if opts.Model != "" {
		decision.RecommendedModel = opts.Model
	}
	prompt := Prompt(task, decision)
	status := "not_executed"
	nextStep := "ejecutar este prompt en el agente/modelo recomendado y volver con recibo verificable antes de continuar desde Pi"
	mustStop := false
	supervisorOnly := false
	execAllowed := true

	if isPiAgent(opts.Agent) {
		supervisorOnly = true
		if !opts.Executed {
			mustStop = true
			execAllowed = false
		}
	}

	if opts.Executed {
		status = "executed_unverified"
		nextStep = "adjuntar recibo verificable del agente externo antes de cerrar la tarea"
		mustStop = false
		execAllowed = true
	}

	var autoCmd string
	if isAGYAgent(opts.Agent) || isAGYAgent(decision.RecommendedAgent) || opts.HandoffPath != "" {
		autoCmd = BuildAGYCommand(opts, decision)
	}

	if autoCmd != "" && !opts.Executed && isPiAgent(opts.Agent) {
		nextStep = "ejecutar el comando sugerido en AGY CLI y volver con recibo verificable antes de continuar desde Pi"
	}

	return Result{
		Status:                status,
		Decision:              decision,
		Prompt:                prompt,
		AutonomousCommand:     autoCmd,
		Command:               autoCmd,
		NextStep:              nextStep,
		MustStopForDelegation: mustStop,
		SupervisorOnly:        supervisorOnly,
		ExecutionAgentAllowed: execAllowed,
	}
}

func Plan(task string, currentAgent string, executed bool) Result {
	return PlanWithOptions(PlanOptions{
		Task:     task,
		Agent:    currentAgent,
		Executed: executed,
	})
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
