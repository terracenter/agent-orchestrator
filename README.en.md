**English** · [Español](README.md)

# agent-orchestrator

![License](https://img.shields.io/badge/license-AGPL--3.0--or--later-blue)
![Status](https://img.shields.io/badge/status-operational%20MVP-orange)
![Stack](https://img.shields.io/badge/stack-Go%20%7C%20Docker%20%7C%20Observer-informational)
![PRs](https://img.shields.io/badge/PRs-docs%20%2B%20tests%20required-brightgreen)

> Local-first orchestrator for agents and models: classifies tasks, recommends the cheapest sufficient safe agent/model, records verifiable evidence, and keeps documentation as an operational changelog.

`orq` coordinates runners such as Pi, Claude Code, AGY, OpenClaw, NVIDIA/local, and workspace adapters without turning automation into a black box. Its core rule is simple: **verified facts, minimum sufficient cost, and dry-run before mutation**.

---

## Current status

| Area | Status |
|---|---|
| Classification and routing | Operational MVP with `orq classify` and `orq route` |
| Evidence | JSONL ledger + verifiable receipts |
| Observer LLM | Best-effort sync and capacity snapshots |
| Budget control | Compaction guardrails and low-cost routing |
| Automatic execution | Limited; dry-run and confirmation before sensitive actions |
| Documentation | ROADMAP/RELEASES/README/docs act as the operational changelog |

> ⚠️ **This is not an autonomous production executor.** Destructive actions, credentials, deploys, or remote changes require explicit confirmation.

---

## What it solves

- Avoids using expensive models for mechanical tasks.
- Detects when a task requires a stronger validator because of risk or security.
- Records which agent/model acted, with verifiable receipts.
- Sends telemetry to Observer LLM.
- Uses capacity/quota snapshots to avoid routing non-critical work to exhausted agents.
- Keeps public documentation auditable on every PR.

---

## Development quickstart

This repository uses Go and Docker for reproducible validation.

```bash
rtk docker compose run --rm dev go test ./...
rtk docker compose run --rm dev go run ./cmd/orq --help
rtk docker compose run --rm dev go run ./cmd/orq config --config examples/config.example.toml --check-adapters
```

Install the local binary:

```bash
rtk docker compose run --rm dev make build
mkdir -p ~/.local/bin
install -m 0755 bin/orq ~/.local/bin/orq
orq --help
```

---

## Essential usage

```bash
orq classify "fix a broken reference"
orq route "rotate a production token"
orq route --capacity-file /path/capacity.json "simple mechanical task"
orq record --task test --agent pi --model gpt-5.5 --status ok
orq status
orq delegate "organize vault information related to GLPI"
orq observer sync --format json
orq observer send-capacity --agent claude-code --provider-group anthropic --model-group haiku --remaining-percent 80 --window daily
```

---

## Architecture summary

| Component | Role |
|---|---|
| `cmd/orq` | Main CLI |
| `internal/route` | Classification, routing, and capacity-aware adjustment |
| `internal/ledger` / `internal/receipt` | Local evidence and receipts |
| `internal/observer` | Observer LLM client |
| `internal/adapters` | Workspace tooling integration (`rtk`, `vg`, runners) |
| `examples/config.example.toml` | Reference configuration |

---

## Key documentation

- [ROADMAP.md](ROADMAP.md) — live status, phases, and update policy.
- [RELEASES.md](RELEASES.md) — operational changelog by deliverable.
- [docs/uso.md](docs/uso.md) — Spanish usage guide.
- [docs/usage.md](docs/usage.md) — English usage guide.
- [docs/prueba-integral-orq.md](docs/prueba-integral-orq.md) — full harness test.
- [docs/orca-inspiracion.md](docs/orca-inspiracion.md) — Orca technical inspiration.

---

## Documentation policy

Every closed issue, PR, or deliverable must update, when applicable:

- `ROADMAP.md`
- `RELEASES.md`
- `README.md` / `README.en.md`
- `docs/uso.md` / `docs/usage.md`
- related operational documentation

If documentation does not apply, the PR must explicitly say: `Docs: no aplica` with justification.

---

## Security

`main` is protected by a GitHub ruleset: pull request required, force-push/deletion blocked, and the `go-test` check required.

See [SECURITY.md](SECURITY.md).

---

## Philosophy and inspiration

This project reuses useful patterns from the AI coding ecosystem — Engram, Gentle-AI, Gentleman Guardian Angel, skills, and receipt systems — only when they fit the local-first goal.

Principles:

- Obsidian can be the human SSoT, but the project must work without Obsidian.
- Kuzu/vg can be the documentation graph layer, but it is optional.
- rtk reduces command noise, but it does not replace evidence.
- Verified facts > model opinions.
- Dry-run first.

---

## License

GNU AGPL-3.0-or-later. If you run a modified version as a network service, you must provide the corresponding source code under the license.
