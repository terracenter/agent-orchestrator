package delegate

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/terracenter/agent-orchestrator/internal/agent"
	"github.com/terracenter/agent-orchestrator/internal/receipt"
	"github.com/terracenter/agent-orchestrator/internal/route"
)

type PlanOptions struct {
	Task         string `json:"task"`
	Agent        string `json:"agent"`
	Executed     bool   `json:"executed"`
	HandoffPath  string `json:"handoff_path,omitempty"`
	RepoPath     string `json:"repo_path,omitempty"`
	AgentsDir    string `json:"agents_dir,omitempty"`
	Workspace    string `json:"workspace,omitempty"`
	Model        string `json:"model,omitempty"`
	WriteHandoff string `json:"write_handoff,omitempty"`
	WriteReceipt string `json:"write_receipt,omitempty"`
	Force        bool   `json:"force,omitempty"`
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
	WrittenHandoff        string         `json:"written_handoff,omitempty"`
	WrittenReceipt        string         `json:"written_receipt,omitempty"`
}

func isPiAgent(agentName string) bool {
	normalized := strings.ToLower(strings.TrimSpace(agentName))
	return normalized == "pi" || normalized == "pi-api" || strings.HasPrefix(normalized, "pi/")
}

func isAGYAgent(agentName string) bool {
	normalized := strings.ToLower(strings.TrimSpace(agentName))
	return normalized == "agy" || normalized == "agy-cli" || normalized == "antigravity" || normalized == "antigravity-cli" || strings.HasPrefix(normalized, "agy/")
}

func isHermesAgent(agentName string) bool {
	normalized := strings.ToLower(strings.TrimSpace(agentName))
	return normalized == "hermes" || strings.HasPrefix(normalized, "hermes/")
}

func isOpenClawAgent(agentName string) bool {
	normalized := strings.ToLower(strings.TrimSpace(agentName))
	return normalized == "openclaw" || normalized == "open-claw" || strings.HasPrefix(normalized, "openclaw/")
}

func requiresExternalCheapExecution(agentName string) bool {
	normalized := strings.ToLower(strings.TrimSpace(agentName))
	return isAGYAgent(normalized) || isOpenClawAgent(normalized) || normalized == "nvidia-api" || strings.HasPrefix(normalized, "nvidia-api/") || normalized == "local" || normalized == "local-or-cheap"
}

func BuildOpenClawCommand(opts PlanOptions, decision route.Decision) string {
	model := opts.Model
	handoffTarget := opts.HandoffPath
	if handoffTarget == "" && opts.WriteHandoff != "" {
		handoffTarget = opts.WriteHandoff
	}

	agentID := "main"
	if opts.Agent != "" && isOpenClawAgent(opts.Agent) {
		parts := strings.Split(opts.Agent, "/")
		if len(parts) > 1 {
			agentID = parts[1]
		}
	} else if isOpenClawAgent(decision.RecommendedAgent) {
		parts := strings.Split(decision.RecommendedAgent, "/")
		if len(parts) > 1 {
			agentID = parts[1]
		}
	}

	var args []string
	args = append(args, "agent", "--agent", agentID)

	if model != "" {
		args = append(args, "--model", model)
	}

	if handoffTarget != "" {
		args = append(args, "--message-file", handoffTarget)
	} else {
		printInstruction := "Olvida el historial anterior."
		taskTrimmed := strings.TrimSpace(opts.Task)
		if strings.HasSuffix(taskTrimmed, ".md") || strings.Contains(taskTrimmed, "handoffs/") {
			printInstruction = fmt.Sprintf("Olvida el historial anterior. Lee y ejecuta %s", taskTrimmed)
		} else if taskTrimmed != "" {
			printInstruction = fmt.Sprintf("Olvida el historial anterior. %s", taskTrimmed)
		}
		args = append(args, "--message", printInstruction)
	}

	var cmdParts []string
	cmdParts = append(cmdParts, "rtk", "openclaw")
	for i := 0; i < len(args); i++ {
		if args[i] == "--message" && i+1 < len(args) {
			cmdParts = append(cmdParts, "--message", fmt.Sprintf("%q", args[i+1]))
			i++
		} else {
			cmdParts = append(cmdParts, args[i])
		}
	}

	return strings.Join(cmdParts, " ")
}

