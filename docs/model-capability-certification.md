# Certificación progresiva de capacidades de agentes/modelos

## Problema

Orq no debe decidir rutas solo por intuición, listas estáticas o memoria conversacional. Las ideas y descubrimientos de uso pueden repetirse o contradecirse si no quedan registrados como evidencia verificable.

Ejemplos recientes:

- `qwen-code/qwen3.8-max` funciona como CLI real, pero el registry de Orq lo rechaza como `unknown agent/model pair`.
- `claude-3-5-haiku-20241022` aparece como ruta barata en Orq, pero el proveedor lo reporta deprecated/EOL; el uso real requiere `claude-haiku-4-5`.
- `orq-watch-clients` solo observaba `COMMAND` y omitía runners envueltos por `rtk`, `node` o `python` hasta que se amplió a `ARGS`.
- Hermes existe como runner local, pero está en cuarentena/deprecated y no debe ser priorizado.

## Decisión operativa

Toda idea relevante sobre agentes, modelos, routing, política o ejecución debe convertirse en al menos una salida trazable:

1. ADR o decisión técnica si cambia arquitectura/política.
2. RFC/documento si todavía es diseño o hipótesis.
3. Issue local/remoto si implica trabajo pendiente concreto.
4. Receipt/evidencia Orq si se validó mediante ejecución.
5. Nota de vault + `vg sync` cuando afecte conocimiento operativo del vault.

No basta con que la idea exista en el chat.

## Objetivo

Construir un ciclo de aprendizaje controlado para que Orq pueda responder:

> Para este tipo de tarea, este agente/modelo está certificado como suficiente, barato y seguro, según evidencia histórica.

## Modelo de certificación

Cada agente/modelo debe tener una ficha verificable con:

- agente (`pi`, `qwen-code`, `openclaw`, `agy`, `claude-code`, etc.);
- modelo o alias exacto;
- comando real usado;
- estado: `candidate`, `available`, `certified`, `gated`, `deprecated_or_quarantine`, `failed`;
- capacidades observadas: lectura, edición, ejecución headless, safe-mode, soporte de `--model`, soporte de prompt por stdin/archivo;
- límites conocidos: timeout, tamaño de prompt, rate limits, costo relativo;
- tareas donde rindió bien;
- tareas donde falló;
- receipts asociados;
- fecha de última validación.

## Flujo propuesto

### 1. Descubrimiento

Comando futuro:

```bash
orq-agent models --agent qwen-code --format json
```

Debe consultar al runner sin leer secretos y devolver modelos/alias disponibles si el CLI lo soporta.

### 2. Smoke test

Comando futuro:

```bash
orq-agent smoke --agent qwen-code --model qwen3.8-max --format json
```

Debe ejecutar una tarea mínima, acotada y barata, y producir receipt JSON.

### 3. Certificación

Comando futuro:

```bash
orq-agent certify --agent qwen-code --model qwen3.8-max --task-kind rust-review --receipt <receipt.json>
```

Debe registrar que ese agente/modelo pasó un caso concreto.

### 4. Routing por evidencia

`orq route` debe preferir agentes/modelos certificados para el tipo de tarea, antes que rutas caras o gated.

Ejemplo:

- `rust-review` → Qwen certificado + validación local.
- `policy/security-review` → Claude Haiku si está disponible y no deprecated.
- `critical-architecture` → requiere aprobación humana para Sonnet/Opus.
- `legacy/quarantine` → Hermes no se usa salvo autorización explícita.

## Rol de `orq-watch-clients`

`orq-watch-clients` no certifica capacidades. Solo observa procesos vivos y ayuda a verificar si los runners están ejecutándose realmente.

Debe alimentar diagnósticos, no decisiones finales.

La certificación debe basarse en:

- receipts JSON;
- exit code;
- stdout/stderr acotados y sanitizados;
- tests/validaciones pasadas;
- evidencia de archivos modificados cuando aplique;
- historial de resultados.

## Guardrails

- No leer secretos para descubrir modelos.
- No asumir que un modelo existe hasta validarlo.
- No abrir issue/PR remoto sin confirmación explícita.
- Sonnet/Opus siguen gated por aprobación humana.
- NVIDIA queda bajo Pi mientras no exista runner local separado.
- Un modelo deprecated no debe seguir en rutas automáticas aunque esté en config histórica.
- La evidencia debe ser reproducible o al menos auditable.

## Criterios de aceptación del MVP

- [x] `orq-agent models --agent <agent> --format json` existe para al menos `qwen-code` y `pi`, con fallback seguro si el runner no expone catálogo.
- [x] `orq-agent smoke` genera receipt JSON, no lee secretos y valida marcador `ORQ_SMOKE_OK`.
- [x] `orq-agent exec` emite receipts JSON tempranos para `invalid_request` y `spawn_failed`.
- [x] `orq-agent exec` usa lectura concurrente con ring buffer acotado y preserva salida parcial en timeout.
- [x] `orq-agent exec` mata el process group Unix en timeout y acota waits/colección de tails.
- [x] `orq run --execute` consume `orq-agent exec` como backend real progresivo y marca `executed=true` solo con receipt `status=succeeded`.
- El registry acepta `qwen-code/qwen3.8-max` solo después de evidencia positiva.
- Orq registra cuándo un modelo falla por deprecated/EOL.
- `orq route` puede explicar por qué eligió un agente/modelo usando evidencia previa.
- Toda nueva idea de routing/policy queda registrada como doc, issue, ADR o receipt.

## Estado inicial conocido

- `qwen-code/qwen3.8-max`: usable por ejecución directa; smoke real exitoso en `/tmp/orq-agent-smoke-qwen-real-v4.json`, pendiente integrar al registry Orq.
- `claude-haiku-4-5`: usable para revisión barata; reemplaza rutas viejas `claude-3-5-haiku-*`.
- `claude-3-5-haiku-20241022` y `claude-3-5-haiku-latest`: deprecated/EOL, no deben rutearse automáticamente.
- `hermes`: presente localmente, pero `deprecated_or_quarantine`.
- `openclaw`: presente como gateway Node; pendiente validar costo/cuenta Claude Pro.
- `pi`: presente; NVIDIA se mantiene bajo Pi por ahora.
