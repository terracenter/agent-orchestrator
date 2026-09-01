**Español** · [English](README.en.md)

# agent-orchestrator

![Licencia](https://img.shields.io/badge/licencia-AGPL--3.0--or--later-blue)
![Estado](https://img.shields.io/badge/estado-MVP%20operativo-orange)
![Stack](https://img.shields.io/badge/stack-Rust--first%20%7C%20Go%20legacy%20%7C%20Observer-informational)
![PRs](https://img.shields.io/badge/PRs-docs%20%2B%20tests%20obligatorios-brightgreen)

> Orquestador local-first de agentes y modelos: clasifica tareas, recomienda el agente/modelo más barato suficiente, ejecuta agentes reales con receipts verificables y mantiene documentación como changelog operativo.

`orq` coordina runners como Pi, Claude Code, AGY, OpenClaw, Qwen y adaptadores del workspace sin convertir la automatización en una caja negra. Su principio central es simple: **hechos verificados, costo mínimo suficiente y dry-run antes de mutar**.

> Decisión de arquitectura: el proyecto pasa a **Rust-first**. Go queda como implementación legacy temporal y referencia de paridad mientras los comandos se migran por slices al binario Rust objetivo.

---

## Estado actual

| Área | Estado |
|---|---|
| Clasificación y routing | Go legacy mantiene comandos amplios; Rust `orq-agent route` ya usa config/certificados |
| Evidencia | Ledger JSONL + receipts verificables |
| Observer LLM | Sincronización best-effort y snapshots de capacidad |
| Control de presupuesto | Guardrails de compactación y rutas de bajo costo |
| Ejecución automática | Rust `orq-agent` operativo con receipts JSON; Go solo lo consume como puente temporal |
| Documentación | ROADMAP/RELEASES/README/docs funcionan como changelog operativo |

> ⚠️ **No es un ejecutor autónomo de producción.** Acciones destructivas, credenciales, deploys o cambios remotos requieren confirmación explícita.

---

## Qué resuelve

- Evita usar modelos caros para tareas mecánicas.
- Decide cuándo una tarea requiere un validador fuerte por riesgo o seguridad.
- Registra qué agente/modelo actuó, con recibos verificables.
- Integra telemetría hacia Observer LLM.
- Usa snapshots de capacidad/cuota para no enrutar tareas no críticas hacia agentes agotados.
- Mantiene una política documental pública y auditable por PR.

---

## Quickstart de desarrollo

Este repo está migrando a Rust-first. Go se conserva temporalmente para compatibilidad y como referencia de paridad.

```bash
# Tests Rust
cd orq-agent && rtk cargo test

# Tests Go legacy mientras dure la migración
rtk go test ./...

# Ejecución real progresiva vía Rust
rtk go run ./cmd/orq run --execute --agent qwen-code --model qwen3.8-max --orq-agent-bin ./orq-agent/target/debug/orq-agent "Responde exactamente: OK"

# Alias Rust en transición: el crate ya compila `orq-agent` y `orq`.
# Mientras el binario Go legacy siga instalado como `orq`, no instalar el alias Rust
# sobre PATH compartido sin decidir explícitamente el handoff.
cd orq-agent && rtk cargo build --bins
./target/debug/orq models --agent qwen-code --format json
```

Instalación local del binario legacy Go:

```bash
rtk docker compose run --rm dev make build
mkdir -p ~/.local/bin
install -m 0755 bin/orq ~/.local/bin/orq
orq --help
```

> ⚠️ Transición CLI: `scripts/install.sh` todavía instala el `orq` Go legacy. El crate Rust ya compila `orq-agent` y alias `orq`, pero el reemplazo de PATH será un slice separado para no romper comandos Go pendientes (`task`, `handoff`, `inbox`, `observer`, etc.).

Instalador simple con modo seguro:

```bash
# Ver acciones sin modificar archivos
rtk bash scripts/install.sh --dry-run

# Instalación interactiva
rtk bash scripts/install.sh
```

El instalador crea backup de `~/.local/bin/orq` antes de reemplazarlo y advierte si falta `rtk`.

---

## Uso esencial

```bash
# Clasificar una tarea
orq classify "corregir una referencia rota"

# Recomendar agente/modelo
orq route "rotar token de producción"

# Routing asistido por capacidad/cuota agregada
orq route --capacity-file /ruta/capacity.json "tarea mecánica simple"

# Registrar evidencia
orq record --task test --agent pi --model gpt-5.5 --status ok

# Estado del ledger
orq status

# Delegación controlada
orq delegate "ordenar información del vault relacionada con GLPI"

# Sincronizar Observer
orq observer sync --format json

# Enviar snapshot manual de capacidad
orq observer send-capacity --agent claude-code --provider-group anthropic --model-group haiku --remaining-percent 80 --window daily
```

---

## Arquitectura resumida

| Componente | Rol |
|---|---|
| `cmd/orq` | CLI principal |
| `internal/route` | Clasificación, routing y ajuste por capacidad |
| `internal/ledger` / `internal/receipt` | Evidencia local y receipts |
| `internal/observer` | Cliente hacia Observer LLM |
| `internal/adapters` | Integración con herramientas del workspace (`rtk`, `vg`, runners) |
| `examples/config.example.toml` | Configuración de referencia |

---

## Documentación clave

- [ROADMAP.md](ROADMAP.md) — estado vivo, fases y política de actualización.
- [RELEASES.md](RELEASES.md) — changelog operativo por entregable.
- [docs/uso.md](docs/uso.md) — guía de uso en español.
- [docs/usage.md](docs/usage.md) — usage guide in English.
- [docs/prueba-integral-orq.md](docs/prueba-integral-orq.md) — prueba integral del arnés.
- [docs/orca-inspiracion.md](docs/orca-inspiracion.md) — inspiración técnica Orca.

---

## Política de documentación

Todo issue, PR o entregable cerrado debe actualizar, según corresponda:

- `ROADMAP.md`
- `RELEASES.md`
- `README.md` / `README.en.md`
- `docs/uso.md` / `docs/usage.md`
- documentación operativa relacionada

Si no aplica, el PR debe decir explícitamente: `Docs: no aplica` con justificación.

---

## Seguridad

`main` está protegido con Pull Request obligatorio, bloqueo de force-push/borrado y check `go-test` requerido.

Ver [SECURITY.md](SECURITY.md).

---

## Filosofía e inspiración

Este proyecto reutiliza patrones buenos del ecosistema AI coding —Engram, Gentle-AI, Gentleman Guardian Angel, skills y sistemas de receipts— solo cuando encajan con el objetivo local-first.

Principios:

- Obsidian puede ser la SSoT humana, pero el proyecto debe funcionar sin Obsidian.
- Kuzu/vg puede ser graph layer documental, pero es opcional.
- rtk reduce ruido de comandos, pero no reemplaza la evidencia.
- Hechos verificados > opiniones de modelos.
- Dry-run primero.

---

## Licencia

GNU AGPL-3.0-or-later. Si ejecutas una versión modificada como servicio de red, debes ofrecer el código fuente correspondiente según la licencia.
