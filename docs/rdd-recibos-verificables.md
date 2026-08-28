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
