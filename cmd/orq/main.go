package main

import (
	"context"
	"encoding/json"
	"flag"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"time"

	agentpkg "github.com/terracenter/agent-orchestrator/internal/agent"
	"github.com/terracenter/agent-orchestrator/internal/audit"
	"github.com/terracenter/agent-orchestrator/internal/budget"
	"github.com/terracenter/agent-orchestrator/internal/config"
	"github.com/terracenter/agent-orchestrator/internal/delegate"
	"github.com/terracenter/agent-orchestrator/internal/doctor"
	"github.com/terracenter/agent-orchestrator/internal/guard"
	"github.com/terracenter/agent-orchestrator/internal/guides"
	"github.com/terracenter/agent-orchestrator/internal/handoff"
	"github.com/terracenter/agent-orchestrator/internal/heartbeat"
	"github.com/terracenter/agent-orchestrator/internal/inbox"
	"github.com/terracenter/agent-orchestrator/internal/ledger"
	"github.com/terracenter/agent-orchestrator/internal/observer"
	"github.com/terracenter/agent-orchestrator/internal/receipt"
	"github.com/terracenter/agent-orchestrator/internal/repostandard"
	"github.com/terracenter/agent-orchestrator/internal/review4r"
	"github.com/terracenter/agent-orchestrator/internal/route"
	"github.com/terracenter/agent-orchestrator/internal/safety"
	"github.com/terracenter/agent-orchestrator/internal/session"
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
	case "docs":
		err = cmdDocs(os.Args[2:])
	case "task":
		err = cmdTask(os.Args[2:])
	case "handoff":
		err = cmdHandoff(os.Args[2:])
	case "heartbeat":
		err = cmdHeartbeat(os.Args[2:])
	case "inbox":
		err = cmdInbox(os.Args[2:])
	case "agents":
		err = cmdAgents(os.Args[2:])
	case "audit":
		err = cmdAudit(os.Args[2:])
	case "observer":
		err = cmdObserver(os.Args[2:])
	case "receipt":
		err = cmdReceipt(os.Args[2:])
	case "session":
		err = cmdSession(os.Args[2:])
	case "budget":
		err = cmdBudget(os.Args[2:])
	case "repo":
		err = cmdRepo(os.Args[2:])
	case "safety":
		err = cmdSafety(os.Args[2:])
	case "review":
		err = cmdReview(os.Args[2:])
	case "doctor":
		err = cmdDoctor(os.Args[2:])
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
  orq delegate <task> [--agent pi|claude|codex|hermes|agy] [--model <model>] [--handoff <path>] [--write-handoff <path>] [--write-receipt <path>] [--force] [--repo <path>] [--agents-dir <path>] [--workspace <path>] [--executed] [--format json]
  orq docs usage|orchestration
  orq task create <title> [--tasks path] [--format json]
  orq task list [--tasks path] [--format json]
  orq task update <id> --state <state> [--agent name] [--model name] [--host name] [--evidence text] [--tasks path] [--format json]
  orq task assign <id> --agent name [--model name] [--host name] [--tasks path] [--format json]
  orq handoff draft --task-id <id> [--template reviewer-4r|security-reviewer|implementer|documenter|architect] [--tasks path] [--output path] [--format json]
  orq handoff validate-template --file path [--format json]
  orq handoff chain --from path --to path --task text --next-agent name [--format json]
  orq heartbeat run [--workspace path] [--format json]
  orq inbox feedbacks [--path dir] [--format json]
  orq inbox next [--path dir] [--seen-file path] [--format json]
  orq inbox ack --file path [--seen-file path]
  orq agents [--format json]
  orq agents detect [--format json]
  orq doctor [--format json]
  orq audit prs [--path repo] [--format json]
  orq audit issues [--path repo] [--format json]
  orq audit models [--format json]
  orq audit worktrees [--path repo] [--format json]
  orq observer send-test [--project name] [--agent name] [--model name] [--tokens-in n] [--tokens-out n] [--format json]
  orq observer cost --reported-estimate-usd n --reported-label text --monthly-plan-usd n [--payment-fee-usd n] [--format json]
  orq receipt create --task text --command text [--command text ...] --evidence text --rollback text [--command-result passed|failed|skipped|recorded ...] [--human-edits-required-value unknown|N] [--human-edits-required] [--correcciones-humanas-requeridas] [--human-edits-notes text] [--output path] [--agent name] [--provider name] [--model name] [--risk bajo|medio|alto] [--pr n] [--files a,b] [--security-notes a,b]
  orq receipt verify --path receipt.json [--format json]
  orq receipt from-pr --pr N [--output path] [--agent name] [--provider name] [--model name] [--risk bajo|medio|alto]
  orq session validate --guard-collision text --repo-check text --safety-check text --tests text --receipt text [--handoff text] [--touches-dangerous] [--human-approval] [--format json]
  orq budget --context-percent n --codex-5h-percent n [--weekly-percent n] [--agent pi|claude|codex|hermes|agy] [--compact-applied] [--format json]
  orq repo check [--path repo] [--format json]
  orq repo init-template --path repo [--name project] [--format json]
  orq safety check [--path repo] [--command text] [--format json]
  orq review 4r [--path repo] [--format json]
`)
}

func cmdDocs(args []string) error {
	if len(args) != 1 {
		return fmt.Errorf("docs guide is required: %s", strings.Join(guides.Names(), "|"))
	}
	text, err := guides.Text(args[0])
	if err != nil {
		return err
	}
	fmt.Print(text)
	return nil
}

func cmdSession(args []string) error {
	if len(args) < 1 {
		return fmt.Errorf("session subcommand is required")
	}
	format, remaining, err := extractStringFlag(args[1:], "--format", "text")
	if err != nil {
		return err
	}
	switch args[0] {
	case "validate":
		guardCollision, remaining, err := extractStringFlag(remaining, "--guard-collision", "")
		if err != nil {
			return err
		}
		repoCheck, remaining, err := extractStringFlag(remaining, "--repo-check", "")
		if err != nil {
			return err
		}
		safetyCheck, remaining, err := extractStringFlag(remaining, "--safety-check", "")
		if err != nil {
			return err
		}
		tests, remaining, err := extractStringFlag(remaining, "--tests", "")
		if err != nil {
			return err
		}
		receiptText, remaining, err := extractStringFlag(remaining, "--receipt", "")
		if err != nil {
			return err
		}
		handoffText, remaining, err := extractStringFlag(remaining, "--handoff", "")
		if err != nil {
			return err
		}
		touchesDangerous, remaining := extractBoolFlag(remaining, "--touches-dangerous", false)
		humanApproval, remaining := extractBoolFlag(remaining, "--human-approval", false)
		if len(remaining) > 0 {
			return fmt.Errorf("unexpected arguments: %s", strings.Join(remaining, " "))
		}
		report := session.Validate(session.Input{RepoCheck: repoCheck, SafetyCheck: safetyCheck, GuardCollision: guardCollision, Tests: tests, Receipt: receiptText, Handoff: handoffText, TouchesDangerous: touchesDangerous, HumanApproval: humanApproval})
		if format == "json" {
			return json.NewEncoder(os.Stdout).Encode(report)
		}
		status := "OK"
		if !report.Valid {
			status = "BLOCKED"
		}
		fmt.Printf("%s checks=%d findings=%d\n", status, len(report.Checks), len(report.Findings))
		for _, finding := range report.Findings {
			fmt.Printf("finding severity=%s message=%q\n", finding.Severity, finding.Message)
		}
		return nil
	default:
		return fmt.Errorf("unknown session subcommand %q", args[0])
	}
}

func cmdSafety(args []string) error {
	if len(args) == 0 {
		return fmt.Errorf("safety subcommand is required")
	}
	switch args[0] {
	case "check":
		format, remaining, err := extractStringFlag(args[1:], "--format", "text")
		if err != nil {
			return err
		}
		path, remaining, err := extractStringFlag(remaining, "--path", ".")
		if err != nil {
			return err
		}
		command, remaining, err := extractStringFlag(remaining, "--command", "")
		if err != nil {
			return err
		}
		if len(remaining) > 0 {
			return fmt.Errorf("unexpected arguments: %s", strings.Join(remaining, " "))
		}
		report := safety.CheckRepo(path)
		if command != "" {
			if bad, reason := safety.UnsafeCommand(command); bad {
				report.Passed = false
				report.Risk = "alto"
				report.Findings = append(report.Findings, safety.Finding{Level: "alto", Reason: reason})
			}
		}
		if format == "json" {
			if err := json.NewEncoder(os.Stdout).Encode(report); err != nil {
				return err
			}
		} else {
			status := "OK"
			if !report.Passed {
				status = "BLOCKED"
			}
			fmt.Printf("%s repo=%s risk=%s findings=%d\n", status, report.Root, report.Risk, len(report.Findings))
			for _, finding := range report.Findings {
				fmt.Printf("%s path=%s reason=%s\n", finding.Level, finding.Path, finding.Reason)
			}
		}
		if !report.Passed {
			return fmt.Errorf("safety check failed")
		}
		return nil
	default:
		return fmt.Errorf("unknown safety subcommand %q", args[0])
	}
}

func cmdAudit(args []string) error {
	if len(args) == 0 {
		return fmt.Errorf("audit subcommand is required")
	}
	switch args[0] {
	case "worktrees":
		format, remaining, err := extractStringFlag(args[1:], "--format", "text")
		if err != nil {
			return err
		}
		path, remaining, err := extractStringFlag(remaining, "--path", ".")
		if err != nil {
			return err
		}
		if len(remaining) > 0 {
			return fmt.Errorf("unexpected arguments: %s", strings.Join(remaining, " "))
		}
		report, err := audit.AuditWorktrees(path)
		if err != nil {
			return err
		}
		if format == "json" {
			return json.NewEncoder(os.Stdout).Encode(report)
		}
		status := "OK"
		if len(report.Findings) > 0 {
			status = "REVIEW"
		}
		fmt.Printf("%s root=%s worktrees=%d findings=%d\n", status, report.Root, len(report.Worktrees), len(report.Findings))
		for _, wt := range report.Worktrees {
			branch := wt.Branch
			if branch == "" && wt.Detached {
				branch = "detached"
			}
			fmt.Printf("worktree path=%s branch=%s prunable=%t\n", wt.Path, branch, wt.Prunable)
		}
		for _, finding := range report.Findings {
			fmt.Printf("finding=%s\n", finding)
		}
		return nil
	case "issues":
		format, remaining, err := extractStringFlag(args[1:], "--format", "text")
		if err != nil {
			return err
		}
		path, remaining, err := extractStringFlag(remaining, "--path", ".")
		if err != nil {
			return err
		}
		if len(remaining) > 0 {
			return fmt.Errorf("unexpected arguments: %s", strings.Join(remaining, " "))
		}
		report, err := audit.AuditIssues(path)
		if err != nil {
			return err
		}
		if format == "json" {
			return json.NewEncoder(os.Stdout).Encode(report)
		}
		fmt.Printf("repo=%s open_issues=%d findings=%d\n", report.Repository, len(report.Issues), len(report.Findings))
		for _, finding := range report.Findings {
			fmt.Printf("finding=%s\n", finding)
		}
		return nil
	case "models":
		format, remaining, err := extractStringFlag(args[1:], "--format", "text")
		if err != nil {
			return err
		}
		if len(remaining) > 0 {
			return fmt.Errorf("unexpected arguments: %s", strings.Join(remaining, " "))
		}
		report := audit.AuditModels()
		if format == "json" {
			return json.NewEncoder(os.Stdout).Encode(report)
		}
		fmt.Printf("models=%d findings=%d\n", len(report.Models), len(report.Findings))
		for _, model := range report.Models {
			assignable := "assignable"
			if !model.Assignable {
				assignable = "not_assignable"
			}
			fmt.Printf("%s agent=%s provider=%s model=%s verified=%t cost=%d reason=%q\n", assignable, model.Agent, model.Provider, model.Model, model.Verified, model.CostLevel, model.Reason)
		}
		return nil
	case "prs":
		format, remaining, err := extractStringFlag(args[1:], "--format", "text")
		if err != nil {
			return err
		}
		path, remaining, err := extractStringFlag(remaining, "--path", ".")
		if err != nil {
			return err
		}
		if len(remaining) > 0 {
			return fmt.Errorf("unexpected arguments: %s", strings.Join(remaining, " "))
		}
		report, err := audit.AuditPullRequests(path)
		if err != nil {
			return err
		}
		if format == "json" {
			return json.NewEncoder(os.Stdout).Encode(report)
		}
		fmt.Printf("repo=%s open_prs=%d\n", report.Repository, len(report.Pulls))
		for _, pr := range report.Pulls {
			state := "OK"
			if pr.Blocked {
				state = "BLOCKED"
			}
			fmt.Printf("%s #%d %s mergeable=%s review=%s checks=%d\n", state, pr.Number, pr.Title, pr.Mergeable, pr.ReviewDecision, len(pr.Checks))
			for _, blocker := range pr.Blockers {
				fmt.Printf("  blocker=%s\n", blocker)
			}
		}
		return nil
	default:
		return fmt.Errorf("unknown audit subcommand %q", args[0])
	}
}

func cmdReview(args []string) error {
	if len(args) == 0 {
		return fmt.Errorf("review subcommand is required")
	}
	switch args[0] {
	case "4r":
		format, remaining, err := extractStringFlag(args[1:], "--format", "text")
		if err != nil {
			return err
		}
		path, remaining, err := extractStringFlag(remaining, "--path", ".")
		if err != nil {
			return err
		}
		if len(remaining) > 0 {
			return fmt.Errorf("unexpected arguments: %s", strings.Join(remaining, " "))
		}
		report := review4r.Build(path)
		if format == "json" {
			return json.NewEncoder(os.Stdout).Encode(report)
		}
		fmt.Printf("repo=%s changed_files=%d\n", report.Root, len(report.ChangedFiles))
		for _, item := range report.Items {
			fmt.Printf("- %s: %s\n", item.Area, item.Question)
			if len(item.Focus) > 0 {
				fmt.Printf("  foco=%s\n", strings.Join(item.Focus, ","))
			}
		}
		return nil
	default:
		return fmt.Errorf("unknown review subcommand %q", args[0])
	}
}

func cmdRepo(args []string) error {
	if len(args) == 0 {
		return fmt.Errorf("repo subcommand is required")
	}
	switch args[0] {
	case "init-template":
		format, remaining, err := extractStringFlag(args[1:], "--format", "text")
		if err != nil {
			return err
		}
		path, remaining, err := extractStringFlag(remaining, "--path", "")
		if err != nil {
			return err
		}
		name, remaining, err := extractStringFlag(remaining, "--name", "")
		if err != nil {
			return err
		}
		if path == "" {
			return fmt.Errorf("--path is required")
		}
		if len(remaining) > 0 {
			return fmt.Errorf("unexpected arguments: %s", strings.Join(remaining, " "))
		}
		result, err := repostandard.InitRepo(path, repostandard.TemplateData{ProjectName: name})
		if err != nil {
			return err
		}
		if format == "json" {
			return json.NewEncoder(os.Stdout).Encode(result)
		}
		fmt.Printf("repo=%s created=%d skipped=%d\n", result.Root, len(result.Created), len(result.Skipped))
		return nil
	case "check":
		format, remaining, err := extractStringFlag(args[1:], "--format", "text")
		if err != nil {
			return err
		}
		path, remaining, err := extractStringFlag(remaining, "--path", ".")
		if err != nil {
			return err
		}
		if len(remaining) > 0 {
			return fmt.Errorf("unexpected arguments: %s", strings.Join(remaining, " "))
		}
		report := repostandard.CheckRepo(path)
		if format == "json" {
			return json.NewEncoder(os.Stdout).Encode(report)
		}
		status := "OK"
		if !report.Passed {
			status = "FAIL"
		}
		fmt.Printf("%s repo=%s\n", status, report.Root)
		for _, check := range report.Checks {
			mark := "OK"
			if !check.Passed {
				if check.Required {
					mark = "FAIL"
				} else {
					mark = "WARN"
				}
			}
			fmt.Printf("%s %s path=%s reason=%s\n", mark, check.Name, check.Path, check.Reason)
		}
		if !report.Passed {
			return fmt.Errorf("repo standard check failed")
		}
		return nil
	default:
		return fmt.Errorf("unknown repo subcommand %q", args[0])
	}
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
	emitObserverRecord(event)
	fmt.Println(*path)
	return nil
}

func emitObserverRecord(event ledger.Event) {
	client, ok, err := observer.FromEnv()
	if err != nil || !ok {
		return
	}
	raw, _ := json.Marshal(map[string]string{"task": event.Task, "status": event.Status})
	obsEvent := observer.NewEvent("agent-orchestrator", event.Agent, event.Model, "orq-ledger", "orq_record", 0, 0, "orq record", string(raw))
	ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
	defer cancel()
	if _, err := client.Ingest(ctx, []observer.Event{obsEvent}); err != nil {
		fmt.Fprintf(os.Stderr, "warning: observer ingest failed: %v\n", err)
	}
}

func emitReceiptObserver(r receipt.Receipt, sourcePath string) {
	client, ok, err := observer.FromEnv()
	if err != nil || !ok {
		return
	}
	raw, _ := json.Marshal(map[string]any{"task": r.Task, "pr": r.PR, "human_edits_required": r.HumanEditsRequired, "human_edits_required_value": r.HumanEditsRequiredValue, "correcciones_humanas_requeridas": r.CorreccionesHumanasRequeridas})
	obsEvent := observer.NewEvent("agent-orchestrator", r.Agent, r.Model, "orq-receipt", "orq_receipt", 0, 0, sourcePath, string(raw))
	ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
	defer cancel()
	if _, err := client.Ingest(ctx, []observer.Event{obsEvent}); err != nil {
		fmt.Fprintf(os.Stderr, "warning: observer ingest failed: %v\n", err)
	}
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
	if len(remaining) > 0 && remaining[0] == "detect" {
		if len(remaining) > 1 {
			return fmt.Errorf("unexpected arguments after detect: %s", strings.Join(remaining[1:], " "))
		}
		detections := agentpkg.DetectAgents()
		if format == "json" {
			return json.NewEncoder(os.Stdout).Encode(detections)
		}
		for _, d := range detections {
			fmt.Printf("agent=%s installed=%t binary=%s config=%s cost=%d verified=%t review_only=%t role=%s\n",
				d.Agent, d.Installed, d.BinaryPath, d.ConfigPath, d.CostLevel, d.Verified, d.ReviewOnly, d.Role)
			if d.Notes != "" {
				fmt.Printf("  notes=%s\n", d.Notes)
			}
		}
		return nil
	}
	if len(remaining) > 0 {
		return fmt.Errorf("unexpected arguments: %s", strings.Join(remaining, " "))
	}
	if format == "json" {
		return json.NewEncoder(os.Stdout).Encode(agentpkg.DefaultProfiles)
	}
	for _, profile := range agentpkg.DefaultProfiles {
		fmt.Printf("agent=%s provider=%s model=%s cost=%d verified=%t review_only=%t use_for=%s\n", profile.Agent, profile.Provider, profile.Model, profile.CostLevel, profile.Verified, profile.ReviewOnly, profile.UseFor)
	}
	return nil
}

func cmdDoctor(args []string) error {
	format, remaining, err := extractStringFlag(args, "--format", "text")
	if err != nil {
		return err
	}
	if len(remaining) > 0 {
		return fmt.Errorf("unexpected arguments: %s", strings.Join(remaining, " "))
	}
	report := doctor.Run(context.Background(), doctor.Options{})
	if format == "json" {
		return json.NewEncoder(os.Stdout).Encode(report)
	}

	fmt.Printf("doctor status=%s total=%d ok=%d missing=%d degraded=%d\n",
		report.Status, report.Summary.Total, report.Summary.OK, report.Summary.Missing, report.Summary.Degraded)

	var currentCat doctor.Category = ""
	for _, check := range report.Tools {
		if check.Category != currentCat {
			currentCat = check.Category
			fmt.Printf("\n[%s]\n", currentCat)
		}
		statusTag := string(check.Status)
		if check.Required {
			statusTag += " (required)"
		}
		details := ""
		if check.Path != "" {
			details = check.Path
		}
		if check.ConfigPath != "" {
			if details != "" {
				details += ", config: " + check.ConfigPath
			} else {
				details = "config: " + check.ConfigPath
			}
		}
		if check.Version != "" {
			if details != "" {
				details += ", " + check.Version
			} else {
				details = check.Version
			}
		}
		if check.Note != "" {
			if details != "" {
				details += " (" + check.Note + ")"
			} else {
				details = check.Note
			}
		}
		if details != "" {
			fmt.Printf("  %-10s : %s [%s]\n", check.Name, statusTag, details)
		} else {
			fmt.Printf("  %-10s : %s\n", check.Name, statusTag)
		}
		if check.Recommendation != "" && check.Status != doctor.StatusOK {
			fmt.Printf("    recommendation: %s\n", check.Recommendation)
		}
	}
	return nil
}

func cmdBudget(args []string) error {
	format, remaining, err := extractStringFlag(args, "--format", "text")
	if err != nil {
		return err
	}
	contextText, remaining, err := extractStringFlag(remaining, "--context-percent", "")
	if err != nil {
		return err
	}
	codexText, remaining, err := extractStringFlag(remaining, "--codex-5h-percent", "")
	if err != nil {
		return err
	}
	weeklyText, remaining, err := extractStringFlag(remaining, "--weekly-percent", "0")
	if err != nil {
		return err
	}
	agentName, remaining, err := extractStringFlag(remaining, "--agent", "unknown")
	if err != nil {
		return err
	}
	compactApplied, remaining := extractBoolFlag(remaining, "--compact-applied", false)
	if len(remaining) > 0 {
		return fmt.Errorf("unexpected arguments: %s", strings.Join(remaining, " "))
	}
	if contextText == "" || codexText == "" {
		return fmt.Errorf("--context-percent and --codex-5h-percent are required")
	}
	contextPercent, err := parseFloatFlag("--context-percent", contextText)
	if err != nil {
		return err
	}
	codexPercent, err := parseFloatFlag("--codex-5h-percent", codexText)
	if err != nil {
		return err
	}
	weeklyPercent, err := parseFloatFlag("--weekly-percent", weeklyText)
	if err != nil {
		return err
	}
	for name, value := range map[string]float64{"--context-percent": contextPercent, "--codex-5h-percent": codexPercent, "--weekly-percent": weeklyPercent} {
		if err := budget.ValidatePercent(name, value); err != nil {
			return err
		}
	}
	advice := budget.DecideForAgentWithCompactApplied(contextPercent, codexPercent, weeklyPercent, agentName, compactApplied)
	if format == "json" {
		return json.NewEncoder(os.Stdout).Encode(advice)
	}
	fmt.Printf("action=%s preflight_compact_required=%t manual_compact_stop=%t compact_applied=%t must_stop_for_delegation=%t supervisor_only=%t execution_agent_allowed=%t reason=%s\n", advice.Action, advice.PreflightCompactRequired, advice.ManualCompactStop, advice.CompactApplied, advice.MustStopForDelegation, advice.SupervisorOnly, advice.ExecutionAgentAllowed, advice.Reason)
	if advice.CompactPrompt != "" {
		fmt.Println(advice.CompactPrompt)
	}
	if advice.CompactInstruction != "" {
		fmt.Println(advice.CompactInstruction)
	}
	fmt.Printf("use=%s avoid=%s\n", strings.Join(advice.UseAgents, ","), strings.Join(advice.AvoidAgents, ","))
	return nil
}

func cmdReceipt(args []string) error {
	if len(args) == 0 {
		return fmt.Errorf("receipt subcommand is required")
	}
	switch args[0] {
	case "create":
		task, remaining, err := extractStringFlag(args[1:], "--task", "")
		if err != nil {
			return err
		}
		agent, remaining, err := extractStringFlag(remaining, "--agent", "orq")
		if err != nil {
			return err
		}
		provider, remaining, err := extractStringFlag(remaining, "--provider", "manual")
		if err != nil {
			return err
		}
		model, remaining, err := extractStringFlag(remaining, "--model", "unknown")
		if err != nil {
			return err
		}
		risk, remaining, err := extractStringFlag(remaining, "--risk", "bajo")
		if err != nil {
			return err
		}
		commands, remaining, err := extractStringFlags(remaining, "--command")
		if err != nil {
			return err
		}
		commandResults, remaining, err := extractStringFlags(remaining, "--command-result")
		if err != nil {
			return err
		}
		if len(commandResults) == 0 {
			commandResults = []string{"recorded"}
		}
		for _, commandResult := range commandResults {
			if !receipt.ValidCommandResult(commandResult) {
				return fmt.Errorf("invalid --command-result %q: use passed, failed, skipped or recorded", commandResult)
			}
		}
		evidence, remaining, err := extractStringFlag(remaining, "--evidence", "")
		if err != nil {
			return err
		}
		rollback, remaining, err := extractStringFlag(remaining, "--rollback", "")
		if err != nil {
			return err
		}
		output, remaining, err := extractStringFlag(remaining, "--output", "receipt.json")
		if err != nil {
			return err
		}
		files, remaining, err := extractStringFlag(remaining, "--files", "")
		if err != nil {
			return err
		}
		securityNotes, remaining, err := extractStringFlag(remaining, "--security-notes", "")
		if err != nil {
			return err
		}
		humanEditsNotes, remaining, err := extractStringFlag(remaining, "--human-edits-notes", "")
		if err != nil {
			return err
		}
		humanEditsRequiredValue, remaining, err := extractStringFlag(remaining, "--human-edits-required-value", "unknown")
		if err != nil {
			return err
		}
		if !receipt.ValidHumanEditsRequiredValue(humanEditsRequiredValue) {
			return fmt.Errorf("invalid --human-edits-required-value: use unknown or non-negative integer")
		}
		humanEditsRequired, remaining := extractBoolFlag(remaining, "--human-edits-required", false)
		correccionesHumanasRequeridas, remaining := extractBoolFlag(remaining, "--correcciones-humanas-requeridas", false)
		if humanEditsRequired != correccionesHumanasRequeridas {
			return fmt.Errorf("--human-edits-required and --correcciones-humanas-requeridas must be used together")
		}
		prText, remaining, err := extractStringFlag(remaining, "--pr", "0")
		if err != nil {
			return err
		}
		pr, err := strconv.Atoi(prText)
		if err != nil {
			return fmt.Errorf("invalid --pr: %w", err)
		}
		if len(remaining) > 0 {
			return fmt.Errorf("unexpected arguments: %s", strings.Join(remaining, " "))
		}
		r := receipt.New(task, agent, provider, model, risk, pr)
		r.FilesChanged = splitCSV(files)
		builtCommands, err := buildReceiptCommands(commands, commandResults)
		if err != nil {
			return err
		}
		r.Commands = builtCommands
		r.Evidence = splitCSV(evidence)
		r.SecurityNotes = splitCSV(securityNotes)
		r.HumanEditsRequired = humanEditsRequired
		r.HumanEditsRequiredValue = humanEditsRequiredValue
		r.CorreccionesHumanasRequeridas = correccionesHumanasRequeridas
		r.HumanEditsNotes = splitCSV(humanEditsNotes)
		r.Rollback = rollback
		if findings := receipt.Verify(r); len(findings) > 0 {
			return fmt.Errorf("invalid receipt: %s", strings.Join(findings, "; "))
		}
		if err := receipt.Save(output, r); err != nil {
			return err
		}
		emitReceiptObserver(r, output)
		fmt.Printf("receipt path=%s task=%s risk=%s evidence=%d human_edits_required=%t human_edits_required_value=%s\n", output, r.Task, r.Risk, len(r.Evidence), r.HumanEditsRequired, r.HumanEditsRequiredValue)
		return nil
	case "from-pr":
		output, remaining, err := extractStringFlag(args[1:], "--output", "receipt.json")
		if err != nil {
			return err
		}
		agent, remaining, err := extractStringFlag(remaining, "--agent", "orq")
		if err != nil {
			return err
		}
		provider, remaining, err := extractStringFlag(remaining, "--provider", "manual")
		if err != nil {
			return err
		}
		model, remaining, err := extractStringFlag(remaining, "--model", "unknown")
		if err != nil {
			return err
		}
		risk, remaining, err := extractStringFlag(remaining, "--risk", "bajo")
		if err != nil {
			return err
		}
		prText, remaining, err := extractStringFlag(remaining, "--pr", "")
		if err != nil {
			return err
		}
		if prText == "" {
			return fmt.Errorf("--pr is required")
		}
		pr, err := strconv.Atoi(prText)
		if err != nil {
			return fmt.Errorf("invalid --pr: %w", err)
		}
		if len(remaining) > 0 {
			return fmt.Errorf("unexpected arguments: %s", strings.Join(remaining, " "))
		}
		info, err := loadPRInfo(pr)
		if err != nil {
			return err
		}
		r := receipt.FromPR(info, agent, provider, model, risk)
		if findings := receipt.Verify(r); len(findings) > 0 {
			return fmt.Errorf("invalid receipt from PR: %s", strings.Join(findings, "; "))
		}
		if err := receipt.Save(output, r); err != nil {
			return err
		}
		fmt.Printf("receipt path=%s pr=%d files=%d evidence=%d\n", output, r.PR, len(r.FilesChanged), len(r.Evidence))
		return nil
	case "verify":
		format, remaining, err := extractStringFlag(args[1:], "--format", "text")
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
		r, err := receipt.Load(path)
		if err != nil {
			return err
		}
		findings := receipt.Verify(r)
		if format == "json" {
			return json.NewEncoder(os.Stdout).Encode(map[string]any{"valid": len(findings) == 0, "findings": findings, "receipt": r})
		}
		if len(findings) > 0 {
			fmt.Printf("INVALID path=%s findings=%d\n", path, len(findings))
			for _, finding := range findings {
				fmt.Printf("finding=%s\n", finding)
			}
			return fmt.Errorf("receipt verification failed")
		}
		fmt.Printf("OK path=%s task=%s risk=%s evidence=%d\n", path, r.Task, r.Risk, len(r.Evidence))
		return nil
	default:
		return fmt.Errorf("unknown receipt subcommand %q", args[0])
	}
}

type ghPRView struct {
	Number      int    `json:"number"`
	Title       string `json:"title"`
	URL         string `json:"url"`
	HeadRefName string `json:"headRefName"`
	BaseRefName string `json:"baseRefName"`
	MergeCommit struct {
		OID string `json:"oid"`
	} `json:"mergeCommit"`
	Files []struct {
		Path string `json:"path"`
	} `json:"files"`
	StatusCheckRollup []struct {
		Name       string `json:"name"`
		Status     string `json:"status"`
		Conclusion string `json:"conclusion"`
	} `json:"statusCheckRollup"`
}

func loadPRInfo(pr int) (receipt.PRInfo, error) {
	cmd := exec.Command("gh", "pr", "view", strconv.Itoa(pr), "--json", "number,title,url,headRefName,baseRefName,mergeCommit,files,statusCheckRollup")
	data, err := cmd.Output()
	if err != nil {
		return receipt.PRInfo{}, fmt.Errorf("gh pr view failed: %w", err)
	}
	var view ghPRView
	if err := json.Unmarshal(data, &view); err != nil {
		return receipt.PRInfo{}, err
	}
	info := receipt.PRInfo{Number: view.Number, Title: view.Title, URL: view.URL, HeadRef: view.HeadRefName, BaseRef: view.BaseRefName, MergeCommit: view.MergeCommit.OID}
	for _, file := range view.Files {
		if strings.TrimSpace(file.Path) != "" {
			info.Files = append(info.Files, file.Path)
		}
	}
	for _, check := range view.StatusCheckRollup {
		status := strings.TrimSpace(check.Conclusion)
		if status == "" {
			status = check.Status
		}
		if strings.TrimSpace(check.Name) != "" {
			info.Checks = append(info.Checks, check.Name+" "+status)
		}
	}
	if len(info.Checks) == 0 {
		info.Checks = append(info.Checks, "sin checks reportados")
	}
	return info, nil
}

func splitCSV(value string) []string {
	if strings.TrimSpace(value) == "" {
		return nil
	}
	var values []string
	for _, part := range strings.Split(value, ",") {
		part = strings.TrimSpace(part)
		if part != "" {
			values = append(values, part)
		}
	}
	return values
}

func cmdObserver(args []string) error {
	if len(args) < 1 {
		return fmt.Errorf("observer subcommand is required")
	}
	format, remaining, err := extractStringFlag(args[1:], "--format", "text")
	if err != nil {
		return err
	}
	switch args[0] {
	case "send-test":
		project, remaining, err := extractStringFlag(remaining, "--project", "agent-orchestrator")
		if err != nil {
			return err
		}
		agent, remaining, err := extractStringFlag(remaining, "--agent", "nvidia-api")
		if err != nil {
			return err
		}
		model, remaining, err := extractStringFlag(remaining, "--model", "openai/gpt-oss-20b")
		if err != nil {
			return err
		}
		tokensInText, remaining, err := extractStringFlag(remaining, "--tokens-in", "100")
		if err != nil {
			return err
		}
		tokensOutText, remaining, err := extractStringFlag(remaining, "--tokens-out", "20")
		if err != nil {
			return err
		}
		if len(remaining) > 0 {
			return fmt.Errorf("unexpected arguments: %s", strings.Join(remaining, " "))
		}
		tokensIn, err := parseInt64Flag("--tokens-in", tokensInText)
		if err != nil {
			return err
		}
		tokensOut, err := parseInt64Flag("--tokens-out", tokensOutText)
		if err != nil {
			return err
		}
		client, ok, err := observer.FromEnv()
		if err != nil {
			return err
		}
		if !ok {
			return fmt.Errorf("observer token not configured; set ORQ_OBSERVER_HOST_TOKEN or ORQ_OBSERVER_HOST_TOKEN_FILE")
		}
		event := observer.SyntheticEvent(project, agent, model, tokensIn, tokensOut)
		result, err := client.Ingest(context.Background(), []observer.Event{event})
		if err != nil {
			return err
		}
		if format == "json" {
			return json.NewEncoder(os.Stdout).Encode(struct {
				Event  observer.Event        `json:"event"`
				Result observer.IngestResult `json:"result"`
			}{Event: event, Result: result})
		}
		fmt.Printf("observer=ok inserted=%d event_id=%s project=%s agent=%s model=%s\n", result.Inserted, event.EventID, event.Project, event.Agent, event.Model)
		return nil
	case "cost":
		reportedText, remaining, err := extractStringFlag(remaining, "--reported-estimate-usd", "0")
		if err != nil {
			return err
		}
		label, remaining, err := extractStringFlag(remaining, "--reported-label", "")
		if err != nil {
			return err
		}
		planText, remaining, err := extractStringFlag(remaining, "--monthly-plan-usd", "0")
		if err != nil {
			return err
		}
		feeText, remaining, err := extractStringFlag(remaining, "--payment-fee-usd", "0")
		if err != nil {
			return err
		}
		if len(remaining) > 0 {
			return fmt.Errorf("unexpected arguments: %s", strings.Join(remaining, " "))
		}
		reported, err := parseFloatFlag("--reported-estimate-usd", reportedText)
		if err != nil {
			return err
		}
		plan, err := parseFloatFlag("--monthly-plan-usd", planText)
		if err != nil {
			return err
		}
		fee, err := parseFloatFlag("--payment-fee-usd", feeText)
		if err != nil {
			return err
		}
		report := observer.InterpretCost(reported, label, plan, fee)
		if format == "json" {
			return json.NewEncoder(os.Stdout).Encode(report)
		}
		fmt.Printf("reported_estimate_usd=%.3f label=%s expected_invoice_usd=%.2f billable_real_usd=%.2f\n", report.ReportedEstimateUSD, report.ReportedLabel, report.ExpectedInvoiceUSD, report.BillableRealUSD)
		if report.Warning != "" {
			fmt.Println("warning=" + report.Warning)
		}
		return nil
	default:
		return fmt.Errorf("unknown observer subcommand %q", args[0])
	}
}

func cmdHeartbeat(args []string) error {
	if len(args) < 1 {
		return fmt.Errorf("heartbeat subcommand is required")
	}
	switch args[0] {
	case "run":
		format, remaining, err := extractStringFlag(args[1:], "--format", "text")
		if err != nil {
			return err
		}
		workspace, remaining, err := extractStringFlag(remaining, "--workspace", "/home/freddy/Workspace")
		if err != nil {
			return err
		}
		if len(remaining) > 0 {
			return fmt.Errorf("unexpected arguments: %s", strings.Join(remaining, " "))
		}
		report, err := heartbeat.Run(workspace)
		if err != nil {
			return err
		}
		if format == "json" {
			return json.NewEncoder(os.Stdout).Encode(report)
		}
		fmt.Printf("heartbeat workspace=%s projects=%d sources=%d policies=%d actions=%d\n", report.Workspace, len(report.Projects), len(report.Sources), len(report.Policies), len(report.Actions))
		for _, project := range report.Projects {
			fmt.Printf("project=%s manifests=%s\n", project.Path, strings.Join(project.Manifests, ","))
		}
		for _, policy := range report.Policies {
			fmt.Printf("policy=%s mode=%s requirement=%s\n", policy.Name, policy.Mode, policy.Requirement)
		}
		for _, action := range report.Actions {
			fmt.Printf("%s policy=%s %s\n", action.Priority, action.Policy, action.Text)
		}
		return nil
	default:
		return fmt.Errorf("unknown heartbeat subcommand %q", args[0])
	}
}

func defaultInboxSeenFile() string {
	home, err := os.UserHomeDir()
	if err != nil || home == "" {
		return ".orq-inbox-seen"
	}
	return filepath.Join(home, ".local", "share", "orq", "inbox-seen.txt")
}

func cmdInbox(args []string) error {
	if len(args) < 1 {
		return fmt.Errorf("inbox subcommand is required")
	}
	format, remaining, err := extractStringFlag(args[1:], "--format", "text")
	if err != nil {
		return err
	}
	path, remaining, err := extractStringFlag(remaining, "--path", "/home/freddy/Workspace/.agents/handoffs/hermes-orq")
	if err != nil {
		return err
	}
	seenFile, remaining, err := extractStringFlag(remaining, "--seen-file", defaultInboxSeenFile())
	if err != nil {
		return err
	}
	fileArg, remaining, err := extractStringFlag(remaining, "--file", "")
	if err != nil {
		return err
	}
	if len(remaining) > 0 {
		return fmt.Errorf("unexpected arguments: %s", strings.Join(remaining, " "))
	}
	switch args[0] {
	case "ack":
		if fileArg == "" {
			return fmt.Errorf("--file is required")
		}
		if err := inbox.MarkSeen(seenFile, fileArg); err != nil {
			return err
		}
		fmt.Printf("ack file=%s seen_file=%s\n", fileArg, seenFile)
		return nil
	case "feedbacks", "next":
		items, err := inbox.ScanFeedbacks(path)
		if err != nil {
			return err
		}
		if args[0] == "next" {
			seen, err := inbox.LoadSeen(seenFile)
			if err != nil {
				return err
			}
			item, ok := inbox.NextUnseenFeedback(items, seen)
			if format == "json" {
				return json.NewEncoder(os.Stdout).Encode(struct {
					Found bool                 `json:"found"`
					Item  inbox.FeedbackResume `json:"item,omitempty"`
				}{Found: ok, Item: item})
			}
			if !ok {
				fmt.Printf("next=none path=%s\n", path)
				return nil
			}
			fmt.Printf("next task=%s result=%s file=%s human=%t pi=%t\n", item.TaskID, item.Result, item.Path, item.NeedsHuman, item.NextForPi)
			return nil
		}
		if format == "json" {
			return json.NewEncoder(os.Stdout).Encode(items)
		}
		fmt.Printf("feedbacks=%d path=%s\n", len(items), path)
		for _, item := range items {
			flags := []string{}
			if item.NeedsHuman {
				flags = append(flags, "human")
			}
			if item.NextForPi {
				flags = append(flags, "pi")
			}
			if len(flags) == 0 {
				flags = append(flags, "info")
			}
			fmt.Printf("%s task=%s result=%s file=%s\n", strings.Join(flags, ","), item.TaskID, item.Result, item.Path)
		}
		return nil
	default:
		return fmt.Errorf("unknown inbox subcommand %q", args[0])
	}
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
	case "validate-template":
		file, remaining, err := extractStringFlag(remaining, "--file", "")
		if err != nil {
			return err
		}
		if file == "" {
			return fmt.Errorf("--file is required")
		}
		if len(remaining) > 0 {
			return fmt.Errorf("unexpected arguments: %s", strings.Join(remaining, " "))
		}
		content, err := os.ReadFile(file)
		if err != nil {
			return err
		}
		warnings := handoff.ValidateCacheableTemplate(string(content))
		if format == "json" {
			return json.NewEncoder(os.Stdout).Encode(struct {
				Valid    bool     `json:"valid"`
				Warnings []string `json:"warnings"`
			}{Valid: len(warnings) == 0, Warnings: warnings})
		}
		if len(warnings) == 0 {
			fmt.Println("OK warnings=0")
			return nil
		}
		fmt.Printf("WARN warnings=%d\n", len(warnings))
		for _, warning := range warnings {
			fmt.Printf("warning=%q\n", warning)
		}
		return nil
	case "chain":
		from, remaining, err := extractStringFlag(remaining, "--from", "")
		if err != nil {
			return err
		}
		to, remaining, err := extractStringFlag(remaining, "--to", output)
		if err != nil {
			return err
		}
		taskText, remaining, err := extractStringFlag(remaining, "--task", "")
		if err != nil {
			return err
		}
		nextAgent, remaining, err := extractStringFlag(remaining, "--next-agent", "")
		if err != nil {
			return err
		}
		if len(remaining) > 0 {
			return fmt.Errorf("unexpected arguments: %s", strings.Join(remaining, " "))
		}
		result, err := handoff.Chain(handoff.ChainRequest{From: from, To: to, Task: taskText, NextAgent: nextAgent})
		if err != nil {
			return err
		}
		if format == "json" {
			return json.NewEncoder(os.Stdout).Encode(result)
		}
		fmt.Printf("handoff_chain from=%s to=%s next_agent=%s bytes=%d\n", result.From, result.To, result.NextAgent, result.Bytes)
		return nil
	case "draft":
		id, remaining, err := extractStringFlag(remaining, "--task-id", "")
		if err != nil {
			return err
		}
		templateName, remaining, err := extractStringFlag(remaining, "--template", "default")
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
		draft, err := handoff.DraftWithTemplate(item, templateName)
		if err != nil {
			return err
		}
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
				TaskID   string `json:"task_id"`
				Template string `json:"template"`
				Output   string `json:"output,omitempty"`
				Draft    string `json:"draft"`
			}{TaskID: id, Template: templateName, Output: output, Draft: draft})
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
	case "next":
		if len(remaining) > 0 {
			return fmt.Errorf("unexpected arguments: %s", strings.Join(remaining, " "))
		}
		item, ok, err := task.Next(path)
		if err != nil {
			return err
		}
		if format == "json" {
			return json.NewEncoder(os.Stdout).Encode(struct {
				Found bool      `json:"found"`
				Item  task.Item `json:"item,omitempty"`
			}{Found: ok, Item: item})
		}
		if !ok {
			fmt.Println("no_next_task")
			return nil
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
		if !profile.Verified {
			return fmt.Errorf("agent/model pair %s/%s is registered but not verified; run real validation before assigning", agent, model)
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
	agentName, remaining, err := extractStringFlag(remaining, "--agent", "pi")
	if err != nil {
		return err
	}
	model, remaining, err := extractStringFlag(remaining, "--model", "")
	if err != nil {
		return err
	}
	handoffPath, remaining, err := extractStringFlag(remaining, "--handoff", "")
	if err != nil {
		return err
	}
	writeHandoff, remaining, err := extractStringFlag(remaining, "--write-handoff", "")
	if err != nil {
		return err
	}
	writeReceipt, remaining, err := extractStringFlag(remaining, "--write-receipt", "")
	if err != nil {
		return err
	}
	force, remaining := extractBoolFlag(remaining, "--force", false)
	repoPath, remaining, err := extractStringFlag(remaining, "--repo", "")
	if err != nil {
		return err
	}
	agentsDir, remaining, err := extractStringFlag(remaining, "--agents-dir", "")
	if err != nil {
		return err
	}
	workspace, remaining, err := extractStringFlag(remaining, "--workspace", "")
	if err != nil {
		return err
	}
	executed, remaining := extractBoolFlag(remaining, "--executed", false)
	task := strings.TrimSpace(strings.Join(remaining, " "))
	if task == "" && handoffPath == "" && writeHandoff == "" {
		return fmt.Errorf("task or --handoff or --write-handoff is required")
	}
	if task == "" && handoffPath != "" {
		task = fmt.Sprintf("ejecutar handoff %s", filepath.Base(handoffPath))
	}

	effectiveHandoff := handoffPath
	if effectiveHandoff == "" && writeHandoff != "" {
		effectiveHandoff = writeHandoff
	}

	opts := delegate.PlanOptions{
		Task:         task,
		Agent:        agentName,
		Executed:     executed,
		HandoffPath:  effectiveHandoff,
		RepoPath:     repoPath,
		AgentsDir:    agentsDir,
		Workspace:    workspace,
		Model:        model,
		WriteHandoff: writeHandoff,
		WriteReceipt: writeReceipt,
		Force:        force,
	}
	res, err := delegate.PlanWithOptions(opts)
	if err != nil {
		return err
	}
	if err := delegate.WriteDelegationFiles(opts, &res); err != nil {
		return err
	}

	if format == "json" {
		return json.NewEncoder(os.Stdout).Encode(res)
	}

	if res.WrittenHandoff != "" {
		fmt.Printf("Handoff escrito en: %s\n", res.WrittenHandoff)
	}
	if res.WrittenReceipt != "" {
		fmt.Printf("Receipt inicial escrito en: %s\n", res.WrittenReceipt)
	}
	if res.WrittenHandoff != "" || res.WrittenReceipt != "" {
		fmt.Println()
	}

	fmt.Printf("status=%s must_stop_for_delegation=%t supervisor_only=%t execution_agent_allowed=%t\nnext_step=%s\n\n", res.Status, res.MustStopForDelegation, res.SupervisorOnly, res.ExecutionAgentAllowed, res.NextStep)
	if res.AutonomousCommand != "" {
		fmt.Printf("Comando sugerido:\n%s\n\n", res.AutonomousCommand)
	}
	fmt.Println(res.Prompt)
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

func parseInt64Flag(name, value string) (int64, error) {
	parsed, err := strconv.ParseInt(value, 10, 64)
	if err != nil {
		return 0, fmt.Errorf("%s must be an integer", name)
	}
	if parsed < 0 {
		return 0, fmt.Errorf("%s must be >= 0", name)
	}
	return parsed, nil
}

func parseFloatFlag(name, value string) (float64, error) {
	parsed, err := strconv.ParseFloat(value, 64)
	if err != nil {
		return 0, fmt.Errorf("%s must be a number", name)
	}
	return parsed, nil
}

func buildReceiptCommands(commands []string, commandResults []string) ([]receipt.Command, error) {
	if len(commands) == 0 {
		return nil, nil
	}
	if len(commandResults) != 1 && len(commandResults) != len(commands) {
		return nil, fmt.Errorf("--command-result count must be 1 or match --command count")
	}
	built := make([]receipt.Command, 0, len(commands))
	for i, command := range commands {
		result := commandResults[0]
		if len(commandResults) == len(commands) {
			result = commandResults[i]
		}
		built = append(built, receipt.Command{Cmd: command, Result: result})
	}
	return built, nil
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

func extractStringFlags(args []string, name string) ([]string, []string, error) {
	values := []string{}
	remaining := make([]string, 0, len(args))
	for i := 0; i < len(args); i++ {
		arg := args[i]
		if arg == name {
			if i+1 >= len(args) {
				return nil, nil, fmt.Errorf("%s requires a value", name)
			}
			values = append(values, args[i+1])
			i++
			continue
		}
		prefix := name + "="
		if strings.HasPrefix(arg, prefix) {
			values = append(values, strings.TrimPrefix(arg, prefix))
			continue
		}
		remaining = append(remaining, arg)
	}
	return values, remaining, nil
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