func BuildHermesCommand(opts PlanOptions, decision route.Decision) string {
	workspace := opts.Workspace
	if workspace == "" {
		workspace = "/home/freddy/Workspace"
	}

	model := opts.Model
	if model == "" {
		if isHermesAgent(decision.RecommendedAgent) && decision.RecommendedModel != "" {
			model = decision.RecommendedModel
		} else {
			model = "deepseek-v4-flash"
		}
	}

	handoffTarget := opts.HandoffPath
	if handoffTarget == "" && opts.WriteHandoff != "" {
		handoffTarget = opts.WriteHandoff
	}

	var printInstruction string
	if handoffTarget != "" {
		printInstruction = fmt.Sprintf("Olvida el historial anterior. Lee y ejecuta %s", handoffTarget)
	} else if strings.HasSuffix(strings.TrimSpace(opts.Task), ".md") || strings.Contains(opts.Task, "handoffs/") {
		printInstruction = fmt.Sprintf("Olvida el historial anterior. Lee y ejecuta %s", strings.TrimSpace(opts.Task))
	} else if strings.TrimSpace(opts.Task) != "" {
		printInstruction = fmt.Sprintf("Olvida el historial anterior. %s", strings.TrimSpace(opts.Task))
	} else {
		printInstruction = "Olvida el historial anterior."
	}

	return fmt.Sprintf("cd %s\nrtk hermes -m %s -z %q",
		workspace,
		model,
		printInstruction,
	)
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

	handoffTarget := opts.HandoffPath
	if handoffTarget == "" && opts.WriteHandoff != "" {
		handoffTarget = opts.WriteHandoff
	}

	var printInstruction string
	if handoffTarget != "" {
		printInstruction = fmt.Sprintf("Olvida el historial anterior. Lee y ejecuta %s", handoffTarget)
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

func PlanWithOptions(opts PlanOptions) (Result, error) {
	if isOpenClawAgent(opts.Agent) {
		detections := agent.DetectAgents()
		installed := false
		for _, d := range detections {
			if d.Agent == "openclaw" {
				installed = d.Installed
				break
			}
		}
		if !installed {
			return Result{}, fmt.Errorf("agente openclaw no está detectado o instalado en el sistema")
		}
	}

	task := opts.Task
	if task == "" && opts.HandoffPath != "" {
		task = fmt.Sprintf("ejecutar handoff %s", opts.HandoffPath)
	} else if task == "" && opts.WriteHandoff != "" {
		task = fmt.Sprintf("ejecutar handoff %s", filepath.Base(opts.WriteHandoff))
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
	if isOpenClawAgent(opts.Agent) {
		autoCmd = BuildOpenClawCommand(opts, decision)
	} else if isHermesAgent(opts.Agent) {
		autoCmd = BuildHermesCommand(opts, decision)
	} else if isAGYAgent(opts.Agent) {
		autoCmd = BuildAGYCommand(opts, decision)
	} else if isOpenClawAgent(decision.RecommendedAgent) {
		autoCmd = BuildOpenClawCommand(opts, decision)
	} else if isHermesAgent(decision.RecommendedAgent) {
		autoCmd = BuildHermesCommand(opts, decision)
	} else if isAGYAgent(decision.RecommendedAgent) || requiresExternalCheapExecution(decision.RecommendedAgent) || opts.HandoffPath != "" || opts.WriteHandoff != "" {
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
	}, nil
}

func Plan(task string, currentAgent string, executed bool) (Result, error) {
	return PlanWithOptions(PlanOptions{
		Task:     task,
		Agent:    currentAgent,
		Executed: executed,
	})
}

func Prompt(task string, decision route.Decision) string {
	lines := []string{
		"OBLIGATORIO: Usa rtk; todo comando de terminal/git/filesystem debe ir prefijado con rtk. Si un comando no usa rtk, reportalo como BUG de orq y no lo ocultes.",
		"Usa `rtk vg` para consultar el vault cuando aplique — nunca `vg` pelado. Si `rtk vg` falla por PATH, reportalo como BUG-RTK-VG-001 y no lo ocultes.",
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

func BuildHandoff(opts PlanOptions, decision route.Decision) string {
	taskTitle := strings.TrimSpace(opts.Task)
	if taskTitle == "" && opts.HandoffPath != "" {
		taskTitle = fmt.Sprintf("ejecutar handoff %s", filepath.Base(opts.HandoffPath))
	} else if taskTitle == "" && opts.WriteHandoff != "" {
		taskTitle = fmt.Sprintf("ejecutar handoff %s", filepath.Base(opts.WriteHandoff))
	}
	if taskTitle == "" {
		taskTitle = "tarea delegada"
	}

	agent := opts.Agent
	if agent == "" {
		agent = "supervisor"
	}

	targetAgent := decision.RecommendedAgent
	targetModel := decision.RecommendedModel
	if opts.Model != "" {
		targetModel = opts.Model
	}

	lines := []string{
		fmt.Sprintf("# HANDOFF: %s", taskTitle),
		"",
		fmt.Sprintf("**Fecha:** %s · **Supervisor:** %s", time.Now().UTC().Format(time.RFC3339), agent),
		fmt.Sprintf("**Agente recomendado:** %s · **Modelo:** %s", targetAgent, targetModel),
		fmt.Sprintf("**Categoría:** %s (Nivel %d)", decision.Category, decision.RecommendedLevel),
		"",
		"## Objetivo",
		taskTitle,
		"",
		"## Protocolo obligatorio",
		"- Olvida el historial anterior.",
		"- Todo comando de terminal/git/filesystem debe usar `rtk`.",
		"- Usar `vg` para consultar el vault cuando aplique.",
		"- No usar `sudo`.",
		"- No push directo a `main`.",
		"- No leer credenciales ni tokens ni exponer secretos.",
		"- No ejecutar acciones destructivas sin validación previa.",
	}

	if len(decision.AvoidAgents) > 0 {
		lines = append(lines, fmt.Sprintf("- Evitar escalar a: %s salvo error o duda explícita.", strings.Join(decision.AvoidAgents, ", ")))
	}

	lines = append(lines,
		"",
		"## Validación requerida y entrega esperada",
		"- Ejecutar suite de pruebas y validaciones locales (`rtk go test ./...`, etc.).",
		"- Reportar comandos de validación ejecutados y su resultado literal.",
		"- Adjuntar recibo verificable (receipt) antes de cerrar la tarea.",
		"",
	)

	return strings.Join(lines, "\n")
}

func BuildReceipt(opts PlanOptions, decision route.Decision) receipt.Receipt {
	taskTitle := strings.TrimSpace(opts.Task)
	if taskTitle == "" && opts.HandoffPath != "" {
		taskTitle = fmt.Sprintf("ejecutar handoff %s", filepath.Base(opts.HandoffPath))
	} else if taskTitle == "" && opts.WriteHandoff != "" {
		taskTitle = fmt.Sprintf("ejecutar handoff %s", filepath.Base(opts.WriteHandoff))
	}
	if taskTitle == "" {
		taskTitle = "tarea delegada"
	}

	agent := decision.RecommendedAgent
	if agent == "" {
		agent = opts.Agent
	}
	provider := providerForAgent(agent)
	model := decision.RecommendedModel
	if opts.Model != "" {
		model = opts.Model
	}

	risk := "bajo"
	if decision.Category == "seguridad" || decision.RecommendedLevel >= 3 {
		risk = "medio"
	}

	r := receipt.New(taskTitle, agent, provider, model, risk, 0)
	r.Commands = []receipt.Command{
		{Cmd: "rtk go test ./...", Result: "recorded"},
	}
	r.Rollback = "revert branch or worktree changes"
	r.Evidence = []string{
		fmt.Sprintf("delegated from %s via orq delegate", opts.Agent),
	}
	return r
}

func providerForAgent(agent string) string {
	switch strings.ToLower(strings.TrimSpace(agent)) {
	case "agy", "agy-cli", "antigravity", "antigravity-cli":
		return "google"
	case "pi", "pi-api":
		return "openai"
	case "claude", "claude-code":
		return "anthropic"
	case "hermes":
		return "hermes"
	case "nvidia-api":
		return "nvidia"
	default:
		return "local"
	}
}

func WriteDelegationFiles(opts PlanOptions, res *Result) error {
	if opts.WriteHandoff != "" {
		handoffContent := BuildHandoff(opts, res.Decision)
		if err := writeFileWithOverwriteCheck(opts.WriteHandoff, []byte(handoffContent), opts.Force); err != nil {
			return err
		}
		res.WrittenHandoff = opts.WriteHandoff
	}

	if opts.WriteReceipt != "" {
		r := BuildReceipt(opts, res.Decision)
		data, err := json.MarshalIndent(r, "", "  ")
		if err != nil {
			return fmt.Errorf("failed to marshal initial receipt: %w", err)
		}
		data = append(data, '\n')
		if err := writeFileWithOverwriteCheck(opts.WriteReceipt, data, opts.Force); err != nil {
			return err
		}
		res.WrittenReceipt = opts.WriteReceipt
	}
	return nil
}

func writeFileWithOverwriteCheck(path string, data []byte, force bool) error {
	cleanPath := filepath.Clean(path)
	if !force {
		if _, err := os.Stat(cleanPath); err == nil {
			return fmt.Errorf("file %q already exists (use --force to overwrite)", cleanPath)
		} else if !os.IsNotExist(err) {
			return fmt.Errorf("checking file %q: %w", cleanPath, err)
		}
	}

	dir := filepath.Dir(cleanPath)
	if dir != "" && dir != "." {
		if err := os.MkdirAll(dir, 0o755); err != nil {
			return fmt.Errorf("creating directory %q: %w", dir, err)
		}
	}

	if err := os.WriteFile(cleanPath, data, 0o644); err != nil {
		return fmt.Errorf("writing file %q: %w", cleanPath, err)
	}
	return nil
}
