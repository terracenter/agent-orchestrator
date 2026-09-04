# Capacidades de Agentes y Modelos (orq)

Este documento registra los resultados de la inspección de capacidades de agentes y modelos detectados en el sistema de desarrollo local, basándose en la salida de las herramientas del orquestador (`orq`).

## 0. Principio operativo: agentes y modelos son dinámicos

Los agentes, sus binarios y los modelos disponibles **no forman un catálogo estático**: cambian con la instalación local, la disponibilidad del proveedor, la deprecación/EOL y la evidencia de ejecución. Todo listado de agentes/modelos en este repo es **evidencia de inspección o bootstrap reproducible**, nunca una fuente de verdad congelada. La doctrina operativa:

1. **JSON = bootstrap/export (no es la fuente de verdad).** Los archivos `orq-agent/config/*.json` (`models-catalog.json`, `routing-matrix.json`, `policy.json`, `adapters-registry.json`) son semilla versionada y reproducible: declaran candidatos, matriz de ruteo, política y adaptadores conocidos para onboarding y auditoría.
2. **SQLite = fuente operativa viva.** El state DB de `orq-agent` (`ORQ_STATE_DB`) persiste el estado real: agentes, modelos (flags `gated`/`active`), certifications, receipts (hash `sha256` en `receipt_hash`), `route_scores` y `circuit_breakers`. Default: `~/.local/state/orq-agent/orq-state.sqlite`. El CLI `orq` (Go) mantiene su propia telemetría JSONL en `~/.local/state/orq/` (`ledger.jsonl`, `tasks.jsonl`); no confundir con el state DB de `orq-agent`.
3. **Detección y descubrimiento = presencia de binario + candidatos de bootstrap.** `orq-agent detect` confirma solo la presencia del binario en el `PATH` (resolución por `which` o `ORQ_AGENT_BIN_*`), sin leer secretos (`secrets_read=false`); `models --agent <agent>` y `discover` reportan los candidatos del catálogo bootstrap (`discovery: config_catalog`) y `discover` los persiste en SQLite. La disponibilidad real del modelo queda confirmada únicamente al ejecutarlo (`exec`/`smoke`/`certify`) y registrar receipt/certification.
4. **Cuarentena = evidencia histórica, nunca default.** Un par agente/modelo con estado `deprecated_or_quarantine`, o con circuito abierto por timeout/fallo, documenta un hallazgo (deprecación/EOL, timeout, adapter roto) para revisión humana y no participa del ruteo automático: `policy.json` define `blocked_adapter_statuses: ["deprecated_or_quarantine"]` y un circuito abierto hace que `route` omita el par aunque siga presente en el JSON de bootstrap.
5. **Seguridad.** Ni los docs ni los JSON de config contienen secretos (los runners nunca se consultan con credenciales); el state DB SQLite se crea con permisos `0600` automáticos; los receipts se persisten hasheados (`sha256`) y los snapshots externos deben guardar extracto sanitizado con su `sha256`.

### Variables de entorno (`orq-agent`)

| Variable | Rol | Default |
|---|---|---|
| `ORQ_STATE_DB` | Ruta del state DB SQLite vivo (agentes, modelos, certifications, receipts, circuit breakers). Permisos `0600` automáticos. | `~/.local/state/orq-agent/orq-state.sqlite` |
| `ORQ_ROUTING_CONFIG` | Config de ruteo: matriz `task_kind` → `default_agent`/`default_model`, `cheap_sufficient`, `escalate_to`, `avoid`. | Ruta absoluta fijada en compilación: `<CARGO_MANIFEST_DIR>/config/routing-matrix.json` — fuera del árbol de compilación, fijar `ORQ_ROUTING_CONFIG`. |
| `ORQ_MODELS_CATALOG` | Catálogo de candidatos (bootstrap/export): modelos conocidos por agente, con `source`, `confidence` y `notes`. | Ruta absoluta fijada en compilación: `<CARGO_MANIFEST_DIR>/config/models-catalog.json` — fuera del árbol de compilación, fijar `ORQ_MODELS_CATALOG`. |
| `ORQ_POLICY_CONFIG` | Política: patrones de modelos con aprobación requerida (`sonnet`, `opus`) y estados de adapter bloqueados/gated. | Ruta absoluta fijada en compilación: `<CARGO_MANIFEST_DIR>/config/policy.json` — fuera del árbol de compilación, fijar `ORQ_POLICY_CONFIG`. |
| `ORQ_ADAPTERS_REGISTRY` | Registro de adaptadores: binario, estado (`available`, `missing`, `deprecated_or_quarantine`, `gated`) y `argv`. | Ruta absoluta fijada en compilación: `<CARGO_MANIFEST_DIR>/config/adapters-registry.json` — fuera del árbol de compilación, fijar `ORQ_ADAPTERS_REGISTRY`. |

