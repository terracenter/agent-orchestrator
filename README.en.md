**English** · [Espanol](README.md)

# agent-orchestrator

![License](https://img.shields.io/badge/license-AGPL--3.0--or--later-blue)
![Status](https://img.shields.io/badge/status-operational%20MVP-orange)
![Stack](https://img.shields.io/badge/stack-Rust--only%20%7C%20Observer-informational)
![PRs](https://img.shields.io/badge/PRs-docs%20%2B%20tests%20required-brightgreen)

Local-first agent and model orchestrator. `orq` discovers runners without
reading secrets, routes from evidence, executes bounded tasks, and emits
verifiable receipts.

> Operational architecture: `orq` and `orq-agent` are Rust binaries. The Go
> code remains archived only as a parity reference and is neither built nor
> installed as `orq`.

## Quick start

```bash
# Validate the Rust crate
cd orq-agent && rtk cargo test

# Inspect the installation without changing the system
rtk bash scripts/install.sh --dry-run

# Install orq and orq-agent into ~/.local/bin
rtk bash scripts/install.sh

# Inspect commands and route an explicit task kind
orq --help
orq route --task-kind mechanical --format json
```

Project targets are also available:

```bash
make build
make install
```

## Essential usage

```bash
orq detect --format json
orq discover --format json
orq agents discover
orq models --agent qwen-code --format json
orq route --task-kind mechanical --format json
orq quota --help
orq compliance --rtk-usage --format json
```

Use `orq <command> --help` before running `exec`, `smoke`, `certify`,
`delegate`, or `observer`, which can interact with runners or local state.

## Architecture

| Component | Role |
|---|---|
| `orq-agent/src/bin/orq.rs` | Rust CLI installed as `orq` |
| `orq-agent/src/bin/orq_agent.rs` | Compatible Rust `orq-agent` entry point |
| `orq-agent/src/route.rs` | Catalog-, certificate-, and quota-aware routing |
| `orq-agent/config/` | Routing matrix, catalog, and policies |
| `cmd/`, `internal/` | Archived Go reference; not installable |

## Documentation

- [docs/uso.md](docs/uso.md) - Spanish usage guide.
- [docs/usage.md](docs/usage.md) - English usage guide.
- [docs/legacy-go.md](docs/legacy-go.md) - Go source status and removal plan.
- [docs/matriz-paridad-rust.md](docs/matriz-paridad-rust.md) - remaining parity work.
- [ROADMAP.md](ROADMAP.md) - phases and operational state.
- [RELEASES.md](RELEASES.md) - change history.

## Development and CI

```bash
rtk cargo test --manifest-path orq-agent/Cargo.toml
rtk cargo clippy --manifest-path orq-agent/Cargo.toml --all-targets -- -D warnings
```

CI runs Rust tests and clippy. Destructive actions, credentials, deployments,
and remote changes require explicit confirmation.

## License

GNU AGPL-3.0-or-later.
