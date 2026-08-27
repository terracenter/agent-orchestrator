package main

import (
	"context"
	"encoding/json"
	"flag"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	agentpkg "github.com/terracenter/agent-orchestrator/internal/agent"
	"github.com/terracenter/agent-orchestrator/internal/config"
	"github.com/terracenter/agent-orchestrator/internal/delegate"
	"github.com/terracenter/agent-orchestrator/internal/guard"
	"github.com/terracenter/agent-orchestrator/internal/handoff"
	"github.com/terracenter/agent-orchestrator/internal/ledger"
	"github.com/terracenter/agent-orchestrator/internal/route"
	"github.com/terracenter/agent-orchestrator/internal/task"
	"github.com/terracenter/agent-orchestrator/internal/vaultorder"
)

func main() {
	if len(os.Args) < 2 {
		usage()
		os.Exit(2)
	}

	var err error
	switch os.Args[1] {
	case "classify":
		err = cmdClassify(os.Args[2:])
	case "route":
		err = cmdRoute(os.Args[2:])
	case "record":
		err = cmdRecord(os.Args[2:])
	case "status":
		err = cmdStatus(os.Args[2:])
	case "run":
		err = cmdRun(os.Args[2:])
	case "guard":
		err = cmdGuard(os.Args[2:])
	case "guard-collision":
		err = cmdGuardCollision(os.Args[2:])
	case "config":
		err = cmdConfig(os.Args[2:])
	case "vault-order":
		err = cmdVaultOrder(os.Args[2:])
	case "delegate":
		err = cmdDelegate(os.Args[2:])
	case "task":
		err = cmdTask(os.Args[2:])
	case "handoff":
		err = cmdHandoff(os.Args[2:])
	case "agents":
		err = cmdAgents(os.Args[2:])
	case "help", "--help", "-h":
		usage()
		return
	default:
		err = fmt.Errorf("unknown command %q", os.Args[1])
	}
	if err != nil {
		fmt.Fprintln(os.Stderr, "Error:", err)
		os.Exit(1)
	}
}

func usage() {
	fmt.Print(`orq — local-first agent/model orchestrator

Usage:
  orq classify <task>
  orq route <task> [--format json]
  orq record --task <task> --agent <agent> --model <model> --status <status> [--ledger path]
  orq status [--ledger path]
  orq run <task> [--dry-run] [--format json]
  orq guard --vault <path> [--format json]
  orq guard-collision --path <repo> [--format json]
  orq config [--config path] [--check-adapters] [--format json]
  orq vault-order --vault <path> --query <term> [--format json]
  orq delegate <task> [--format json]
  orq task create <title> [--tasks path] [--format json]
  orq task list [--tasks path] [--format json]
  orq task update <id> --state <state> [--agent name] [--model name] [--host name] [--evidence text] [--tasks path] [--format json]
  orq task assign <id> --agent name [--model name] [--host name] [--tasks path] [--format json]
  orq handoff draft --task-id <id> [--tasks path] [--output path] [--format json]
  orq agents [--format json]
`)
}

func cmdClassify(args []string) error {
	task := strings.TrimSpace(strings.Join(args, " "))
	if task == "" {
		return fmt.Errorf("task is required")
	}
	fmt.Println(route.Classify(task))
	return nil
}

func cmdRoute(args []string) error {
	format, remaining, err := extractStringFlag(args, "--format", "text")
	if err != nil {
		return err
	}
	task := strings.TrimSpace(strings.Join(remaining, " "))
	if task == "" {
		return fmt.Errorf("task is required")
	}
	decision := route.Decide(task)
	if format == "json" {
		return json.NewEncoder(os.Stdout).Encode(decision)
	}
	fmt.Printf("agent=%s model=%s level=%d category=%s reason=%s\n", decision.RecommendedAgent, decision.RecommendedModel, decision.RecommendedLevel, decision.Category, decision.Reason)
	return nil
}

func cmdRecord(args []string) error {
	fs := flag.NewFlagSet("record", flag.ContinueOnError)
	path := fs.String("ledger", ledger.DefaultPath(), "ledger path")
	task := fs.String("task", "", "task")
	agent := fs.String("agent", "", "agent")
	model := fs.String("model", "", "model")
	status := fs.String("status", "", "status")
	if err := fs.Parse(args); err != nil {
		return err
	}
	event := ledger.Event{Task: *task, Agent: *agent, Model: *model, Status: *status}
	if err := ledger.Append(*path, event); err != nil {
		return err
	}
	fmt.Println(*path)
	return nil
}

