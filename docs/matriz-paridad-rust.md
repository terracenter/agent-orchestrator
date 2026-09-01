# Matriz de paridad Go → Rust para `orq-agent`

## Estado

Borrador inicial para decidir migración progresiva o completa a Rust.

## Objetivo

Definir qué capacidades del `orq` Go actual deben conservarse, cuáles deben delegarse al nuevo `orq-agent` Rust y cuáles son candidatas a reemplazo completo.

RFC relacionado: [`rfc-orq-agent-rust.md`](rfc-orq-agent-rust.md)
ADR relacionado: [`adr-migracion-rust.md`](adr-migracion-rust.md)

## Regla de decisión

- **MVP Rust primero**: ejecutar agentes reales con receipts.
- **Go legacy sigue vivo** hasta que exista paridad suficiente.
- **Migración completa** solo si Rust cubre rutas críticas sin regresión.

## Matriz por comando/capacidad

| Capacidad Go actual | Estado actual | Prioridad Rust | Acción propuesta |
|---|---:|---:|---|
| `orq route` | Usa matriz JSON versionada en `orq-agent/config/routing-matrix.json`, soporta `--config`, lookup dinámico por `--task-kind`, intersección con agentes detectados y policy gating. | Alta | Agregar certificados/capacidad y extraer registro de adapters/modelos. |
| `orq task` | Registry/estados; útil pero con pares agente/modelo rígidos | Alta | Portar tipos de estado y transiciones; corregir soporte dinámico de agentes. |
| `orq agents detect` | Detecta binarios/configs sin secretos | Crítica | Primer comando MVP: `orq-agent detect --format json`. |
| `orq agents configure` | Documentado, pero binario actual no lo acepta correctamente | Media | Replantear en Rust con dry-run obligatorio; no tocar configs sin confirmación. |
| `orq models snapshot` | Útil para capacidad/modelos | Media | Portar cuando `detect` y `models` estén estables. |
| `orq delegate` | Genera handoff, no ejecuta runner | Crítica | Reemplazar semántica por `orq-agent exec`; mantener handoff como modo fallback. |
| `orq run` | Reporta `executed=false` siempre | Crítica | Reemplazar por ejecución real con receipt JSON. |
| `orq receipt` | Valida recibos iniciales | Alta | Portar schemas y validación; Rust debe ser fuente de receipts. |
| `orq observer sync` | Sincroniza ledger con Observer | Media | Mantener Go al inicio; integrar Rust emitiendo JSONL compatible. |
| `orq record/status` | Ledger operativo | Media | Mantener compatible; Rust debe poder escribir eventos seguros. |
| `orq audit session` | Auditoría de cumplimiento | Media | Portar luego; depende de receipts y trace confiable. |
| `orq trace` | Tracking de sesión/comandos | Media | Portar luego; MVP solo escribe receipts/eventos mínimos. |
| `orq doctor` | Diagnóstico de entorno | Media | Unificar con `orq-agent detect`; posible reemplazo posterior. |
| `orq budget` | Guardrails de presupuesto | Media | Portar policy mínima para bloquear modelos caros. |
| `orq roadmap check` | Gobierno de fases | Baja | Mantener en Go hasta estabilizar Rust. |
| `orq safety` | Checks de seguridad | Media | Portar reglas relevantes a policy Rust. |
| `orq handoff` | Gestión documental | Baja | Mantener en Go; Rust no debe empezar por documentación. |
| `orq inbox` | Feedback/inbox | Baja | Mantener Go inicialmente. |
| `orq repo/review/docs/config/vault-order` | Utilidades auxiliares | Baja | No portar en MVP. |

## Paridad mínima para aprobar migración completa

Antes de considerar reemplazar `orq` Go por Rust, deben estar cubiertos:

1. `detect`: detectar `pi`, `openclaw`, `agy`, `hermes`, `claude-code` sin leer secretos.
2. `exec`: ejecución real con timeout y receipt.
3. `receipt`: schema estable y validador.
4. `task`: estados y transiciones sin pares agente/modelo rígidos.
5. `record/status`: ledger compatible con Observer.
6. `route`: decisiones equivalentes para tareas mecánicas, código, documentación y críticas. La matriz vive en config JSON versionada y puede reemplazarse con `--config`.
7. `policy`: reglas de aprobación/bloqueo viven en `orq-agent/config/policy.json`; `exec` y `smoke` aceptan `--policy-config`.
8. `models`: catálogo de modelos vive en `orq-agent/config/models-catalog.json`; `models` acepta `--config`.
7. `policy`: bloqueo de Sonnet/Opus sin aprobación explícita.
8. `models/smoke`: validación runtime de modelos, incluyendo 404/model_not_found.

## MVP Rust concreto

### Comandos

```bash
orq-agent detect --format json
orq-agent models --agent pi --format json
orq-agent smoke --agent pi --model nvidia/openai/gpt-oss-20b --format json
orq-agent exec --agent pi --model nvidia/openai/gpt-oss-20b --task-file task.md --timeout 120 --format json
orq-agent route --task-kind documentation --config orq-agent/config/routing-matrix.json --format json
orq-agent exec --agent qwen-code --model qwen3.6-flash --task-file /tmp/task.md --policy-config orq-agent/config/policy.json --format json
orq-agent models --agent qwen-code --config orq-agent/config/models-catalog.json --format json
orq-agent adapters propose --agent hermes --format json
```

### Adapters

- `pi`: prioritario; también encapsula modelos NVIDIA por ahora.
- `openclaw`: prioritario como candidato Claude-compatible.
- `agy`: prioritario para tareas baratas/revisión.
- `hermes`: detectar y marcar `deprecated_or_quarantine`.
- `claude-code`: detectar y bloquear si modelo requiere aprobación.

## Riesgos de migración completa

- Regresión en comandos auxiliares que hoy sí funcionan.
- Pérdida de compatibilidad con ledger/Observer.
- Mayor tiempo de compilación en Pi.
- Tentación de reescribir demasiado antes de cerrar el bug principal.

## Recomendación

Avanzar con Rust, pero de forma controlada:

1. `orq-agent` Rust como dispatcher real.
2. Integración desde `orq` Go hacia `orq-agent`.
3. Medir paridad.
4. Si Rust demuestra ejecución real, receipts y política más robusta, planificar migración completa como fase posterior.
