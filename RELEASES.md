# Historial de Cambios

Notas de release operativas para `agent-orchestrator`, con foco en seguridad, validación y rollback.

## main — desarrollo continuo

### Agregado

- `orq exec` y `orq delegate --execute` aplican un techo real de gasto
  diario/mensual (`orq-agent/config/budget.json`, flag `--budget-config`)
  contra el `cost_hint` del catalogo de modelos (`--models-config`),
  ortogonal a `approval_required_model_patterns`: un modelo caro que no
  contenga `sonnet`/`opus` en el nombre ahora puede rechazarse por costo
  real. El gasto se persiste en un ledger SQLite nuevo (`budget_ledger`,
  migracion 7 del state DB) agrupado por dia/mes calendario UTC. Sin
  `cost_hint` configurado para el modelo, o sin techos en `budget.json`
  (default builtin), el gate no bloquea nada y la politica por patron sigue
  siendo la unica autoridad (#183).
- `orq delegate` registra cada transicion de estado en un event log JSONL
  auditable (`ORQ_EVENT_LOG_PATH`, default `$HOME/.local/state/orq-agent/events.jsonl`)
  y aplica un watchdog anti-loop minimo que bloquea reintentos cuando el
  mismo `--task-id` acumula 3+ eventos fallidos/bloqueados/timeout en 600s,
  como base auditable para reroute adaptivo (#172).
- `orq` pasa a ser Rust-only en sus rutas operativas: `scripts/install.sh`, `make build`, `make install`, Docker, CI y las guias instalan, prueban y documentan los binarios Rust `orq` y `orq-agent` (#194). El codigo Go queda archivado como referencia de paridad y no se instala.
- Estándar profesional de presentación GitHub en `docs/github-repository-standard.md`.
- README en español e inglés rediseñados con badges, estado, quickstart, arquitectura resumida y política documental.
- Plantillas GitHub piloto para issues de bug/documentación, alineadas con PRs con evidencia Orq.
- Documento de guardrails para que Orq detecte uso prolongado de Pi, delegación omitida y loops caros: `docs/guardrails-pi-token-control.md`.
- `scripts/install.sh` instala `orq` con modo interactivo, `--dry-run`, advertencia si falta `rtk` y backup antes de reemplazar binarios.
- `orq audit issue-from-session` genera borradores de issue desde findings de auditoría con evidencia, comportamiento esperado/actual, criterios de aceptación y revisión humana obligatoria para guardrails.
- `orq audit session` audita sesiones `orq trace`, detecta comandos sin `rtk`, ejecución directa por agente caro/supervisor y mutaciones sin `--dry-run` o confirmación, con findings de código estable y severidad.
- `orq delegate` valida con pruebas que Pi quede en modo supervisor y no pueda ejecutar cuando la recomendación apunta a AGY/local-or-cheap sin recibo externo.
- `orq budget` ahora registra cada decisión de presupuesto en el ledger como evento `budget_<action>` para trazabilidad de sesión.
- `orq budget` ahora bloquea la continuación con `action=compactar_manual` y `manual_compact_stop=true` cuando el agente actual no puede compactar automáticamente.
- `orq budget` ahora acepta `--agent` y distingue capacidad de compactación por agente/sesión; si no puede compactar automáticamente, instruye al usuario a ejecutar `/compact`.
- `orq budget` ahora emite `preflight_compact_required=true` y prompt `/compact` en todas las decisiones.
- `orq route` expone `rtk_required=true` y `orq delegate` marca el prefijo `rtk` como obligatorio para comandos.
- `orq route --capacity-file` ajusta decisiones no críticas con snapshots agregados de capacidad/cuota, preservando `SecurityOverride`.
- `orq delegate` ahora declara `status=not_executed` si solo generó prompt y no existe recibo de ejecución externa.
- `orq observer send-capacity` envía snapshots manuales de capacidad/cuota a Observer LLM mediante `X-Host-Token`.
- `orq observer cost` separa costo estimado/subscription de costo real facturado configurado por el usuario.
- Orquestación CLI `orq` para rutas de agentes, tareas, handoffs, recibos RDD y auditorías seguras.
- Auditorías recurrentes read-only: PRs, issues, modelos y worktrees.
- Plantillas cacheables para handoffs con validación de contexto volátil.
- Recibos verificables con métrica `human_edits_required_value`.
- Router móvil seguro para Telegram/WireGuard en `internal/mobile`.
- Puente MVP OpenClaw/Telegram -> Orq: cada tarea autorizada pasa por `route.Decide` y la auditoría registra `routed_by_orq`, categoría, agente y modelo recomendados (#82).
- `orq agents detect` reconoce Qwen Code (`qwen-code`) de forma segura por binario/directorio sin leer secretos, y el registry incluye perfiles Bailian iniciales para `qwen3.8-max`, `qwen3.5`, `qwen3.6` y `qwen3.7-plus` (#81).
- `orq models snapshot` emite snapshots fechados de capacidades/modelos con fuentes (`registry`, `empirical`, `status`) y notas de seguridad para aprendizaje/scoring posterior (#81).
- `orq roadmap check` valida el orden de fases del ROADMAP y bloquea trabajo de fases futuras salvo override explícito de seguridad u optimización/costo (#87).
- `orq agents configure <agent|all>` configura guardrails de `rtk_required` en runners independientes (openclaw, agy, hermes, claude-code) con modo dry-run por defecto, confirmación explícita vía `--yes` y backup automático de archivos existentes antes de modificarlos. Documenta runners independientes en `docs/uso.md` con política de no asumir credenciales ni modificar configuración real sin confirmación (Fase 7).
- `orq score` resume el ledger por agente/modelo y penaliza `not_executed` sin contarlo como éxito, preparando scoring personalizado por usuario/tarea/repo (#81).

### Seguridad

- Comandos de seguridad con confirmación explícita cuando corresponde.
- Modelos no verificados o `review_only` marcados como no asignables para ejecución.
- Telemetría Observer best-effort sin guardar secretos en git.
- Los snapshots de capacidad solo guardan métricas agregadas, porcentajes, ventanas y timestamps; no guardan credenciales ni datos crudos de cuenta.

### Validación recurrente

```bash
rtk docker compose run --rm dev cargo test --manifest-path orq-agent/Cargo.toml
rtk orq guard-collision --path .
rtk orq repo check --path .
rtk orq safety check --path .
```

### Rollback

- Revertir el PR correspondiente con `git revert` y reinstalar el binario `orq` desde `main` validado.