func cmdStatus(args []string) error {
	fs := flag.NewFlagSet("status", flag.ContinueOnError)
	path := fs.String("ledger", ledger.DefaultPath(), "ledger path")
	if err := fs.Parse(args); err != nil {
		return err
	}
	events, err := ledger.ReadAll(*path)
	if err != nil {
		return err
	}
	fmt.Printf("events=%d ledger=%s\n", len(events), *path)
	return nil
}

func cmdGuardCollision(args []string) error {
	format, remaining, err := extractStringFlag(args, "--format", "text")
	if err != nil {
		return err
	}
	path, remaining, err := extractStringFlag(remaining, "--path", "")
	if err != nil {
		return err
	}
	if path == "" {
		return fmt.Errorf("--path is required")
	}
	if len(remaining) > 0 {
		return fmt.Errorf("unexpected arguments: %s", strings.Join(remaining, " "))
	}
	status := guard.GitCollision(context.Background(), path)
	if format == "json" {
		return json.NewEncoder(os.Stdout).Encode(status)
	}
	fmt.Printf("path=%s branch=%s dirty=%t stop=%t reason=%s\n", status.Path, status.Branch, status.Dirty, status.ShouldStop, status.Reason)
	if status.ShouldStop {
		return fmt.Errorf("collision guard failed")
	}
	return nil
}

func cmdGuard(args []string) error {
	format, remaining, err := extractStringFlag(args, "--format", "text")
	if err != nil {
		return err
	}
	vault, remaining, err := extractStringFlag(remaining, "--vault", "")
	if err != nil {
		return err
	}
	if len(remaining) > 0 {
		return fmt.Errorf("unexpected arguments: %s", strings.Join(remaining, " "))
	}
	results := guard.AntiSplitBrain(vault)
	if format == "json" {
		if err := json.NewEncoder(os.Stdout).Encode(results); err != nil {
			return err
		}
	} else {
		for _, result := range results {
			status := "FAIL"
			if result.Passed {
				status = "OK"
			}
			fmt.Printf("%s %s %s\n", status, result.Name, result.Reason)
		}
	}
	if !guard.Passed(results) {
		return fmt.Errorf("guard failed")
	}
	return nil
}

func cmdAgents(args []string) error {
	format, remaining, err := extractStringFlag(args, "--format", "text")
	if err != nil {
		return err
	}
	if len(remaining) > 0 {
		return fmt.Errorf("unexpected arguments: %s", strings.Join(remaining, " "))
	}
	if format == "json" {
		return json.NewEncoder(os.Stdout).Encode(agentpkg.DefaultProfiles)
	}
	for _, profile := range agentpkg.DefaultProfiles {
		fmt.Printf("agent=%s model=%s cost=%d review_only=%t use_for=%s\n", profile.Agent, profile.Model, profile.CostLevel, profile.ReviewOnly, profile.UseFor)
	}
	return nil
}

func cmdHandoff(args []string) error {
	if len(args) < 1 {
		return fmt.Errorf("handoff subcommand is required")
	}
	format, remaining, err := extractStringFlag(args[1:], "--format", "text")
	if err != nil {
		return err
	}
	path, remaining, err := extractStringFlag(remaining, "--tasks", task.DefaultPath())
	if err != nil {
		return err
	}
	output, remaining, err := extractStringFlag(remaining, "--output", "")
	if err != nil {
		return err
	}
	switch args[0] {
	case "draft":
		id, remaining, err := extractStringFlag(remaining, "--task-id", "")
		if err != nil {
			return err
		}
		if id == "" {
			return fmt.Errorf("--task-id is required")
		}
		if len(remaining) > 0 {
			return fmt.Errorf("unexpected arguments: %s", strings.Join(remaining, " "))
		}
		item, err := task.Get(path, id)
		if err != nil {
			return err
		}
		draft := handoff.Draft(item)
		if output != "" {
			if _, err := os.Stat(output); err == nil {
				return fmt.Errorf("output already exists: %s", output)
			} else if !os.IsNotExist(err) {
				return err
			}
			if err := os.MkdirAll(filepath.Dir(output), 0o755); err != nil {
				return err
			}
			if err := os.WriteFile(output, []byte(draft), 0o644); err != nil {
				return err
			}
		}
		if format == "json" {
			return json.NewEncoder(os.Stdout).Encode(struct {
				TaskID string `json:"task_id"`
				Output string `json:"output,omitempty"`
				Draft  string `json:"draft"`
			}{TaskID: id, Output: output, Draft: draft})
		}
		fmt.Print(draft)
		return nil
	default:
		return fmt.Errorf("unknown handoff subcommand %q", args[0])
	}
}

