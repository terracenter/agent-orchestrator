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

```bash
orq classify "fix a broken reference"
orq route "rotate production token"
orq run "audit project" --dry-run
orq record --task test --agent pi --model gpt-5.5 --status ok
orq status
```

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
- validates basic guards;
- loads config and adapters;
- generates documentation-order plans with `vault-order`;
- **does not automatically execute agents yet**.
