# Historial de Cambios

Notas de release operativas para `agent-orchestrator`, con foco en seguridad, validación y rollback.

## main — desarrollo continuo

### Agregado

- Documento de guardrails para que Orq detecte uso prolongado de Pi, delegación omitida y loops caros: `docs/guardrails-pi-token-control.md`.
- Orquestación CLI `orq` para rutas de agentes, tareas, handoffs, recibos RDD y auditorías seguras.
- Auditorías recurrentes read-only: PRs, issues, modelos y worktrees.
- Plantillas cacheables para handoffs con validación de contexto volátil.
- Recibos verificables con métrica `human_edits_required_value`.
- Router móvil seguro para Telegram/WireGuard en `internal/mobile`.

### Seguridad

- Comandos de seguridad con confirmación explícita cuando corresponde.
- Modelos no verificados o `review_only` marcados como no asignables para ejecución.
- Telemetría Observer best-effort sin guardar secretos en git.

### Validación recurrente

```bash
rtk docker compose run --rm dev go test ./...
rtk orq guard-collision --path .
rtk orq repo check --path .
rtk orq safety check --path .
```

### Rollback

- Revertir el PR correspondiente con `git revert` y reinstalar el binario `orq` desde `main` validado.