Las tablas de las secciones siguientes son **instantáneas de inspección fechadas**, no catálogo operativo: para conocer el estado vivo se consulta el state DB SQLite (el discovery solo re-reporta presencia de binario y candidatos del catálogo bootstrap, §0.3).

## 1. Agentes Configurados y Modelos

A continuación se detallan los agentes registrados y sus configuraciones de modelos obtenidas mediante `rtk orq agents --format json` (instantánea de inspección de la sección 0; el estado vivo está en SQLite y en el runtime):

| Agente | Proveedor | Modelo | Nivel de Costo | Propósito de Uso | Verificado |
|--------|-----------|--------|----------------|------------------|------------|
| **pi** | openai | `gpt-5.5` | 2 | Orquestación principal y síntesis de decisiones | Sí |
| **pi** | openai | `cheap-or-fast` | 1 | Alias de menor costo suficiente para tareas mecánicas/documentales | Sí |
| **nvidia-api** | nvidia | `openai/gpt-oss-20b` | 0 | Smoke tests, clasificación barata y tareas mecánicas con API NVIDIA | Sí |
| **nvidia-api** | nvidia | `openai/gpt-oss-120b` | 0 | Validación barata y razonamiento hospedado con API NVIDIA | Sí |
| **pi** | nvidia | `free-or-low-cost` | 0 | Tareas mecánicas, resumen y clasificación cuando el provider esté disponible | No |
| **claude-code** | anthropic | `haiku` | 1 | Tareas mecánicas con instrucciones cerradas cuando se quiere usar Claude barato | Sí |
| **agy** | google | `gemini-3.5-flash-low` | 1 | Tareas mecánicas, clasificación y validaciones baratas | Sí |
| **agy** | google | `gemini-3.5-flash-medium` | 1 | Tareas rápidas, resúmenes y documentación | No |
| **agy** | google | `gemini-3.7-flash-high` | 1 | Implementación de código y análisis técnico medio | No |
| **agy** | google | `gemini-3.1-pro-low` | 2 | Análisis técnico fuerte y refutación antes de escalar a Claude | No |
| **agy** | open-model | `gpt-oss-120b-medium` | 0 | Validación barata de prompts, resúmenes y tareas mecánicas cuando AGY lo expone | Sí |
| **agy** | nvidia | `free-or-low-cost` | 0 | Tareas mecánicas y validaciones baratas si AGY lo expone | No |
| **qwen-code** | bailian | `qwen3.8-max` | 1 | Código, búsqueda en repos, shell/git/docker y tareas técnicas medianas bajo plan Standard reportado por runtime | Sí |
| **qwen-code** | bailian | `qwen3.5` | 1 | Modelo disponible reportado por Qwen Code; validar empíricamente antes de asignación crítica | No |
| **qwen-code** | bailian | `qwen3.6` | 1 | Modelo disponible reportado por Qwen Code; validar empíricamente antes de asignación crítica | No |
| **qwen-code** | bailian | `qwen3.7-plus` | 1 | Modelo disponible reportado por Qwen Code; validar empíricamente antes de asignación crítica | No |
| **claude-code** | anthropic | `sonnet` | 3 | Código, revisión crítica, seguridad, bloqueos y refutación de decisiones | Sí |
| **claude-code** | anthropic | `opus` | 4 | Arquitectura compleja, auditoría crítica o decisión mayor | Sí |
| **claude-code** | anthropic | `fable` | 2 | Modelo Claude pendiente de clasificar; usar solo si la CLI lo expone y hay confirmación operativa | No |

## 2. Detección Local de Agentes e Instalación

Resultados de la auto-detección local mediante `rtk orq agents detect --format json`:

