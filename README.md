**Espanol** · [English](README.en.md)

# agent-orchestrator

![Licencia](https://img.shields.io/badge/licencia-AGPL--3.0--or--later-blue)
![Estado](https://img.shields.io/badge/estado-MVP%20operativo-orange)
![Stack](https://img.shields.io/badge/stack-Rust--only%20%7C%20Observer-informational)
![PRs](https://img.shields.io/badge/PRs-docs%20%2B%20tests%20obligatorios-brightgreen)

Orquestador local-first de agentes y modelos. `orq` descubre runners sin leer
secretos, enruta por evidencia, ejecuta tareas acotadas y produce receipts
verificables.

> Arquitectura operativa: `orq` y `orq-agent` son binarios Rust. El codigo Go
> permanece archivado solo como referencia de paridad y no se construye ni se
> instala como `orq`.

## Inicio rapido

```bash
# Validar el crate Rust
cd orq-agent && rtk cargo test

# Ver que instalaria sin modificar el sistema
rtk bash scripts/install.sh --dry-run

# Instalar orq y orq-agent en ~/.local/bin
rtk bash scripts/install.sh

# Ver comandos y enrutar una tarea explicita
orq --help
orq route --task-kind mechanical --format json
```

Tambien puedes usar los objetivos del proyecto:

```bash
make build
make install
```

## Uso esencial

```bash
orq detect --format json
orq discover --format json
orq agents discover
orq models --agent qwen-code --format json
orq route --task-kind mechanical --format json

# Enrutar con disponibilidad diaria de agentes (allowlist/denylist)
ORQ_DAILY_AVAILABILITY_PATH=config/daily-availability.json orq route --task-kind mechanical --format json
orq route --daily-availability-config config/daily-availability.json --task-kind mechanical --format json

orq quota --help
orq compliance --rtk-usage --format json
```

Usa `orq <comando> --help` antes de ejecutar `exec`, `smoke`, `certify`,
`delegate` u `observer`, que pueden interactuar con runners o estado local.

## Arquitectura

| Componente | Rol |
|---|---|
| `orq-agent/src/bin/orq.rs` | CLI Rust instalado como `orq` |
| `orq-agent/src/bin/orq_agent.rs` | Entrada Rust compatible `orq-agent` |
| `orq-agent/src/route.rs` | Routing basado en catalogo, certificados y cuota |
| `orq-agent/src/event_log.rs` | Event log JSONL de delegaciones y watchdog anti-loop (#172) |
| `orq-agent/config/` | Matriz, catalogo y politicas de routing |
| `cmd/`, `internal/` | Referencia Go archivada; no instalable |

## Documentacion

- [docs/uso.md](docs/uso.md) - guia de uso en espanol.
- [docs/usage.md](docs/usage.md) - English usage guide.
- [docs/legacy-go.md](docs/legacy-go.md) - estado y retiro del codigo Go.
- [docs/matriz-paridad-rust.md](docs/matriz-paridad-rust.md) - paridad pendiente.
- [ROADMAP.md](ROADMAP.md) - fases y estado operativo.
- [RELEASES.md](RELEASES.md) - historial de cambios.

## Desarrollo y CI

```bash
rtk cargo test --manifest-path orq-agent/Cargo.toml
rtk cargo clippy --manifest-path orq-agent/Cargo.toml --all-targets -- -D warnings
```

El CI ejecuta tests y clippy de Rust. Las acciones destructivas, credenciales,
deploys y cambios remotos requieren confirmacion explicita.

## Licencia

GNU AGPL-3.0-or-later.
