# ROADMAP — agent-orchestrator

> Proyecto AGPL-3.0-or-later. El roadmap es parte del contrato público del proyecto: cada PR funcional relevante debe actualizar este archivo o justificar explícitamente por qué no aplica.

## Visión

`orq` debe seguir siendo una capa local-first de control multi-agente: detecta herramientas, elige el agente/modelo más barato suficiente, delega ejecución real, registra evidencia, valida cumplimiento y abre issue/PR cuando el contrato operativo falla.

La visión funcional no cambia. Cambia la implementación objetivo: **Orq será Rust-first y tenderá a un solo binario Rust**. Go queda como implementación legacy temporal y referencia de paridad mientras se migra por slices verificables.

## Principios no negociables

1. **Costo mínimo suficiente:** evitar Pi/OpenAI/Codex cuando exista runner de suscripción o gratuito suficiente.
2. **Evidencia antes que opinión:** toda tarea debe cerrar con recibo verificable.
3. **RTK por defecto:** comandos de terminal/git/filesystem deben usar `rtk` cuando esté disponible; si falta, instalarlo o guiar al usuario.
4. **Supervisor ≠ ejecutor:** si un agente caro o limitado actúa como supervisor, debe detenerse tras delegar.
5. **Dry-run antes de mutación:** acciones destructivas o sensibles requieren confirmación humana.
6. **AGPL visible:** licencia, contribución, seguridad, roadmap y releases deben estar presentes y actualizados.

## Estado actual

- Visión Rust-first aprobada en [`docs/adr-migracion-rust.md`](docs/adr-migracion-rust.md): Go queda congelado para features nuevas salvo compatibilidad/migración.
- `orq-agent` Rust ya ejecuta agentes reales con receipts JSON, timeouts, process group kill, ring buffer y smoke real Qwen.
- `orq run --execute` en Go consume temporalmente `orq-agent exec`; esta capa es puente de migración, no arquitectura final.
- `orq budget`: guardrails de presupuesto y compactación manual en Pi.
- `orq route`: clasificación básica, recomendación de agente/modelo y ajuste opcional por snapshots de capacidad agregados vía `--capacity-file`.
- `orq observer send-capacity`: envío manual de snapshots de capacidad/cuota a Observer LLM usando el token de host existente.
- `orq delegate`: genera prompt seguro con `rtk_required=true`, pero aún no ejecuta ni impone stop duro.
- `orq receipt/session`: validación inicial de recibos y checks.
- Plantilla base: incluye README, README.en, CONTRIBUTING, SECURITY, RELEASES, CI y Makefile; falta reforzar ROADMAP como obligatorio.
- Documentación como changelog operativo: todo issue/PR/entregable cerrado debe reflejarse en ROADMAP, RELEASES y docs relevantes.

## Fase 1 — Guardrails de sesión y presupuesto

- [x] Exponer `compact_capability` por agente.
- [x] Bloquear Pi/API con `manual_compact_stop=true` cuando falta `/compact`.
- [x] Permitir continuación explícita con `--compact-applied` tras compactación manual.
- [x] Registrar eventos de presupuesto en ledger/traza de sesión.

## Fase 2 — Delegación real y stop obligatorio

- [x] Agregar `must_stop_for_delegation=true` cuando el agente actual no debe ejecutar.
- [x] Exponer `supervisor_only=true` y `execution_agent_allowed=false` para Pi/Codex bajo presión de presupuesto.
- [x] Hacer que `orq delegate` pueda escribir handoff/receipt en archivo.
- [x] Validar que Pi no ejecute trabajo largo si `delegate` recomendó AGY/OpenClaw/NVIDIA/local.

## Fase 3 — Tracking de agentes

