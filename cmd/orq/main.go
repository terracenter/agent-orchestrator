package main

import (
	"encoding/json"
	"flag"
	"fmt"
	"os"
	"strings"

	"github.com/terracenter/agent-orchestrator/internal/config"
	"github.com/terracenter/agent-orchestrator/internal/delegate"
	"github.com/terracenter/agent-orchestrator/internal/guard"
	"github.com/terracenter/agent-orchestrator/internal/ledger"
	"github.com/terracenter/agent-orchestrator/internal/route"
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
	case "config":
		err = cmdConfig(os.Args[2:])
	case "vault-order":
		err = cmdVaultOrder(os.Args[2:])
	case "delegate":
		err = cmdDelegate(os.Args[2:])
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
  orq config [--config path] [--check-adapters] [--format json]
  orq vault-order --vault <path> --query <term> [--format json]
  orq delegate <task> [--format json]
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
