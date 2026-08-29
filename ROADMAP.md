# ROADMAP — agent-orchestrator

> Proyecto AGPL-3.0-or-later. El roadmap es parte del contrato público del proyecto: cada PR funcional relevante debe actualizar este archivo o justificar explícitamente por qué no aplica.

## Visión

`orq` debe ser una capa local-first de control multi-agente: detecta herramientas, elige el agente/modelo más barato suficiente, delega ejecución, registra evidencia, valida cumplimiento y abre issue/PR cuando el contrato operativo falla.

## Principios no negociables

1. **Costo mínimo suficiente:** evitar Pi/OpenAI/Codex cuando exista runner de suscripción o gratuito suficiente.
2. **Evidencia antes que opinión:** toda tarea debe cerrar con recibo verificable.
3. **RTK por defecto:** comandos de terminal/git/filesystem deben usar `rtk` cuando esté disponible; si falta, instalarlo o guiar al usuario.
4. **Supervisor ≠ ejecutor:** si un agente caro o limitado actúa como supervisor, debe detenerse tras delegar.
5. **Dry-run antes de mutación:** acciones destructivas o sensibles requieren confirmación humana.
6. **AGPL visible:** licencia, contribución, seguridad, roadmap y releases deben estar presentes y actualizados.

## Estado actual

- `orq budget`: guardrails de presupuesto y compactación manual en Pi.
- `orq route`: clasificación básica y recomendación de agente/modelo.
- `orq delegate`: genera prompt seguro con `rtk_required=true`, pero aún no ejecuta ni impone stop duro.
- `orq receipt/session`: validación inicial de recibos y checks.
- Plantilla base: incluye README, README.en, CONTRIBUTING, SECURITY, RELEASES, CI y Makefile; falta reforzar ROADMAP como obligatorio.

## Fase 1 — Guardrails de sesión y presupuesto

- [x] Exponer `compact_capability` por agente.
- [x] Bloquear Pi/API con `manual_compact_stop=true` cuando falta `/compact`.
- [x] Permitir continuación explícita con `--compact-applied` tras compactación manual.
- [ ] Registrar eventos de presupuesto en ledger/traza de sesión.

## Fase 2 — Delegación real y stop obligatorio

- [x] Agregar `must_stop_for_delegation=true` cuando el agente actual no debe ejecutar.
- [x] Exponer `supervisor_only=true` y `execution_agent_allowed=false` para Pi/Codex bajo presión de presupuesto.
- [ ] Hacer que `orq delegate` pueda escribir handoff/receipt en archivo.
- [ ] Validar que Pi no ejecute trabajo largo si `delegate` recomendó AGY/OpenClaw/NVIDIA/local.

## Fase 3 — Tracking de agentes

- [ ] Diseñar `orq trace start/status/stop`.
- [ ] Registrar comandos ejecutados, archivos leídos/modificados, tests, commits, PRs e issues.
- [ ] Registrar descubrimientos nuevos que deban alimentar memoria/configuración.
- [ ] Soportar ingestión de recibos de agentes externos.

## Fase 4 — Auditoría de cumplimiento

- [ ] Implementar `orq audit session`.
- [ ] Detectar comandos sin `rtk` cuando `rtk_required=true`.
- [ ] Detectar ejecución en agente caro cuando la política exigía delegación.
- [ ] Detectar mutaciones sin dry-run/confirmación cuando aplique.
- [ ] Emitir findings con códigos estables y severidad.

## Fase 5 — Issue/PR automático ante fallas

- [ ] Implementar generador de issue desde findings de auditoría.
- [ ] Incluir comportamiento esperado, comportamiento actual, evidencia y criterios de aceptación.
- [ ] Evaluar generación automática de rama/PR para fixes mecánicos.
- [ ] Requerir revisión humana antes de mergear cambios de guardrails.

## Fase 6 — Installer y doctor para usuarios básicos

- [ ] Crear instalador simple estilo `curl | bash`, con modo interactivo y dry-run.
- [ ] Implementar `orq doctor` para detectar `rtk`, `git`, `gh`, `vg`, `openclaw`, `agy`, `hermes`, `codex` y runners futuros.
- [ ] Si falta `rtk`, ofrecer instalación automática o instrucciones manuales.
- [ ] Si falta una herramienta opcional, marcar estado `missing`, `degraded` o `blocked` según impacto.
- [ ] Crear backups antes de tocar configuración de agentes.

## Fase 7 — Configuración multi-agente

- [ ] Implementar `orq agents detect`.
- [ ] Implementar `orq agents configure <agent|all>`.
- [ ] Configurar prompts/hooks de `rtk_required` cuando el agente lo soporte.
- [ ] Documentar OpenClaw, AGY, Hermes y Codex como runners independientes.
- [ ] No asumir credenciales; pedir confirmación antes de modificar configs.

## Fase 8 — Piloto OpenClaw + vault

- [ ] Validar si OpenClaw usa Claude Pro/suscripción sin gasto adicional.
- [ ] Ejecutar tareas pequeñas del vault con OpenClaw como runner principal.
- [ ] Pi queda como supervisor corto y validador de recibos.
- [ ] Medir si el flujo reduce consumo de Pi/OpenAI sin perder seguridad.

## Política de actualización

Todo PR que agregue comando, guardrail, integración, instalador, tracking o política de ejecución debe actualizar este roadmap o indicar en el PR: `Roadmap: no aplica` con justificación.
