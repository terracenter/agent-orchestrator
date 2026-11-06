# Reviews multiagente: `orq-agent` Rust — 2026-08-31

## Contexto

Freddy pidió aprovechar otros agentes/modelos, en particular Claude, para continuar el MVP de `orq-agent`.

## Estado de Orq

- Se asignó tarea a `claude-code/claude-3-5-haiku-20241022`, pero el runtime Claude reportó deprecación/EOL de ese modelo.
- Se reintentó con `claude-haiku-4-5`, sin usar Sonnet/Opus.

## Review Claude Haiku — síntesis

Claude enfocó seguridad/policy.

Hallazgos principales:

1. Antes de ejecutar, bloquear adapters `Gated` sin aprobación explícita.
2. Bloquear `DeprecatedOrQuarantine` por defecto.
3. Sanitizar `stdout_tail` y `stderr_tail` antes de emitir receipts.
4. No heredar entorno completo del proceso padre sin allowlist.
5. Usar whitelist de comandos/argv por adapter.
6. `executed=true` solo debe significar que el proceso real fue invocado.
7. `exit_code` debe existir si `executed=true`, salvo errores previos al spawn.
8. Sonnet/Opus deben quedar bloqueados sin aprobación humana.

## Cambios aplicados tras las reviews

- Se agregó `orq-agent/.gitignore` con `/target`.

## Próxima implementación recomendada

1. `policy.rs` con bloqueo de `gated`, `deprecated_or_quarantine`, Sonnet/Opus y timeouts inválidos.
2. `receipt.rs` con schema estable.
3. `exec.rs` con `tokio::process`, timeout y salida acotada.
4. Tests con runners fake.
5. CI/Makefile para Rust.
