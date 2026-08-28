# Operación recurrente del estándar

> [!WARNING]
> La automatización recurrente no autoriza deploys, merges ni cambios destructivos. Solo observa, reporta y prepara evidencia.

## Tareas sugeridas

| Frecuencia | Tarea | Acción segura |
|---|---|---|
| Diaria | Revisar PRs/issues acumulados | Reporte, no merge automático |
| Semanal | Revisar costos y eventos de SGE Observer | Resumen por proyecto/agente/modelo |
| Semanal | Detectar worktrees huérfanos | Reporte y propuesta de limpieza |
| Mensual | Revisar modelos disponibles | Comparar costo, calidad, disponibilidad |
| Mensual | Revisar prompts/plantillas | Ajustar por cambios de modelos |
| Mensual | Revisar dependencias | `govulncheck`, `gosec`, auditorías equivalentes |

## Regla de autonomía por riesgo

| Riesgo | Autonomía permitida |
|---|---|
| Bajo | Ejecutar, validar, documentar y abrir PR. |
| Medio | Preparar PR y pedir revisión. |
| Alto | Pausar y pedir confirmación humana/modelo fuerte. |

## Métricas para A/B testing de agentes

- costo estimado;
- duración;
- tokens in/out;
- tasa de errores;
- cantidad de reintentos;
- tests que pasan/fallan;
- hallazgos de seguridad;
- utilidad percibida;
- rollback requerido o no.

## Revisión mensual mínima

```txt
1. ¿Qué modelos nuevos aparecieron?
2. ¿Qué modelos subieron/bajaron calidad?
3. ¿Qué prompts dejaron de funcionar bien?
4. ¿Qué agente aportó valor medible?
5. ¿Qué agente debería eliminarse o simplificarse?
6. ¿Qué repos incumplen el estándar?
7. ¿Hay issues/PRs acumulados por causa raíz común?
```
