# agent-orchestrator

> Local-first agent/model orchestrator: classifies tasks, recommends the cheapest safe agent/model, and records verifiable evidence before automating execution.

**Language:** Venezuelan Spanish is the primary project language. This file provides American English documentation.

## Philosophy

This project does not try to reinvent the wheel. It reuses good ideas and patterns from the AI coding ecosystem — especially Engram, Gentle-AI, Gentleman Guardian Angel, and skill systems — when they fit the project's goals.

Principles:

- **Obsidian can be the human SSoT**, but the project also works without Obsidian.
- **Kuzu/vg can be the documentation graph layer**, but it is an optional adapter.
- **Engram can be cross-session operational memory**, but it does not replace the source of truth.
- **rtk can wrap commands to reduce noise**, but third-party users can use standard execution.
- **Verified facts > model opinions.**
- **Dry-run first.** Automatic agent execution is not part of the initial MVP.

## Status

Initial MVP: **ledger + advisory mode**.

Usage guide: [docs/usage.md](docs/usage.md).

Current/planned commands:

```bash
orq classify "fix a broken reference"
orq route "rotate production token"
orq record --task test --agent pi --model gpt-5.5 --status ok
orq status
orq run "audit project" --dry-run
orq guard --vault /path/to/vault --format json
orq config --config examples/config.example.toml --check-adapters --format json
orq vault-order --vault /home/freddy/Workspace/Obsidian --query glpi --format json
orq delegate "organize vault information related to GLPI"
```

## Development setup

This repo uses Go and a Docker environment for reproducible validation.

```bash
docker compose run --rm dev go test ./...
docker compose run --rm dev go run ./cmd/orq --help
docker compose run --rm dev go run ./cmd/orq config --config examples/config.example.toml --check-adapters
```

## Local CLI installation

To use `orq` in future Pi sessions or after `/reload`, install the binary into `~/.local/bin`:

```bash
docker compose run --rm dev make build
mkdir -p ~/.local/bin
install -m 0755 bin/orq ~/.local/bin/orq
orq --help
```

If you are developing without a container:

```bash
make install
```

## Project languages

- Primary documentation: Venezuelan Spanish.
- Secondary documentation: American English.
- Standard infrastructure and development terms stay in English when appropriate.

## Repository security

`main` is protected with a GitHub ruleset: Pull Requests are required, force pushes/deletion are blocked, and the `go-test` check is required.

See [SECURITY.md](SECURITY.md).

## License

AGPLv3. The goal is to ensure improvements and derivatives offered as a service are shared back with the community.

## Inspiration and attribution

This project is inspired by ideas from the [Gentleman-Programming](https://github.com/Gentleman-Programming) ecosystem, especially:

- Engram — persistent memory for agents.
- Gentle-AI — workflows, SDD, phase routing, and multi-agent ecosystem ideas.
- Gentleman Guardian Angel — review/verdict contracts and provider-agnostic validation.
- Gentleman-Skills — community skill format and governance.

Ideas are evaluated and adapted as needed. Any third-party code reused literally must preserve its license and attribution.
