# RFC: `orq-agent` en Rust como dispatcher real

## Estado

Borrador inicial.

## Problema

El `orq` actual decide rutas, genera handoffs y registra eventos, pero no garantiza ejecución real de agentes externos. Esto crea un cuello de botella: Pi termina actuando como supervisor y ejecutor, y no hay forma fiable de confirmar que AGY, OpenClaw, Hermes, Claude o modelos NVIDIA hayan trabajado salvo ejecución manual fuera de Orq.

Hallazgos actuales:

- `orq run` devuelve `executed=false`.
- `orq delegate` genera handoff/comando, pero no invoca el runner.
- `orq delegate --executed` solo marca `executed_unverified`.
- La documentación menciona comandos como `orq agents configure all --dry-run`, pero el binario actual no los acepta.
- `nvidia-api` mezcla conceptos: provider remoto, modelo y supuesto agente.

Esto se considera BUG funcional del orquestador para el requisito: "usar solo Orq y asegurar que todos los agentes trabajen".

## Objetivo

Crear un `orq-agent` real en Rust que pueda detectar agentes, elegir adapters, ejecutar runners, capturar resultados, generar recibos verificables y registrar telemetría.

## No objetivos iniciales

- No borrar el `orq` actual en Go durante el MVP.
- No migrar toda la lógica histórica de una vez sin validación.
- No tocar servicios vivos.
- No leer ni exponer secretos.
- No depender de minipc como requisito obligatorio.

## Pregunta estratégica: ¿migración completa a Rust?

La migración completa a Rust debe evaluarse como objetivo posible, no asumirla sin una prueba de equivalencia funcional.

Beneficios esperados de migrar por completo:

- Un solo lenguaje para CLI, dispatcher, adapters, receipts y policy engine.
- Mejor control de procesos hijos, timeouts, señales, cancelación y concurrencia.
- Tipado fuerte para estados de tareas, transiciones, receipts y errores.
- Binario único distribuible, con menos dependencias runtime.
- Mejor base para auditoría determinista y seguridad local-first.
- Menos deuda por mezcla Go/Bash/handoffs/manual execution.

Costos/riesgos:

- Reescribir funcionalidad ya existente puede introducir regresiones.
- Hay que preservar compatibilidad de comandos usados por el workspace.
- Hay que migrar pruebas y fixtures.
- Rust exige más disciplina de diseño inicial.
- Builds release pueden ser pesados en Pi; minipc ayuda, pero no puede ser dependencia obligatoria por la inestabilidad eléctrica.

Decisión provisional:

1. Crear `orq-agent` Rust como PoC/MVP real.
2. Mantener `orq` Go legacy mientras se alcanza paridad mínima.
3. Definir una matriz de paridad comando por comando.
4. Migrar completamente solo si el MVP demuestra mejor ejecución real, menor ambigüedad y receipts verificables sin romper flujos actuales.

Criterio para aprobar migración completa:

- `orq-agent` ejecuta agentes reales con recibos.
- Cubre rutas críticas actuales: `route`, `task`, `agents detect`, `delegate/exec`, `receipt`, `observer sync` o sus equivalentes.
- Tiene tests para transiciones de estado, detección, ejecución y errores.
- Mantiene política RTK y seguridad de secretos.
- Puede compilar y probarse en Pi en modo liviano; minipc solo acelera builds pesados.

## Decisiones actuales

- Pi queda como interfaz/supervisor principal.
- NVIDIA queda solo bajo Pi por ahora, para evitar ambigüedad.
- OpenClaw queda como candidato principal de runner Claude/Pro, pendiente validar costo real.
- Hermes queda en cuarentena/deprecated hasta comparar valor con OpenClaw.
- Claude Sonnet/Opus siguen gated por aprobación explícita.
- minipc puede usarse para compilaciones pesadas solo si está disponible; hay problemas de electricidad, por lo que no debe bloquear el flujo.

## Arquitectura propuesta

```text
Freddy / Pi
  -> orq-agent
      -> policy engine
      -> registry de agentes/modelos
      -> adapters detectados
      -> executor con timeout
      -> receipt store
      -> ledger/Observer
```

## Contrato de adapter

Cada adapter debe implementar como mínimo:

```rust
trait AgentAdapter {
    fn name(&self) -> &str;
    fn detect(&self) -> DetectionResult;
    fn list_models(&self) -> Result<Vec<ModelInfo>>;
    fn smoke_test(&self, model: &str) -> Result<SmokeResult>;
    fn execute(&self, request: ExecRequest) -> Result<ExecReceipt>;
}
```

## Detección y adapters faltantes

El sistema no debe ignorar agentes detectados sin adapter.

Flujo esperado:

