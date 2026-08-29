# `orq` usage

`orq` is the local CLI for `agent-orchestrator`.

> Note: the command is `orq`, with **q**. It is not `org`.

## Validate installation

```bash
orq --help
```

If your shell cannot find it, use the full path:

```bash
/home/freddy/.local/bin/orq --help
```

On this machine the installed binary lives at:

```txt
/home/freddy/.local/bin/orq
```

## Basic commands

Versioned guides from the binary:

```bash
orq docs usage
orq docs orchestration
```

Common commands:

```bash
orq classify "fix a broken reference"
orq route "rotate production token"
orq delegate --handoff /home/freddy/Workspace/.agents/handoffs/task.md
orq delegate "implement feature" --agent agy
orq delegate "implement feature" --agent agy --write-handoff /home/freddy/Workspace/.agents/handoffs/task.md --write-receipt /tmp/receipt.json
orq run "audit project" --dry-run
orq record --task test --agent pi --model gpt-5.5 --status ok
orq status
orq agents
orq agents detect
orq doctor  # checks key tools (rtk, git, gh, orq, vg via ORQ_VG_PATH/PATH/known workspace paths)
```

## Task tracking for future mobile dashboard

```bash
orq task create "organize GLPI vault"
orq task list
orq task assign <id> --agent pi --model cheap-or-fast --host minipc
orq handoff draft --task-id <id>
orq handoff draft --task-id <id> --template reviewer-4r
orq handoff draft --task-id <id> --template security-reviewer
orq handoff draft --task-id <id> --template implementer
orq handoff draft --task-id <id> --template documenter
orq handoff draft --task-id <id> --template architect
orq handoff validate-template --file handoff.md
orq task update <id> --state running
orq task update <id> --state done --evidence "validated commit or PR"
```

By default tasks are stored at:

```txt
~/.local/state/orq/tasks.jsonl
```

These states will feed the future WireGuard-only dashboard/PWA.

## Vault order planning

This command **does not move files**. It only proposes actions to create indexes and detect documents without numeric prefixes.

```bash
orq vault-order --vault /home/freddy/Workspace/Obsidian --query glpi
```

For machine-readable output:

```bash
orq vault-order --vault /home/freddy/Workspace/Obsidian --query glpi --format json
```

## Config validation

From inside the repo:

```bash
cd /home/freddy/Workspace/Desarrollo/agent-orchestrator
orq config --config examples/config.example.toml --check-adapters --format json
```

From any directory, use the absolute path:

```bash
orq config --config /home/freddy/Workspace/Desarrollo/agent-orchestrator/examples/config.example.toml --check-adapters --format json
```

## Current MVP state

`orq` is still advisory-only:

- classifies tasks;
- recommends agent/model routing;
- records events;
- records tasks with verifiable state;
- validates basic guards;
- loads config and adapters;
- generates documentation-order plans with `vault-order`;
- **does not automatically execute agents yet**.
