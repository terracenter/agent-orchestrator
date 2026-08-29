package delegate

import (
	"encoding/json"
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

func TestPlanForPiUnexecutedRequiresStop(t *testing.T) {
	res := Plan("ordenar informacion del vault", "pi", false)
	if res.Status != "not_executed" {
		t.Fatalf("Status = %q, want not_executed", res.Status)
	}
	if !res.MustStopForDelegation {
		t.Fatal("MustStopForDelegation = false, want true")
	}
	if !res.SupervisorOnly {
		t.Fatal("SupervisorOnly = false, want true")
	}
	if res.ExecutionAgentAllowed {
		t.Fatal("ExecutionAgentAllowed = true, want false")
	}
	if res.AutonomousCommand == "" {
		t.Fatal("AutonomousCommand should be populated for recommended AGY delegation")
	}
}

func TestPlanForPiExecutedAllowsReceipt(t *testing.T) {
	res := Plan("ordenar informacion del vault", "pi", true)
	if res.Status != "executed_unverified" {
		t.Fatalf("Status = %q, want executed_unverified", res.Status)
	}
	if res.MustStopForDelegation {
		t.Fatal("MustStopForDelegation = true, want false when already executed")
	}
	if !res.SupervisorOnly {
		t.Fatal("SupervisorOnly = false, want true")
	}
	if !res.ExecutionAgentAllowed {
		t.Fatal("ExecutionAgentAllowed = false, want true")
	}
}

func TestPlanForExternalAgentAllowsExecution(t *testing.T) {
	res := Plan("ordenar informacion del vault", "agy", false)
	if res.Status != "not_executed" {
		t.Fatalf("Status = %q, want not_executed", res.Status)
	}
	if res.MustStopForDelegation {
		t.Fatal("MustStopForDelegation = true, want false for agy")
	}
	if res.SupervisorOnly {
		t.Fatal("SupervisorOnly = true, want false for agy")
	}
	if !res.ExecutionAgentAllowed {
		t.Fatal("ExecutionAgentAllowed = false, want true for agy")
	}
	if res.AutonomousCommand == "" {
		t.Fatal("AutonomousCommand should be generated for AGY")
	}
}

func TestBuildAGYCommandWithHandoff(t *testing.T) {
	opts := PlanOptions{
		Task:        "implementar feature",
		Agent:       "pi",
		HandoffPath: "/home/freddy/Workspace/.agents/handoffs/orq-delegate-agy-cli-autonomo-2026-08-29.md",
		RepoPath:    "/home/freddy/Workspace/Desarrollo/agent-orchestrator",
		AgentsDir:   "/home/freddy/Workspace/.agents",
		Workspace:   "/home/freddy/Workspace",
	}
	res := PlanWithOptions(opts)

	if res.AutonomousCommand == "" {
		t.Fatal("expected AutonomousCommand to be populated")
	}
	if res.Command != res.AutonomousCommand {
		t.Fatalf("Command %q != AutonomousCommand %q", res.Command, res.AutonomousCommand)
	}

	wants := []string{
		"cd /home/freddy/Workspace",
		"rtk agy --model gemini-3.7-flash-high",
		"--dangerously-skip-permissions",
		"--add-dir /home/freddy/Workspace/Desarrollo/agent-orchestrator",
		"--add-dir /home/freddy/Workspace/.agents",
		`--print="Olvida el historial anterior. Lee y ejecuta /home/freddy/Workspace/.agents/handoffs/orq-delegate-agy-cli-autonomo-2026-08-29.md"`,
	}
	for _, want := range wants {
		if !strings.Contains(res.AutonomousCommand, want) {
			t.Errorf("AutonomousCommand missing %q:\n%s", want, res.AutonomousCommand)
		}
	}

	if strings.Contains(res.AutonomousCommand, "--effort") {
		t.Errorf("AutonomousCommand should not emit --effort: %s", res.AutonomousCommand)
	}
}

func TestBuildAGYCommandCustomOptions(t *testing.T) {
	opts := PlanOptions{
		Task:        "tarea de prueba",
		Agent:       "agy",
		Model:       "gemini-3.5-flash-low",
		RepoPath:    "/home/freddy/Workspace/Desarrollo/custom-repo",
		AgentsDir:   "/custom/agents",
		Workspace:   "/custom/workspace",
		HandoffPath: "/custom/handoff.md",
	}
	res := PlanWithOptions(opts)

	if !strings.Contains(res.AutonomousCommand, "cd /custom/workspace") {
		t.Errorf("expected custom workspace in command: %s", res.AutonomousCommand)
	}
	if !strings.Contains(res.AutonomousCommand, "--model gemini-3.5-flash-low") {
		t.Errorf("expected custom model in command: %s", res.AutonomousCommand)
	}
	if !strings.Contains(res.AutonomousCommand, "--add-dir /home/freddy/Workspace/Desarrollo/custom-repo") {
		t.Errorf("expected custom repo in command: %s", res.AutonomousCommand)
	}
	if !strings.Contains(res.AutonomousCommand, "--add-dir /custom/agents") {
		t.Errorf("expected custom agents-dir in command: %s", res.AutonomousCommand)
	}
	if !strings.Contains(res.AutonomousCommand, `--print="Olvida el historial anterior. Lee y ejecuta /custom/handoff.md"`) {
		t.Errorf("expected handoff print instruction: %s", res.AutonomousCommand)
	}
}

func TestBuildAGYCommandDocRoutingModel(t *testing.T) {
	opts := PlanOptions{
		Task:  "ordenar documentacion del vault",
		Agent: "pi",
	}
	res := PlanWithOptions(opts)

	if !strings.Contains(res.AutonomousCommand, "--model gpt-oss-120b-medium") {
		t.Errorf("expected gpt-oss-120b-medium for documentation routing: %s", res.AutonomousCommand)
	}
	if !strings.Contains(res.AutonomousCommand, `--print="Olvida el historial anterior. ordenar documentacion del vault"`) {
		t.Errorf("expected task in print instruction: %s", res.AutonomousCommand)
	}
}

func TestPlanWithOptionsJSON(t *testing.T) {
	opts := PlanOptions{
		Task:        "implementar feature",
		Agent:       "pi",
		HandoffPath: "/home/freddy/Workspace/.agents/handoffs/test.md",
	}
	res := PlanWithOptions(opts)

	data, err := json.Marshal(res)
	if err != nil {
		t.Fatalf("failed to marshal Result to JSON: %v", err)
	}

	var parsed map[string]any
	if err := json.Unmarshal(data, &parsed); err != nil {
		t.Fatalf("failed to unmarshal JSON: %v", err)
	}

	if _, ok := parsed["autonomous_command"]; !ok {
		t.Errorf("expected autonomous_command in JSON output")
	}
	if _, ok := parsed["command"]; !ok {
		t.Errorf("expected command in JSON output")
	}
}