1. `orq-agent detect` descubre binarios/configuraciones sin leer secretos.
2. Si existe adapter, marca `adapter=available`.
3. Si no existe adapter, marca `adapter=missing`.
4. Para adapters faltantes, genera propuesta automática de issue/PR/spec.

Ejemplo:

```json
{
  "name": "hermes",
  "detected": true,
  "adapter": "missing",
  "action": "propose_adapter"
}
```

## MVP CLI

```bash
orq-agent detect --format json
orq-agent models --agent pi --format json
orq-agent smoke --agent pi --model nvidia/openai/gpt-oss-20b --format json
orq-agent exec --agent openclaw --model claude-cli/haiku --task-file task.md --timeout 120 --format json
orq-agent adapters propose --agent hermes --format json
```

## Receipt obligatorio

Toda ejecución real debe producir un receipt JSON con:

- `executed: true|false`
- `agent`
- `model`
- `provider`
- `host`
- `started_at`
- `finished_at`
- `duration_ms`
- `exit_code`
- `stdout_tail`
- `stderr_tail`
- `status`
- `error_class`
- `files_changed`
- `commands_observed`
- `tokens_in/out` si el runner los expone; si no, `unknown`, sin inventar
- `requires_human_confirmation`

## Política de host/minipc

- Pi: diseño, pruebas livianas, smoke tests.
- minipc: builds release, tests largos, benchmarks, si está disponible.
- Si minipc está apagado/intermitente por electricidad, el flujo continúa en Pi con `cargo check` y tests pequeños.
- Nunca bloquear el avance por minipc offline.

## Seguridad

- No leer archivos de secretos.
- No imprimir API keys.
- No tocar servicios vivos sin confirmación.
- Todo comando shell/git/filesystem debe mantener política `rtk_required`.
- Modelos caros o review-only requieren confirmación humana.

## Criterios de aceptación MVP

- Dado un agente instalado con adapter disponible, cuando se ejecuta `orq-agent exec`, entonces el proceso real se invoca y el receipt contiene `executed=true` y `exit_code`.
- Dado un agente detectado sin adapter, cuando se ejecuta `orq-agent detect --propose-missing`, entonces se genera una propuesta de adapter verificable.
- Dado un modelo que devuelve 404, cuando se ejecuta `orq-agent smoke`, entonces el resultado queda marcado como `failed` con `error_class=model_not_found`.
- Dado minipc offline, cuando se pide build pesado, entonces el sistema cae a modo local liviano o marca `skipped_remote_host`, no falla de forma opaca.
- Dado Claude Sonnet/Opus, cuando no hay aprobación explícita, entonces la ejecución queda `blocked`.

## Plan MVP por fases

### Fase 0 — Decisión y tracking

- Abrir issue de implementación del `orq-agent` Rust.
- Marcar el bug actual como bloqueante del multiagente real.
- Mantener el binario Go como `orq` legacy durante la PoC.

### Fase 1 — Crate mínimo

Estructura sugerida:

```text
orq-agent/
  Cargo.toml
  src/
    main.rs
    adapters/
      mod.rs
      pi.rs
      openclaw.rs
      agy.rs
    detect.rs
    exec.rs
    receipt.rs
    policy.rs
    error.rs
```

Crates iniciales:

- `clap` para CLI.
- `tokio` para ejecución async y timeouts.
- `serde`/`serde_json` para contratos.
- `thiserror` para errores de librería.
- `color-eyre` en binario.
- `which` para detectar binarios sin leer secretos.

### Fase 2 — Detect read-only

Implementar:

```bash
orq-agent detect --format json
```

Debe detectar como mínimo:

- `pi`
- `openclaw`
- `agy`
- `hermes`, si existe, pero con estado `deprecated_or_quarantine`
- `claude-code`, solo como gated/review-only

### Fase 3 — Exec real con timeout

Implementar:

```bash
orq-agent exec --agent pi --model <modelo> --task-file <archivo> --timeout 120 --format json
```

Debe capturar:

- `stdout`
- `stderr`
- `exit_code`
- timeout
- errores de binario no encontrado
- recibo JSON

### Fase 4 — Adapter missing proposal

Implementar:

```bash
orq-agent adapters propose --agent <nombre> --format json
```

Debe generar spec/issue para el adapter sin fingir soporte.

### Fase 5 — Integración con Orq legacy

El `orq` actual puede invocar `orq-agent` para ejecución real y conservar routing/ledger mientras se migra progresivamente.

## Próximo paso

Implementar PoC Rust mínimo con adapters iniciales detectados:

1. `pi`
2. `openclaw`
3. `agy`

Hermes queda detectado pero no prioritario; si aparece, debe generar propuesta de adapter o decisión de eliminación.