| Agente | Instalado | Ruta del Binario | Ruta de Configuración | Rol / Nota |
|--------|-----------|------------------|-----------------------|------------|
| **openclaw** | Sí | `/home/freddy/.local/share/pi-node/node-v22.23.2-linux-x64/bin/openclaw` | `/home/freddy/.openclaw` | Runner económico para tareas mecánicas (Haiku). Inspección segura. |
| **agy** | Sí | `/home/freddy/.local/bin/agy` | `/home/freddy/.gemini` | Runner rápido para código y análisis técnico medio (Antigravity CLI). |
| **hermes** | Sí | `/home/freddy/.local/bin/hermes` | `/home/freddy/.hermes` | Runner para tareas de integración y exploración. |
| **claude-code**| Sí | `/home/freddy/.local/bin/claude` | `/home/freddy/.claude` | Revisión crítica, seguridad y refutación (review_only). |
| **pi** | Sí | `/home/freddy/.local/share/pi-node/node-v22.23.2-linux-x64/bin/pi` | PENDIENTE | Supervisor principal; detener y delegar si hay tensión de presupuesto. |
| **qwen-code** | Sí | `/home/freddy/.local/bin/qwen` o `PATH` | `/home/freddy/.qwen` | Runner multi-modelo para código y tareas técnicas; detección segura sin leer settings ni secretos. |
| **codex** | No | PENDIENTE | PENDIENTE | Runner y asistente secundario. |
| **nvidia-api** | No | PENDIENTE | PENDIENTE | Smoke tests, clasificación barata y tareas mecánicas. |

## 3. Estado de Herramientas de Soporte (`orq doctor`)

Salida consolidada del comando de diagnóstico:

*   **rtk**: `ok` (v0.46.0)
*   **git**: `ok` (v2.47.3)
*   **gh**: `ok` (v2.46.0)
*   **orq**: `ok`
*   **vg (vault-graph)**: `ok` (detectado en ruta del workspace, sugerido configurar `ORQ_VG_PATH` o agregar al PATH)
*   **openclaw**: `ok`
*   **agy**: `ok`
*   **hermes**: `ok`
*   **claude**: `ok`

*Todos los agentes clave cuentan con CLI funcional que responde correctamente a flags de ayuda (`--help`).*

## 3.1 Qwen Code — piloto de autodiscovery seguro (#81)

`orq agents detect` reconoce `qwen-code` mediante presencia del binario `qwen` y/o directorio de configuración `~/.qwen`, pero no lee archivos de configuración potencialmente sensibles. El primer perfil verificado se registra como `qwen-code/bailian/qwen3.8-max` porque fue reportado por runtime en una ejecución empírica del usuario. Los modelos `qwen3.5`, `qwen3.6` y `qwen3.7-plus` quedan como disponibles/no verificados hasta que exista prueba empírica versionada.

Regla de seguridad: no imprimir ni auditar `.secrets`, tokens, endpoints privados o contenido de `~/.qwen/settings.json`; solo se permite metadata segura como existencia de binario/directorio y modelos declarados por ejecución controlada.

## 3.2 Snapshots de capacidades/modelos (#81)

`orq models snapshot --format json` emite un snapshot fechado por combinación `agente/proveedor/modelo`. Cada registro separa:

- identidad: agente, proveedor y modelo;
- estado de verificación;
- costo operativo;
- entradas, salidas, herramientas y modos declarados;
- evidencia/fuente (`registry`, `empirical`, `status`);
- `captured_at` para trazabilidad temporal;
- nota de seguridad explícita.

La evidencia empírica de Qwen Code se limita al runtime reportado por el usuario en #81. Modelos no verificados reciben fuente `status=pendiente` hasta validación por runtime, documentación oficial o tareas reales. Este comando no lee configuración privada, tokens ni `.secrets`; prepara el contrato de datos para aprendizaje/scoring posterior y para exportación a Observer en un corte separado.

## 3.3 Scoring personalizado inicial (#81)

`orq score --format json` resume el ledger local por `agente/modelo` para aprender del historial real del usuario. El primer scoring es deliberadamente simple y verificable:

- `ok`, `passed`, `success`, `completed` y `executed` cuentan como éxito;
- estados desconocidos cuentan como fallo;
- `not_executed` se contabiliza separado y penaliza medio punto, porque una delegación que solo generó prompt/receipt no debe mejorar la reputación del agente ni ocultarse como éxito;
- el resultado se ordena de mayor a menor score.

Este corte no decide routing automáticamente todavía: expone una señal segura y auditable para un PR posterior que combine scoring con repo, tipo de tarea, riesgo y snapshots de capacidad.

---

## 4. Backlog PRs propuestos

A continuación se lista el backlog de PRs sugeridos para el orquestador:

*   **PR1: Registry de Modelos**: Estandarizar la definición y el descubrimiento de modelos de proveedores en una estructura de registro centralizada con validaciones estáticas.
*   **PR2: Delegate OpenClaw/MiniPC**: Implementar mecanismos de delegación dinámica hacia entornos aislados (e.g., OpenClaw corriendo en MiniPC) para tareas mecánicas/costosas de forma segura.
*   **PR3: Router de Costo/Capacidad**: Crear un enrutador inteligente que seleccione automáticamente el agente y modelo con el costo óptimo de acuerdo a la complejidad detectada en el prompt.
*   **PR4: Suite de Tests**: Desarrollar pruebas de integración y smoke tests automáticos para verificar el correcto parsing y la ejecución de las herramientas CLI de los agentes sin comprometer secretos.

---

## 5. Soporte minipc-local (Modelos Locales)

*   **Capacidad Verificada [V]**: Host `minipc` (CachyOS, AMD Zen4, 30.2 GiB RAM) con IP de Tailscale `100.76.175.78` documentado en el vault ([estaciones-personales.md](file:///home/freddy/Workspace/Obsidian/03.Servidores/Humanbyte/estaciones-personales.md)).
*   **Comandos Verificados [V]**: Comandos de ejecución mediante el agente `hermes` integrando el proveedor `ollama-minipc` documentados en el vault ([config-limpio.md](file:///home/freddy/Workspace/Obsidian/04.Documentacion/Hermes-Agent/config-limpio.md)):
    ```bash
    hermes --model qwen2.5-coder:7b-instruct-q4_K_M --provider ollama-minipc
    ```
*   **Modelos Locales Detectados (Documentados) [V]**:
    *   `qwen2.5-coder:7b-instruct-q4_K_M` (Activo/Producción en el minipc)
    *   `qwen2.5:7b` (Disponible si se carga en Ollama)
    *   `deepseek-r1:7b` (Disponible si se carga en Ollama)
    *   `llama3.2:latest` (Disponible si se carga en Ollama)
*   **Limitaciones**:
    *   No hay binarios de inferencia locales (`ollama`, `llama-server`, `llama-cli`, `lmstudio`, `vllm`, `llamacpp`) instalados en el host del orquestador.
    *   La comunicación con el host `minipc` (`100.76.175.78`) depende enteramente de que la VPN de Tailscale esté activa y ruteable.
    *   Se requiere autorización/evidencia explícita previa para conectarse por SSH al host remoto en flujos automáticos.
*   **Usos Recomendados de Bajo Riesgo**:
    *   Fallback de Nivel 0 (costo cero de API) para tareas de desarrollo mecánico e implementación de código de nivel medio utilizando el modelo `qwen2.5-coder:7b-instruct-q4_K_M`.

---

## 6. Telemetría de Invocación, Timeout/Heartbeat y Consumo de Tokens (Claude CLI)

*   **Supervisión por Invocación**:
    *   Todo agente invocado bajo la supervisión de `orq` opera con control de **timeout** (`context.WithTimeout`) y pulsos periódicos de **heartbeat** (vía `internal/heartbeat/invocation.go`).
    *   El ciclo de vida completo de cada invocación se persiste en el ledger (`~/.local/state/orq/ledger.jsonl`) registrando timestamps de inicio (`started_at`), fin (`finished_at`), duración efectiva (`duration_ms`), status resultante (`ok`, `timeout`, `failed`, `fallback_ok`, `fallback_failed`) y modelo de fallback utilizado.
*   **Tratamiento de Tokens de Claude CLI (`claude-code`)**:
    *   La CLI de Claude Code (`claude`) se ejecuta como subproceso interactivo o de sesión en el host local y no expone de forma estándar ni estructurada el conteo exacto de tokens (*tokens in* / *tokens out*) consumidos por comando en el shell.
    *   **Regla RDD de Veracidad**: Queda estrictamente prohibido simular, inventar o extrapolar cifras no medidas de tokens para `claude-code`.
    *   Cuando no se dispone de telemetría real reportada por la API de Anthropic, el consumo de tokens en `orq record`, en el ledger y en los recibos RDD DEBE registrarse como `unknown` (representado en telemetría de contadores como `tokens_in = 0, tokens_out = 0` y documentado en notas como consumo no medido/unknown).
    *   La métrica primaria auditable y medible en el host para Claude CLI es su **duración temporal (`duration_ms`)**, su **status de salida**, y el control de **timeout/heartbeat** gestionado por el orquestador.