- [x] Registrar eventos de ledger en Observer LLM de forma best-effort sin bloquear la tarea principal.
- [x] Enviar snapshots manuales de capacidad/cuota a Observer LLM mediante `orq observer send-capacity` (#68, PR #76).
- [x] Diseñar `orq trace start/status/stop/list/record` con Manager y modelos (commit d6b47e5).
- [x] Registrar comandos ejecutados, archivos leídos/modificados, tests, commits, PRs e issues (TraceEvent).
- [x] Registrar descubrimientos nuevos que deban alimentar memoria/configuración (EventTypeDiscovery).
- [x] Soportar ingestión de recibos de agentes externos (orq trace record --type discovery).

## Fase 4 — Auditoría de cumplimiento

- [x] Implementar `orq audit session`.
- [x] Detectar comandos sin `rtk` cuando `rtk_required=true`.
- [x] Detectar ejecución en agente caro cuando la política exigía delegación.
- [x] Detectar mutaciones sin dry-run/confirmación cuando aplique.
- [x] Emitir findings con códigos estables y severidad.

## Fase 5 — Issue/PR automático ante fallas

- [x] Implementar generador de issue desde findings de auditoría.
- [x] Incluir comportamiento esperado, comportamiento actual, evidencia y criterios de aceptación.
- [x] Evaluar generación automática de rama/PR para fixes mecánicos (decisión MVP: generar borrador; no crear remoto automáticamente).
- [x] Requerir revisión humana antes de mergear cambios de guardrails.

## Fase 6 — Installer y doctor para usuarios básicos

- [x] Crear instalador simple estilo `curl | bash`, con modo interactivo y dry-run.
- [x] Implementar `orq doctor` para detectar `rtk`, `git`, `gh`, `vg`, `openclaw`, `agy`, `hermes`, `claude`.
- [x] Si falta `rtk`, ofrecer instalación automática o instrucciones manuales.
- [x] Si falta una herramienta opcional, marcar estado `missing`, `degraded` o `blocked` según impacto.
- [x] Crear backups antes de tocar configuración de agentes.

## Fase 7 — Configuración multi-agente

- [x] Implementar `orq agents detect` para inspección segura de presencia/rutas de runners.
- [x] Detectar Qwen Code de forma segura por binario/directorio y registrar perfiles Bailian iniciales sin leer secretos (#81).
- [x] Emitir `orq models snapshot` con capacidades/modelos fechados, fuentes separadas y evidencia segura para aprendizaje posterior (#81).
- [x] Ajustar routing no crítico con snapshots de capacidad agregados desde archivo JSON explícito (`orq route --capacity-file`) (#68, PR #77).
- [x] Implementar `orq agents configure <agent|all>`.
- [x] Configurar prompts/hooks de `rtk_required` cuando el agente lo soporte.
- [x] Documentar OpenClaw, AGY, Hermes y Codex como runners independientes.
- [x] No asumir credenciales; pedir confirmación antes de modificar configs.

## Fase 8 — Piloto OpenClaw + vault

- [x] Registrar contexto operativo: OpenClaw está operativo por Telegram y el modelo default configurado es Haiku.
- [x] Garantizar que cada tarea autorizada desde el router móvil/OpenClaw pase por `route.Decide` y deje auditoría `routed_by_orq` (#82).
- [ ] Validar si OpenClaw usa Claude Pro/suscripción sin gasto adicional.
- [ ] Ejecutar tareas pequeñas del vault con OpenClaw como runner principal.
- [ ] Pi queda como supervisor corto y validador de recibos.
- [ ] Medir si el flujo reduce consumo de Pi/OpenAI sin perder seguridad.

## Fase 9 — Rust-first: `orq-agent` como núcleo del nuevo `orq`

Justificación de salto: **seguridad/costo y simplificación arquitectónica**. La Fase 8 depende de ejecución real verificable; el Orq Go original genera handoffs pero no garantiza que los runners trabajen. Rust ya validó la ejecución real, por lo que mantener una cadena permanente Go -> Rust -> agente no tiene sentido como arquitectura final.

RFC: [`docs/rfc-orq-agent-rust.md`](docs/rfc-orq-agent-rust.md)
Certificación: [`docs/model-capability-certification.md`](docs/model-capability-certification.md)

- [x] Documentar RFC inicial para `orq-agent` Rust.
- [x] Registrar bug funcional: Orq no puede asegurar ejecución real de todos los agentes usando solo Orq.
- [ ] Crear issue de implementación del MVP Rust.
- [x] Crear ADR de decisión: migración completa a Rust mediante slices verificables; Go queda legacy temporal.
- [x] Crear matriz de paridad de comandos para validar una posible migración completa (`docs/matriz-paridad-rust.md`).
- [x] Crear crate/binario `orq-agent` sin reemplazar todavía el `orq` Go legacy.
- [x] Implementar `orq-agent detect --format json` sin leer secretos.
- [x] Implementar adapters iniciales detectados: `pi`, `openclaw`, `agy`, `qwen-code`.
- [x] Marcar `hermes` como `deprecated_or_quarantine` hasta decidir si se elimina.
- [ ] Mantener NVIDIA solo bajo Pi por ahora; no tratar `nvidia-api` como agente local si no hay runner real.
- [x] Implementar MVP inicial de `orq-agent exec` con timeout, stdout/stderr, exit code y receipt JSON; incluye kill por process group Unix, streaming/ring buffer acotado y salida parcial en timeout.
- [x] Implementar `orq-agent models --agent <agent> --format json` para descubrimiento seguro de modelos sin leer secretos.
- [x] Implementar `orq-agent smoke --agent <agent> --model <model> --format json` con receipts de validación y verificación de marcador.
- [ ] Implementar certificación progresiva de capacidades agente/modelo por tipo de tarea usando receipts históricos.
- [ ] Hacer que `orq route` explique selección por evidencia certificada y no por catálogo estático.
- [ ] Implementar propuesta automática de adapter/issue/PR cuando se detecta un agente sin soporte.
- [x] Integrar `orq run --execute` con `orq-agent exec` para ejecución real progresiva y consumo de receipts JSON.
- [ ] Soportar política de host: Pi para pruebas livianas; minipc para builds pesados solo si está disponible por inestabilidad eléctrica.

## Fase 10 — Portar Orq completo a Rust por slices

Objetivo: eliminar la capa Go -> Rust y consolidar Orq como un solo binario Rust. El código Go queda como referencia de comportamiento hasta completar paridad; no recibe features nuevas salvo compatibilidad crítica.

Slices de migración propuestos como PRs independientes:

1. [ ] **PR Rust core/CLI**: reestructurar crate para producir `orq` Rust, mantener compatibilidad temporal con `orq-agent`, módulos `commands/`, `core/`, `adapters/`, `receipts/`.
2. [ ] **PR ejecución**: portar `orq run`, `agents detect`, `models`, `smoke` y preparar `certify`; mantener receipts actuales como contrato.
3. [ ] **PR routing**: portar `classify`, `route`, políticas de costo, gating Sonnet/Opus y selección por evidencia certificada.
4. [ ] **PR estado local**: portar `task`, `record`, `status`, ledger JSONL y transiciones válidas.
5. [ ] **PR Observer/receipts**: portar `observer sync/status/send-capacity`, `receipt create/verify/from-pr` y compatibilidad con SGE Observer.
6. [ ] **PR auditoría/seguridad**: portar `safety`, `audit`, `session`, `trace`, `review 4r`, guardrails de RTK y dry-run.
7. [ ] **PR installer/docs**: actualizar instalación para binario Rust, docs de uso, README, RELEASES y retirar referencias Go como stack principal.
8. [ ] **PR retiro Go**: archivar o eliminar `cmd/orq` e `internal/*` Go cuando la matriz de paridad y tests Rust cubran comandos críticos.

Criterio de aceptación por slice:

- Tests Rust verdes (`rtk cargo test`; `rtk cargo clippy -- -D warnings` cuando aplique).
- Comparación contra comportamiento Go legacy o justificación explícita de cambio.
- ROADMAP/docs actualizados.
- Receipt/evidencia Orq para ejecución real cuando aplique.
- Sin uso de Sonnet/Opus salvo aprobación explícita.

## Política de orden de fases

Las fases son contrato de ejecución: por defecto se trabaja en la fase pendiente más temprana antes de avanzar a fases posteriores.

Solo se permite saltar una fase pendiente por justificación explícita de:

1. **seguridad**; o
2. **optimización/costo**.

Antes de iniciar o mergear trabajo de una fase posterior, ejecutar:

```bash
orq roadmap check --phase <n>
```

Si el comando reporta pendientes en fases anteriores, el PR debe detenerse o declarar el override con `--override security`, `--override optimization` o `--override cost` y explicar la evidencia en la descripción del PR.

## Política de actualización

Todo PR que agregue comando, guardrail, integración, instalador, tracking o política de ejecución debe actualizar este roadmap o indicar en el PR: `Roadmap: no aplica` con justificación.

Además, la documentación funciona como changelog operativo: ROADMAP, RELEASES, README y docs de uso deben quedar alineados con cada issue/PR/entregable cerrado antes de darlo por terminado.
