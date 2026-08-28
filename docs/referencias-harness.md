# Referencias externas para el estándar operativo

> [!IMPORTANT]
> Este documento registra ideas útiles de otros harnesses sin copiarlas a ciegas. Regla: adoptar lo probado, adaptar lo útil y descartar lo pesado.

## DeepSeek Harness

Fuente revisada localmente desde `https://github.com/deepseek-ai/deepseek-harness`.

### Ideas a adoptar

| Idea | Motivo | Aplicación en `orq` |
|---|---|---|
| Safety notice explícito | Declara riesgos reales de agentes que ejecutan comandos, tocan archivos y leen credenciales. | `SECURITY.md` y comandos peligrosos deben explicar límites; no vender sandbox como seguridad total. |
| Least privilege | Reduce daño por errores de modelo o plugin. | Ejecutar agentes con permisos mínimos y rutas explícitas. |
| Backups antes de operar sobre archivos importantes | Evita pérdida por acciones automáticas. | Guardias para vault/repos y evidencia antes de cambios grandes. |
| Sesiones/workspaces separados para benchmarks | Evita contaminación de resultados. | Worktrees/sesiones aisladas para tareas delegadas. |
| Documentación de arquitectura antes de tocar núcleo | Evita parches superficiales. | `orq` debe exigir auditoría cuando se toca routing/loop/guardias. |
| Evidencia proporcional al cambio | No correr todo siempre, pero sí pruebas relevantes. | `orq review 4r` puede sugerir tests según superficie tocada. |
| Model-visible = logged | Todo input que llega al modelo debe ser reconstruible. | Registrar prompt/contexto mínimo y decisión en Observer/ledger sin secretos. |
| Misconfiguración falla temprano | Evita estados silenciosos peligrosos. | Config inválida debe fallar en `orq config`, salvo telemetría no bloqueante. |

### Ideas a adaptar

| Idea | Adaptación |
|---|---|
| Everything-is-a-plugin | Útil como inspiración, pero no meter complejidad TypeScript/Cordis. En Go: interfaces pequeñas y adapters explícitos. |
| Cobertura estricta por paquete | Bueno para núcleo crítico; no imponer 100% global que genere tests basura. |
| Real-API tests self-skip sin key | Adoptar para proveedores; nunca fallar CI por ausencia de key secreta. |
| Snapshot/replay | Útil para decisiones de routing y prompts; implementar simple antes de sofisticar. |
| Capability seams | Adoptar como contratos claros: provider, tool, memory, observer, guard. |

### Ideas a descartar para este workspace

| Idea | Razón |
|---|---|
| Runtime Node/pnpm como base del harness | Viola la preferencia operativa del workspace para servicios propios: Go nativo y bajo bloat. |
| Arquitectura excesivamente plugin-first desde el día 1 | Puede crear burocracia antes de tener contratos estables. |
| Compatibilidad rota constante de developer preview | Aceptable en upstream experimental, no como estándar para repos operativos de Freddy. |

## Gentleman Programming / GentleAI

Pendiente de extracción detallada desde NotebookLM. Ideas ya incorporadas como hipótesis:

- no acumular issues/PRs hasta bloquear avance;
- hacer auditoría arquitectónica cuando hay síntomas relacionados;
- usar revisión 4R: Legibilidad, Robustez, Riesgo y Seguridad;
- result-driven development: abstraer complejidad para que el usuario llegue al resultado sin aprender detalles internos innecesarios.

## Autoridad de entrega

De Gentle Pi se adopta un principio clave: la revisión del harness es evidencia informativa; la entrega final sigue la política ordinaria del repositorio.

En este workspace eso significa:

- el agente no aprueba su propio PR;
- no se desactiva protección para mergear rápido;
- si GitHub exige review, debe venir de una identidad distinta con permisos reales;
- si un repo es single-maintainer, la protección debe diseñarse explícitamente para CI + 4R + evidencia, no para approvals ficticios.

Ver: [Política de aprobación de PRs](politica-aprobacion-pr.md).

## Regla de síntesis

`agent-orchestrator` no debe copiar un harness existente. Debe ser:

- local-first;
- seguro por defecto;
- observable;
- barato en modelos;
- explícito en agente/provider/modelo;
- compatible con el estándar operativo de repos;
- simple antes de sofisticado.