func cmdTask(args []string) error {
	if len(args) < 1 {
		return fmt.Errorf("task subcommand is required")
	}
	format, remaining, err := extractStringFlag(args[1:], "--format", "text")
	if err != nil {
		return err
	}
	path, remaining, err := extractStringFlag(remaining, "--tasks", task.DefaultPath())
	if err != nil {
		return err
	}
	switch args[0] {
	case "create":
		title := strings.TrimSpace(strings.Join(remaining, " "))
		if title == "" {
			return fmt.Errorf("task title is required")
		}
		item, err := task.Create(path, title)
		if err != nil {
			return err
		}
		return printTask(item, format)
	case "list":
		if len(remaining) > 0 {
			return fmt.Errorf("unexpected arguments: %s", strings.Join(remaining, " "))
		}
		items, err := task.List(path)
		if err != nil {
			return err
		}
		if format == "json" {
			return json.NewEncoder(os.Stdout).Encode(items)
		}
		for _, item := range items {
			fmt.Printf("%s %s agent=%s model=%s host=%s title=%s\n", item.ID, item.State, item.Agent, item.Model, item.Host, item.Title)
		}
		return nil
	case "assign":
		agent, rem, err := extractStringFlag(remaining, "--agent", "")
		if err != nil {
			return err
		}
		model, rem, err := extractStringFlag(rem, "--model", "")
		if err != nil {
			return err
		}
		host, rem, err := extractStringFlag(rem, "--host", "")
		if err != nil {
			return err
		}
		if agent == "" {
			return fmt.Errorf("--agent is required")
		}
		if len(rem) != 1 {
			return fmt.Errorf("task id is required")
		}
		profile, err := agentpkg.Find(agent, model)
		if err != nil {
			return err
		}
		item, err := task.Assign(path, rem[0], agent, model, host, profile.CostLevel, profile.UseFor)
		if err != nil {
			return err
		}
		return printTask(item, format)
	case "update":
		state, rem, err := extractStringFlag(remaining, "--state", "")
		if err != nil {
			return err
		}
		agent, rem, err := extractStringFlag(rem, "--agent", "")
		if err != nil {
			return err
		}
		model, rem, err := extractStringFlag(rem, "--model", "")
		if err != nil {
			return err
		}
		host, rem, err := extractStringFlag(rem, "--host", "")
		if err != nil {
			return err
		}
		evidence, rem, err := extractStringFlag(rem, "--evidence", "")
		if err != nil {
			return err
		}
		if len(rem) != 1 {
			return fmt.Errorf("task id is required")
		}
		item, err := task.Update(path, rem[0], task.State(state), agent, model, host, evidence)
		if err != nil {
			return err
		}
		return printTask(item, format)
	default:
		return fmt.Errorf("unknown task subcommand %q", args[0])
	}
}

func printTask(item task.Item, format string) error {
	if format == "json" {
		return json.NewEncoder(os.Stdout).Encode(item)
	}
	fmt.Printf("%s %s agent=%s model=%s host=%s title=%s\n", item.ID, item.State, item.Agent, item.Model, item.Host, item.Title)
	return nil
}

func cmdDelegate(args []string) error {
	format, remaining, err := extractStringFlag(args, "--format", "text")
	if err != nil {
		return err
	}
	task := strings.TrimSpace(strings.Join(remaining, " "))
	if task == "" {
		return fmt.Errorf("task is required")
	}
	decision := route.Decide(task)
	prompt := delegate.Prompt(task, decision)
	if format == "json" {
		return json.NewEncoder(os.Stdout).Encode(struct {
			Decision route.Decision `json:"decision"`
			Prompt   string         `json:"prompt"`
		}{Decision: decision, Prompt: prompt})
	}
	fmt.Println(prompt)
	return nil
}

