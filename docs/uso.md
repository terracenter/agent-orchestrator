# Uso de `orq`

`orq` es el CLI Rust de `agent-orchestrator`. La implementacion Go queda como
referencia archivada de paridad y no se instala ni se soporta como `orq`.

## Validar instalacion

```bash
orq --help
orq route --help
```

Si el shell no encuentra el binario, usa la ruta de instalacion local:

```bash
$HOME/.local/bin/orq --help
```

El instalador instala los binarios Rust `orq` y `orq-agent`:

```bash
rtk bash scripts/install.sh --dry-run
rtk bash scripts/install.sh
```

## Comandos basicos

```bash
# Descubrir runners locales sin leer secretos.
orq detect --format json
orq discover --format json
orq agents discover

# Consultar modelos y enrutar por un task kind explicito.
orq models --agent qwen-code --format json
orq route --task-kind mechanical --format json

# Consultar cuota, evidencia y opciones de los comandos.
orq quota --help
orq score --help
orq compliance --rtk-usage --format json
```

Ejecuta `orq <comando> --help` antes de usar comandos con efectos externos,
como `exec`, `smoke`, `certify`, `delegate` u `observer`.

## Disponibilidad diaria de agentes

`orq route` soporta filtrado por disponibilidad diaria de agentes para evitar recomendar runners no disponibles o fuera de servicio durante una sesión u operación.

### Formato de configuración JSON

```json
{
  "date": "2026-09-09",
  "available_agents": ["claude-code", "codex", "agy"],
  "unavailable_agents": ["hermes", "qwen-code", "openclaw", "pi", "nvidia-api"],
  "notes": "Disponibilidad declarada para la sesión"
}
```

### Reglas operativas
- **Precedencia de allowlist:** Si `available_agents` está presente y contiene agentes, actúa como allowlist prioritaria. Solo los agentes listados serán candidatos elegibles.
- **Denylist de respaldo:** Si `available_agents` está vacío, `unavailable_agents` actúa como denylist para descartar agentes específicos.
- **Fallback adaptativo:** Si el agente por defecto para un `task_kind` no está disponible hoy, `orq route` selecciona automáticamente un fallback disponible permitido por la matriz de routing.
- **Confirmación explícita:** Si ningún candidato para el `task_kind` está disponible, `orq route` marca `requires_confirmation: true` y la razón `daily_availability_unavailable` para evitar asignaciones silenciosas no operativas.

### Variable de entorno y CLI

```bash
# Vía variable de entorno
ORQ_DAILY_AVAILABILITY_PATH=/ruta/al/archivo.json orq route --task-kind mechanical --format json

# Vía flag CLI explícito
orq route --daily-availability-config /ruta/al/archivo.json --task-kind mechanical --format json
```

### Campos de salida JSON (`orq route --format json`)

- `availability_filter_used` (`bool`): `true` si se evaluó y aplicó un archivo de disponibilidad diario activo.
- `availability_filtered` (`usize`): número de candidatos descartados por el filtro diario.
- `availability_source` (`string`): ruta al archivo de disponibilidad usado, o `"none"` si no hay filtro activo.

## Event log de delegaciones y watchdog anti-loop (issue #172)

`orq delegate` registra cada transicion de estado (planificado, bloqueado,
ejecutado, validado, fallido, timeout) en un event log local JSONL
append-only. Es la base auditable para reroute adaptivo (#183/#217/#187); no
implementa el reroute en si.

### Ruta y variable de entorno

```bash
# Ruta por defecto: $HOME/.local/state/orq-agent/events.jsonl
ORQ_EVENT_LOG_PATH=/ruta/al/events.jsonl orq delegate --agent qwen-code --model qwen3.6-flash --task "..." --task-id mi-tarea
```

El archivo se crea con permisos `0600`. Cada linea es un objeto JSON con
`schema_version`, `event_kind`, `timestamp_unix`, `correlation_id`,
`task_id` (si se paso `--task-id`), `agent`, `model`, `status`, `verdict`,
y opcionalmente `failure_class`, `reason`, `exit_code`. Nunca incluye
`stdout_tail`/`stderr_tail` ni credenciales.

`event_kind` toma uno de: `delegation_planned`, `delegation_command_generated`,
`delegation_blocked`, `delegation_executed`, `delegation_validated`,
`delegation_failed`, `delegation_timeout`, `watchdog_loop_detected`.

### Watchdog anti-loop

Antes de ejecutar el proceso real (`--execute`), `orq delegate` revisa el
event log: si el mismo `--task-id` acumulo 3 o mas eventos
fallidos/bloqueados/timeout en los ultimos 600 segundos, la delegacion se
bloquea con `reason` `loop_detected: ...` y **no** se ejecuta el runner. Sin
`--task-id`, el watchdog no aplica (no hay clave estable para agrupar
reintentos sin generar falsos positivos entre tareas distintas del mismo
agente/modelo). Ventana y umbral son fijos por ahora
(`orq-agent/src/event_log.rs`: `DEFAULT_LOOP_WINDOW_SECONDS`,
`DEFAULT_LOOP_THRESHOLD`); hacerlos configurables queda para un follow-up.

## Desarrollo

```bash
rtk cargo test --manifest-path orq-agent/Cargo.toml
rtk cargo clippy --manifest-path orq-agent/Cargo.toml --all-targets -- -D warnings
make build
make install
```

`make install` instala `orq` y `orq-agent` bajo `~/.local/bin` por defecto.

## Codigo Go archivado

Consulta [legacy-go.md](legacy-go.md) para el estado del codigo Go y los
criterios de su eliminacion.
