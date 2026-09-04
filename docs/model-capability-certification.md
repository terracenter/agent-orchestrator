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
- Los archivos de estado sensibles (state DB SQLite) se crean con permisos `0600`; nunca commitear secretos en docs ni configs.
- Los receipts se persisten con hash `sha256` (`receipt_hash` en SQLite) para integridad y trazabilidad.

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

## Pipeline de certificación: fuentes, snapshots y persistencia

El catálogo versionado del repo (`orq-agent/config/models-catalog.json`) es **bootstrap/export**: declara candidatos y semilla reproducible, pero no es la fuente de verdad operativa. La fuente viva es el state DB SQLite (`ORQ_STATE_DB`), donde se persisten agentes, modelos (flags `gated`/`active`), certifications, receipts (hash `sha256` en `receipt_hash`), `route_scores` y `circuit_breakers`. El discovery runtime (`orq-agent models --agent <agent> --format json`) confirma disponibilidad real sin leer secretos. Cuando bootstrap y runtime divergen, el modelo queda como `candidate`, `deprecated_or_quarantine` o `needs_review` — **evidencia histórica para revisión, nunca default de ruteo** — hasta pasar smoke/certify y quedar certificado en SQLite.

**Variables de entorno (`orq-agent`):**

| Variable | Rol | Default |
|---|---|---|
| `ORQ_STATE_DB` | State DB SQLite vivo (agentes, modelos, certifications, receipts, circuit breakers). Permisos `0600` automáticos. | `~/.local/state/orq-agent/orq-state.sqlite` |
| `ORQ_ROUTING_CONFIG` | Config de ruteo: matriz `task_kind` → `default_agent`/`default_model`, `cheap_sufficient`, `escalate_to`, `avoid`. | `orq-agent/config/routing-matrix.json` |
| `ORQ_MODELS_CATALOG` | Catálogo de candidatos (bootstrap/export): modelos conocidos por agente, con `source`, `confidence` y `notes`. | `orq-agent/config/models-catalog.json` |
| `ORQ_POLICY_CONFIG` | Política: patrones de aprobación requerida y estados de adapter bloqueados/gated. | `orq-agent/config/policy.json` |
| `ORQ_ADAPTERS_REGISTRY` | Registro de adaptadores: binario, estado (`available`, `missing`, `deprecated_or_quarantine`, `gated`) y `argv`. | `orq-agent/config/adapters-registry.json` |

El state DB se crea con permisos `0600` y los receipts se persisten hasheados (`sha256`); ni configs ni docs llevan secretos.

Cada fuente externa usada para añadir o modificar un modelo debe conservar snapshot auditable: URI/origen, fecha, `sha256`, versión de formato y extracto sanitizado. Los extractos no guardan secretos, prompts completos ni stdout/stderr crudo; solo estado, exit code, duración, marcadores operativos, errores acotados y rutas de receipts.

Antes de entrar en routing automático, todo par `agent/model` pasa por *proposal gate*:

1. `orq-agent smoke` genera receipt mínimo.
2. `orq-agent certify --task-kind <kind> --receipt <receipt.json>` vincula evidencia con un tipo de tarea.
3. Si el agente/modelo coincide con patrones gated (`sonnet`, `opus`, `claude-code` u otros definidos en política), queda bloqueado hasta aprobación humana explícita.
4. Solo veredictos aprobados alimentan routing certificado.

El circuit-breaker/backoff forma parte de la certificación. Un timeout o fallo repetido abre circuito para ese par agente/modelo; mientras esté abierto, `orq route` debe omitirlo aunque aparezca en el catálogo bootstrap/export. La reapertura requiere smoke/certify nuevo y receipt exitoso. Ejemplos actuales de cuarentena: `qwen-code/qwen3.8-max` para deep reasoning, `qwen-code/deepseek-v4-flash-0731` por timeout, y `openclaw/default` por adapter roto.

La persistencia es obligatoria y múltiple:

- **Receipts JSON**: evidencia primaria y transferible.
- **Engram/memoria**: aprendizaje operacional entre sesiones.
- **Vault Obsidian**: decisiones humanas, notas y contexto durable.
- **`vg`/Kuzu**: grafo `agent -> model -> task_kind -> receipt -> snapshot` para consultas relacionales.
- **Roadmap/docs/config**: contrato público y configuración ejecutable.

No basta con que la decisión exista en el chat. Antes de cambiar routing se consulta Engram/memoria, Vault/`vg` y receipts; después de validar se sincroniza a Engram/memoria, Vault, Roadmap/docs/config y `vg`.

## Estado inicial conocido

> Los registros siguientes son **evidencia histórica** (estado observado en su momento), no un catálogo operativo. El estado vivo está en SQLite (`ORQ_STATE_DB`) y en receipts/certifications; un modelo aquí marcado como deprecated, en cuarentena o bloqueado **no debe usarse como default de ruteo** aunque siga apareciendo en la config JSON de bootstrap/export.

- `qwen-code/qwen3.6-flash`: validado como candidato medio para documentación, mecánica simple y `doc_watcher` corto.
- `agy/gemini-3.6-flash-medium`: validado como default fuerte para `simple_review`, `short_text_review` y ejecución real acotada.
- `agy/gemini-3.1-pro-high`: validado como default para arquitectura/deep reasoning mientras Claude esté gated.
- `pi/nvidia/openai/gpt-oss-20b`: validado solo para juez corto y texto breve; no usar en tareas largas.
- `qwen-code/qwen3.8-flash`: ejecuta tareas mecánicas, pero requiere alineación de catálogo/certificación antes de promoverlo.
- `qwen-code/qwen3.8-max`: en cuarentena para deep reasoning por timeout reciente; no rutear automáticamente.
- `qwen-code/deepseek-v4-flash-0731`: en cuarentena por timeout reciente.
- `qwen-code/glm-5.2`: requiere certificación adicional; no promover como estable para readonly.
- `claude-haiku-4-5`: usable solo con aprobación explícita y `--allow-gated`; reemplaza rutas viejas `claude-3-5-haiku-*`.
- `claude-3-5-haiku-20241022` y `claude-3-5-haiku-latest`: deprecated/EOL, no deben rutearse automáticamente.
- `hermes`: presente localmente, pero `deprecated_or_quarantine`.
- `openclaw/default`: bloqueado hasta corregir adapter; receipt reciente falló con `Unknown command: openclaw default`.
- `pi`: presente; NVIDIA se mantiene bajo Pi por ahora.
