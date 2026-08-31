# Historial de Cambios

Notas de release operativas para `agent-orchestrator`, con foco en seguridad, validación y rollback.

## main — desarrollo continuo

### Agregado

- Documento de guardrails para que Orq detecte uso prolongado de Pi, delegación omitida y loops caros: `docs/guardrails-pi-token-control.md`.
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

### Seguridad

- Comandos de seguridad con confirmación explícita cuando corresponde.
- Modelos no verificados o `review_only` marcados como no asignables para ejecución.
- Telemetría Observer best-effort sin guardar secretos en git.
- Los snapshots de capacidad solo guardan métricas agregadas, porcentajes, ventanas y timestamps; no guardan credenciales ni datos crudos de cuenta.

### Validación recurrente

```bash
rtk docker compose run --rm dev go test ./...
rtk orq guard-collision --path .
rtk orq repo check --path .
rtk orq safety check --path .
```

### Rollback

- Revertir el PR correspondiente con `git revert` y reinstalar el binario `orq` desde `main` validado.