func cmdVaultOrder(args []string) error {
	format, remaining, err := extractStringFlag(args, "--format", "text")
	if err != nil {
		return err
	}
	vault, remaining, err := extractStringFlag(remaining, "--vault", "")
	if err != nil {
		return err
	}
	query, remaining, err := extractStringFlag(remaining, "--query", "")
	if err != nil {
		return err
	}
	if vault == "" {
		return fmt.Errorf("--vault is required")
	}
	if query == "" {
		return fmt.Errorf("--query is required")
	}
	if len(remaining) > 0 {
		return fmt.Errorf("unexpected arguments: %s", strings.Join(remaining, " "))
	}
	plan, err := vaultorder.Build(vault, query)
	if err != nil {
		return err
	}
	if format == "json" {
		return json.NewEncoder(os.Stdout).Encode(plan)
	}
	fmt.Printf("matches=%d directories=%d actions=%d\n", len(plan.Matches), len(plan.Directories), len(plan.Actions))
	for _, action := range plan.Actions {
		fmt.Printf("%s %s — %s\n", action.Type, action.Path, action.Reason)
	}
	return nil
}

func cmdConfig(args []string) error {
	checkAdapters, args := extractBoolFlag(args, "--check-adapters", false)
	format, remaining, err := extractStringFlag(args, "--format", "text")
	if err != nil {
		return err
	}
	path, remaining, err := extractStringFlag(remaining, "--config", "")
	if err != nil {
		return err
	}
	if len(remaining) > 0 {
		return fmt.Errorf("unexpected arguments: %s", strings.Join(remaining, " "))
	}
	cfg := config.Default()
	if path != "" {
		loaded, err := config.Load(path)
		if err != nil {
			return err
		}
		cfg = loaded
	}
	if checkAdapters {
		shell, err := config.ShellAdapter(cfg)
		if err != nil {
			return err
		}
		if _, err := config.GraphAdapter(cfg, shell); err != nil {
			return err
		}
		if _, err := config.MemoryAdapter(cfg); err != nil {
			return err
		}
	}
	if format == "json" {
		return json.NewEncoder(os.Stdout).Encode(cfg)
	}
	fmt.Printf("project=%s ssot=%s graph=%s memory=%s shell=%s mode=%s\n", cfg.Project.Name, cfg.SSoT.Type, cfg.Graph.Type, cfg.Memory.Type, cfg.Shell.Type, cfg.Policy.DefaultMode)
	return nil
}

func cmdRun(args []string) error {
	format, remaining, err := extractStringFlag(args, "--format", "text")
	if err != nil {
		return err
	}
	dryRun, remaining := extractBoolFlag(remaining, "--dry-run", true)
	task := strings.TrimSpace(strings.Join(remaining, " "))
	if task == "" {
		return fmt.Errorf("task is required")
	}
	decision := route.Decide(task)
	result := struct {
		Task     string         `json:"task"`
		Executed bool           `json:"executed"`
		DryRun   bool           `json:"dry_run"`
		Decision route.Decision `json:"decision"`
	}{Task: task, Executed: false, DryRun: dryRun, Decision: decision}
	if format == "json" {
		return json.NewEncoder(os.Stdout).Encode(result)
	}
	fmt.Printf("dry-run=%t executed=false agent=%s model=%s\n", dryRun, decision.RecommendedAgent, decision.RecommendedModel)
	return nil
}

func extractStringFlag(args []string, name, defaultValue string) (string, []string, error) {
	value := defaultValue
	remaining := make([]string, 0, len(args))
	for i := 0; i < len(args); i++ {
		arg := args[i]
		if arg == name {
			if i+1 >= len(args) {
				return "", nil, fmt.Errorf("%s requires a value", name)
			}
			value = args[i+1]
			i++
			continue
		}
		prefix := name + "="
		if strings.HasPrefix(arg, prefix) {
			value = strings.TrimPrefix(arg, prefix)
			continue
		}
		remaining = append(remaining, arg)
	}
	return value, remaining, nil
}

func extractBoolFlag(args []string, name string, defaultValue bool) (bool, []string) {
	value := defaultValue
	remaining := make([]string, 0, len(args))
	for _, arg := range args {
		if arg == name {
			value = true
			continue
		}
		remaining = append(remaining, arg)
	}
	return value, remaining
}
