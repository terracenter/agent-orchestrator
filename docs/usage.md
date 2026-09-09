# `orq` usage

`orq` is the Rust CLI for `agent-orchestrator`. The Go implementation is an
archived parity reference and is not installed or supported as `orq`.

## Validate installation

```bash
orq --help
orq route --help
```

If the shell cannot find it, use the local installation path:

```bash
$HOME/.local/bin/orq --help
```

Install both Rust binaries with the project installer:

```bash
rtk bash scripts/install.sh --dry-run
rtk bash scripts/install.sh
```

## Basic commands

```bash
# Discover local runners without reading secrets.
orq detect --format json
orq discover --format json
orq agents discover

# Inspect known models and route an explicit task kind.
orq models --agent qwen-code --format json
orq route --task-kind mechanical --format json

# Inspect quota, evidence and available command options.
orq quota --help
orq score --help
orq compliance --rtk-usage --format json
```

Run `orq <command> --help` before using a command with external effects such as
`exec`, `smoke`, `certify`, `delegate`, or `observer`.

## Development

```bash
rtk cargo test --manifest-path orq-agent/Cargo.toml
rtk cargo clippy --manifest-path orq-agent/Cargo.toml --all-targets -- -D warnings
make build
make install
```

`make install` installs `orq` and `orq-agent` under `~/.local/bin` by default.

## Legacy source

See [legacy-go.md](legacy-go.md) for the archived Go reference and the planned
removal criteria.
