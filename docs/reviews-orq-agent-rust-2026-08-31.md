# Reviews multiagente: `orq-agent` Rust — 2026-08-31

## Contexto

Freddy pidió aprovechar otros agentes/modelos, en particular Claude y Qwen, para continuar el MVP de `orq-agent`.

## Estado de Orq

- Se asignó tarea a `claude-code/claude-3-5-haiku-20241022`, pero el runtime Claude reportó deprecación/EOL de ese modelo.
- Se reintentó con `claude-haiku-4-5`, sin usar Sonnet/Opus.
- Se intentó asignar `qwen-code/qwen3.8-max` en el registry de tareas, pero Orq rechazó el par con `unknown agent/model pair` aunque el par está documentado en `docs/agent-model-capabilities.md` e `internal/agent/registry.go`.
- Se ejecutó Qwen directo con `qwen --safe-mode -m qwen3.8-max` para revisión de solo lectura.

## Review Qwen — síntesis

Qwen confirmó que el MVP actual cubre solo `detect` y que el bug principal sigue vivo hasta implementar `exec` real.

Hallazgos principales:

1. Falta `exec.rs`, `receipt.rs`, `policy.rs` y `error.rs`.
2. El trait `AgentAdapter` debe evolucionar a `Send + Sync` y exponer `build_argv` para evitar `if/else` por agente.
3. `detect` mezcla estado del adapter con presencia del binario: un adapter puede aparecer `available` aunque `detected=false`.
4. El flujo de adapters faltantes aún no existe: solo se recorren adapters conocidos.
5. Falta CI Rust y `orq-agent/.gitignore` para evitar commitear `target/`.
6. Hay divergencia de schema entre `orq agents detect` Go y `orq-agent detect` Rust.
7. `exec` debe usar argv vectorial, nunca `sh -c`.
8. `stdout/stderr` deben capturarse con buffers acotados y tails sanitizados.

Recomendación Qwen P0:

- Implementar `policy.rs`, `exec.rs`, `receipt.rs`, `error.rs`.
- Agregar `build_argv` al trait.
- Agregar `.gitignore` para `/target`.
- Agregar tests de integración con runners fake.

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
- Se agregó adapter Rust para `qwen-code` detectando binario `qwen`.

## Próxima implementación recomendada

1. `policy.rs` con bloqueo de `gated`, `deprecated_or_quarantine`, Sonnet/Opus y timeouts inválidos.
2. `receipt.rs` con schema estable.
3. `exec.rs` con `tokio::process`, timeout y salida acotada.
4. Tests con runners fake.
5. CI/Makefile para Rust.
