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
| `orq route` | Usa matriz JSON versionada en `orq-agent/config/routing-matrix.json`, soporta `--config`, lookup dinámico por `--task-kind`, certificados (`--cert-dir`), circuit breaker y ruteo consciente de cuotas persistidas (penalización por agotamiento `five_hour` y preferencia gated con `--allow-gated`). | Crítica | Portado completo con observabilidad y circuit breaker. |
| `orq task` | Registry/estados; útil pero con pares agente/modelo rígidos | Alta | Portar tipos de estado y transiciones; corregir soporte dinámico de agentes. |
| `orq agents detect` | Detecta binarios/configs sin secretos | Crítica | Primer comando MVP: `orq-agent detect --format json`. |
| `orq agents configure` | Documentado, pero binario actual no lo acepta correctamente | Media | Replantear en Rust con dry-run obligatorio; no tocar configs sin confirmación. |
| `orq models snapshot` | Útil para capacidad/modelos | Media | Portar cuando `detect` y `models` estén estables. |
| `orq delegate` | Genera handoff, no ejecuta runner | Crítica | Reemplazar semántica por `orq-agent exec`; mantener handoff como modo fallback. |
| `orq run` | Reporta `executed=false` siempre | Crítica | Reemplazar por ejecución real con receipt JSON. |
| `orq receipt` | Valida recibos iniciales | Alta | Portar schemas y validación; Rust debe ser fuente de receipts. |
| `orq quota record` | No existe en Go legacy. | Crítica | Ingesta de snapshots de cuota por proveedor y scope (vía CLI manual o JSON/archivo `@payload.json`) en SQLite. |
| `orq quota report` | No existe en Go legacy. | Crítica | Reporte agregado y filtrado por proveedor del estado de cuota y scopes con timestamps de captura y reset. |
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
9. `adapters`: metadata de adapters detectables vive en `orq-agent/config/adapters-registry.json`; `detect`, `exec`, `models`, `route` y `smoke` aceptan `--adapters-config`.
10. `certify`: MVP genera certificado JSON versionado desde smoke acotado para `(agent, model, task_kind)`.
11. `route` puede consultar certificados con `--cert-dir`: prefiere certificados `certified` exactos y salta certificados `failed`, sin saltarse policy/detection.
12. `quota`: soporte de registro (`quota record`) y consulta (`quota report`) de cuotas en SQLite, con cálculo de reset relativo/absoluto y agregación no-optimista.
13. `policy`: bloqueo de Sonnet/Opus sin aprobación explícita.
14. `models/smoke`: validación runtime de modelos, incluyendo 404/model_not_found.

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
orq-agent detect --adapters-config orq-agent/config/adapters-registry.json --format json
orq-agent certify --agent qwen-code --model qwen3.6-flash --task-kind documentation --output /tmp/orq-cert.json --format json
orq-agent route --task-kind documentation --cert-dir /tmp/orq-certs --format json
orq-agent quota record --provider agy --scope gemini-weekly --remaining-pct 47.17 --format json
orq-agent quota record --json '[{"provider":"codex","scope":"short-term","remaining_pct":22.0,"reset_in_seconds":3600}]' --format json
orq-agent quota report --format json
orq-agent quota report --provider agy --format json
orq-agent adapters propose --agent hermes --format json
```

### Adapters

- `pi`: prioritario; también encapsula modelos NVIDIA por ahora.
- `openclaw`: prioritario como candidato Claude-compatible.
- `agy`: prioritario para tareas baratas/revisión.
- `hermes`: detectar y marcar `deprecated_or_quarantine`.
- `claude-code`: detectar y bloquear si modelo requiere aprobación.

## Contrato de Quota y Quota Unknown

Para la integración del router (Parte 2) y la observabilidad de capacidad:

### 1. Semántica y significado de `quota_unknown`
- **Nivel de Scope**:
  - Se genera automáticamente cuando un snapshot se ingresa sin porcentajes (`remaining_pct` y `used_pct` ausentes) y sin un `--status` explícito, o cuando se indica explícitamente `quota_unknown`.
  - Significa que el proveedor carece de detector automatizado de cuota o que la medición no estuvo disponible durante la captura.
- **Nivel de Proveedor (Estado Agregado)**:
  - Proveedores conocidos sin registros previos reportan `status: "quota_unknown"` y `scopes: []`.
  - Si un proveedor contiene al menos un scope en `quota_unknown` (y ninguno en `exhausted` o `warning`), el estado general del proveedor es `quota_unknown`. Esto evita optimismo falso (marcar `ok`) cuando parte de la capacidad del proveedor no puede ser verificada.

### 2. Jerarquía de agregación en reportes
La determinación del estado consolidado por proveedor sigue esta precedencia determinista:
1. **`exhausted`**: si cualquier scope está en `exhausted`, `exceeded` o tiene `remaining_pct == 0.0`.
2. **`warning`**: si ningún scope está agotado pero al menos uno reporta `warning`.
3. **`quota_unknown`**: si la lista de scopes está vacía o al menos un scope está en `quota_unknown`.
4. **`ok`**: únicamente cuando todos los scopes están evaluados y en estado saludable (`ok`).

### 3. Normalización y precedencia
- **Normalización de proveedores**: los nombres de proveedor se normalizan automáticamente a minúsculas (`trim().to_lowercase()`) tanto en `quota record` como en los filtros de `quota report`.
- **Precedencia de almacenamiento**: el parámetro `--db-path` toma precedencia sobre la variable `ORQ_STATE_DB`, la cual a su vez sobreescribe la ruta SQLite por defecto (`~/.local/state/orq-agent/orq-state.sqlite`).

### 4. Integración con el Router (Routing consciente de cuotas)
El comando `route` consume los snapshots más recientes de la base de estado (`quota_snapshots`) aplicando las siguientes reglas deterministas:
1. **Penalización por agotamiento (`five_hour` / inmediato)**: Si un candidato tiene su grupo `five_hour` o cualquier scope en `0.0%` o `exhausted`/`exceeded`, es penalizado y evitado en favor de una alternativa permitida con cuota disponible. Si todos los candidatos están agotados, se mantiene el fallback por defecto.
2. **Preferencia de proveedores `gated` con `--allow-gated`**: Cuando el usuario autoriza modelos restringidos con `--allow-gated`, si el candidato gated (ej. `claude-code`) cuenta con cuota semanal alta (`>= 50%`) y saludable, el router lo prioriza sobre candidatos estándar.
3. **Neutralidad de `quota_unknown`**: Proveedores sin snapshots o con scopes en `quota_unknown` no sufren penalizaciones; se consideran saludables y preservan el orden y criterios definidos en la matriz de ruteo base.
4. **Prioridad de Circuit Breakers y Certificados**: El filtrado por Circuit Breaker (cooldowns de fallos/timeouts) y la evaluación estricta de políticas de seguridad se ejecutan antes de la ponderación de cuotas.

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
