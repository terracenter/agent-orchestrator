# RDD — Receipt Driven Development

> [!IMPORTANT]
> En este workspace RDD significa desarrollo guiado por recibos verificables. No basta con que un agente diga “listo”: debe dejar evidencia comprobable.

## Principio

Un recibo es un registro pequeño y verificable de una unidad de trabajo.

Debe responder:

| Pregunta | Evidencia aceptada |
|---|---|
| ¿Qué se pidió? | Issue, tarea, prompt/handoff resumido |
| ¿Qué se cambió? | `git diff`, lista de archivos, commits |
| ¿Quién lo hizo? | agente, provider, modelo exacto, humano responsable |
| ¿Cómo se validó? | comandos ejecutados y salida relevante |
| ¿Qué riesgo queda? | revisión 4R, findings, deuda explícita |
| ¿Cómo se revierte? | commit/PR revertible, rollback documentado |
| ¿Dónde queda trazado? | PR, issue, SGE Observer, ledger local |

## Reglas

- El recibo no puede depender solo de narración del LLM.
- La evidencia debe ser derivable del repo, CI, logs, comandos o Observer.
- Si un comando no se ejecutó, no se declara como validado.
- Si una validación falla, el recibo debe conservar el fallo y la decisión tomada.
- Cambios de seguridad, deploy, secretos o datos requieren confirmación humana aunque haya recibo.
- **Telemetría y Veracidad de Tokens (Claude CLI)**: Si una invocación proviene de `claude-code` (Claude CLI en host), no se inventan tokens de entrada/salida no expuestos por la CLI. Se documenta como `unknown` (o `0` tokens reportados con nota de consumo no medido), midiendo en su lugar la **duración real en milisegundos (`duration_ms`)** y el **status** de salida.

## Formato mínimo

```json
{
  "task": "describir tarea",
  "agent": "orq",
  "provider": "pi|agy|nvidia-api|manual",
  "model": "modelo exacto",
  "files_changed": ["ruta"],
  "commands": [
    {"cmd": "go test ./...", "result": "passed"}
  ],
  "risk": "bajo|medio|alto",
  "security_notes": ["sin secretos", "auth no cambia"],
  "rollback": "revert PR #N",
  "evidence": ["commit abc123", "PR #N", "CI go-test OK"]
}
```

## Integración con `orq`

Objetivo futuro:

```bash
orq receipt create --task "..." --pr 23 --risk bajo
orq receipt verify --path receipt.json
```

El recibo debe poder enviarse a SGE Observer como evento operacional sin exponer secretos.

## Estados de Delegación y `DelegateReceipt` (#79)

`orq-agent delegate` genera y valida recibos de delegación estructurados (`DelegateReceipt`):

| Estado (`status`) | Descripción | Criterio de Transición |
|---|---|---|
| `planned` | Tarea analizada y prompt generado sin ejecución ni template de comando autónomo. | `--execute` omitido / no template |
| `command_generated` | Comando autónomo estructurado generado para el agente destino (ej. `agy`, `hermes`, `openclaw`). | `--execute` omitido + template disponible |
| `executed` | Ejecución finalizada con código de salida non-zero o timeout, pero con evidencia comprobable generada (ej. commit nuevo). | `--execute` activo + nuevo commit |
| `validated` | Ejecución exitosa (exit 0) certificada mediante evidencia verificable en git (commit nuevo, rama nueva o ref extraída). | `--execute` activo + exit 0 + evidencia |
| `failed` | Ejecución fallida por falta de evidencia comprobable (`no_executed`), detección de respuesta que es solo plan (`plan_solo` sin cambios), o timeout sin cambios (`timeout_sin_evidencia`). | Fallo, plan-only o sin evidencia |

### Veredicto y Evidencia

- **Veredicto (`verdict`)**:
  - `util`: Aportó cambios o evidencia verificable al repositorio.
  - `non_util`: No generó cambios útiles o falló sin evidencia.
  - `indeterminado`: Estado planificado o comando generado sin ejecución.
- **Evidencia (`evidence`)**:
  - Hash SHA-1/SHA-256 del commit creado en HEAD (`post_head`).
  - Identificador de rama nueva creada (`branch:<nombre>`).
  - Enlace o referencia a PR detectado.
  - `"none"` cuando no se generó evidencia verificable.


