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
