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
