package delegate

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/terracenter/agent-orchestrator/internal/receipt"
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

func TestBuildHandoffProtocolAndContent(t *testing.T) {
	decision := route.Decide("rotar token de producción")
	opts := PlanOptions{
		Task:  "rotar token de producción",
		Agent: "pi",
	}
	content := BuildHandoff(opts, decision)

	wants := []string{
		"# HANDOFF: rotar token de producción",
		"**Supervisor:** pi",
		"**Agente recomendado:**",
		"## Objetivo",
		"rotar token de producción",
		"## Protocolo obligatorio",
		"Olvida el historial anterior.",
		"Todo comando de terminal/git/filesystem debe usar `rtk`.",
		"Usar `vg` para consultar el vault cuando aplique.",
		"No usar `sudo`.",
		"No push directo a `main`.",
		"No leer credenciales ni tokens ni exponer secretos.",
		"No ejecutar acciones destructivas sin validación previa.",
		"## Validación requerida y entrega esperada",
		"rtk go test ./...",
		"Adjuntar recibo verificable (receipt) antes de cerrar la tarea.",
	}

	for _, want := range wants {
		if !strings.Contains(content, want) {
			t.Errorf("BuildHandoff missing %q:\n%s", want, content)
		}
	}
}

func TestBuildReceiptValidJSONAndStructure(t *testing.T) {
	decision := route.Decide("implementar feature")
	opts := PlanOptions{
		Task:  "implementar feature",
		Agent: "pi",
	}
	r := BuildReceipt(opts, decision)

	if r.Task != "implementar feature" {
		t.Errorf("Receipt Task = %q, want 'implementar feature'", r.Task)
	}
	if r.Agent == "" {
		t.Error("Receipt Agent should not be empty")
	}
	if r.Provider == "" {
		t.Error("Receipt Provider should not be empty")
	}
	if r.Model == "" {
		t.Error("Receipt Model should not be empty")
	}
	if len(r.Commands) == 0 {
		t.Error("Receipt Commands should not be empty")
	}
	if r.Rollback == "" {
		t.Error("Receipt Rollback should not be empty")
	}

	data, err := json.Marshal(r)
	if err != nil {
		t.Fatalf("failed to marshal Receipt to JSON: %v", err)
	}

	var parsed receipt.Receipt
	if err := json.Unmarshal(data, &parsed); err != nil {
		t.Fatalf("failed to unmarshal JSON: %v", err)
	}
}

func TestWriteDelegationFilesSuccess(t *testing.T) {
	tempDir := t.TempDir()
	handoffFile := filepath.Join(tempDir, "handoffs", "test-handoff.md")
	receiptFile := filepath.Join(tempDir, "receipts", "test-receipt.json")

	opts := PlanOptions{
		Task:         "probar escritura de delegacion",
		Agent:        "pi",
		WriteHandoff: handoffFile,
		WriteReceipt: receiptFile,
	}

	res := PlanWithOptions(opts)
	err := WriteDelegationFiles(opts, &res)
	if err != nil {
		t.Fatalf("WriteDelegationFiles failed: %v", err)
	}

	if res.WrittenHandoff != handoffFile {
		t.Errorf("WrittenHandoff = %q, want %q", res.WrittenHandoff, handoffFile)
	}
	if res.WrittenReceipt != receiptFile {
		t.Errorf("WrittenReceipt = %q, want %q", res.WrittenReceipt, receiptFile)
	}

	// Verify handoff file exists and has content
	handoffData, err := os.ReadFile(handoffFile)
	if err != nil {
		t.Fatalf("failed to read written handoff: %v", err)
	}
	if !strings.Contains(string(handoffData), "probar escritura de delegacion") {
		t.Errorf("handoff content missing task: %s", string(handoffData))
	}

	// Verify receipt file exists and is valid JSON
	receiptData, err := os.ReadFile(receiptFile)
	if err != nil {
		t.Fatalf("failed to read written receipt: %v", err)
	}
	var r receipt.Receipt
	if err := json.Unmarshal(receiptData, &r); err != nil {
		t.Fatalf("written receipt is not valid JSON: %v", err)
	}
	if r.Task != "probar escritura de delegacion" {
		t.Errorf("receipt task = %q, want 'probar escritura de delegacion'", r.Task)
	}
}

func TestWriteDelegationFilesNoOverwriteByDefault(t *testing.T) {
	tempDir := t.TempDir()
	handoffFile := filepath.Join(tempDir, "existing-handoff.md")
	initialContent := "original content"
	if err := os.WriteFile(handoffFile, []byte(initialContent), 0o644); err != nil {
		t.Fatalf("failed to create existing file: %v", err)
	}

	opts := PlanOptions{
		Task:         "tarea nueva",
		Agent:        "pi",
		WriteHandoff: handoffFile,
		Force:        false,
	}

	res := PlanWithOptions(opts)
	err := WriteDelegationFiles(opts, &res)
	if err == nil {
		t.Fatal("expected error due to existing file without force, got nil")
	}
	if !strings.Contains(err.Error(), "already exists") {
		t.Fatalf("expected 'already exists' in error, got: %v", err)
	}

	// Content should be unchanged
	data, _ := os.ReadFile(handoffFile)
	if string(data) != initialContent {
		t.Fatalf("file content changed without --force: %s", string(data))
	}
}

func TestWriteDelegationFilesOverwriteWithForce(t *testing.T) {
	tempDir := t.TempDir()
	handoffFile := filepath.Join(tempDir, "existing-handoff.md")
	initialContent := "original content"
	if err := os.WriteFile(handoffFile, []byte(initialContent), 0o644); err != nil {
		t.Fatalf("failed to create existing file: %v", err)
	}

	opts := PlanOptions{
		Task:         "tarea nueva",
		Agent:        "pi",
		WriteHandoff: handoffFile,
		Force:        true,
	}

	res := PlanWithOptions(opts)
	err := WriteDelegationFiles(opts, &res)
	if err != nil {
		t.Fatalf("expected success with force=true, got: %v", err)
	}

	data, _ := os.ReadFile(handoffFile)
	if string(data) == initialContent {
		t.Fatal("file content was not overwritten despite force=true")
	}
	if !strings.Contains(string(data), "tarea nueva") {
		t.Fatalf("file does not contain new task: %s", string(data))
	}
}

func TestPlanWithOptionsWithWriteHandoffSetsAutonomousCommand(t *testing.T) {
	opts := PlanOptions{
		Task:         "probar escritura de handoff",
		Agent:        "agy",
		WriteHandoff: "/tmp/orq-handoff-test.md",
	}
	res := PlanWithOptions(opts)

	if !strings.Contains(res.AutonomousCommand, "Lee y ejecuta /tmp/orq-handoff-test.md") {
		t.Errorf("expected autonomous command to reference write-handoff path: %s", res.AutonomousCommand)
	}
}

func TestWriteDelegationFilesInvalidPath(t *testing.T) {
	opts := PlanOptions{
		Task:         "tarea",
		Agent:        "pi",
		WriteHandoff: "/dev/null/impossible/path/handoff.md",
	}
	res := PlanWithOptions(opts)
	err := WriteDelegationFiles(opts, &res)
	if err == nil {
		t.Fatal("expected error for invalid path, got nil")
	}
}
