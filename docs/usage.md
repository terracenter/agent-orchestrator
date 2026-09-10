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

## Daily agent availability

`orq route` supports daily agent availability filtering to avoid recommending runners that are unavailable or out of service during an operational session.

### JSON configuration format

```json
{
  "date": "2026-09-09",
  "available_agents": ["claude-code", "codex", "agy"],
  "unavailable_agents": ["hermes", "qwen-code", "openclaw", "pi", "nvidia-api"],
  "notes": "Availability declared for the session"
}
```

### Operational rules
- **Allowlist precedence:** If `available_agents` is present and contains agents, it acts as a prioritized allowlist. Only listed agents are eligible candidates.
- **Fallback denylist:** If `available_agents` is empty, `unavailable_agents` acts as a denylist to exclude specific agents.
- **Adaptive fallback:** If the default agent for a `task_kind` is unavailable today, `orq route` automatically selects an available fallback allowed by the routing matrix.
- **Explicit confirmation:** If no candidates are available for the `task_kind`, `orq route` flags `requires_confirmation: true` with reason `daily_availability_unavailable` to prevent silent unworkable assignments.

### Environment variable and CLI flag

```bash
# Via environment variable
ORQ_DAILY_AVAILABILITY_PATH=/path/to/file.json orq route --task-kind mechanical --format json

# Via explicit CLI flag
orq route --daily-availability-config /path/to/file.json --task-kind mechanical --format json
```

### JSON output fields (`orq route --format json`)

- `availability_filter_used` (`bool`): `true` if an active daily availability file was evaluated and applied.
- `availability_filtered` (`usize`): number of candidates filtered out by daily availability.
- `availability_source` (`string`): path to the daily availability file used, or `"none"` if no filter is active.

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
