# RDD — Agent/Model Discovery & Learning (Autodetección Multi-Modelo y Aprendizaje por Usuario)

> [!IMPORTANT]
> **Estado:** Draft / En Revisión  
> **Fecha:** 2026-09-04  
> **Autor:** Freddy Taborda (`terracenter@gmail.com`)  
> **Issue Asociado:** [#81](https://github.com/terracenter/agent-orchestrator/issues/81)  
> **Repositorio:** `terracenter/agent-orchestrator`  
> **Documentos Relacionados:** [docs/rdd-recibos-verificables.md](file:///home/freddy/Workspace/Desarrollo/agent-orchestrator/docs/rdd-recibos-verificables.md) (#79), [docs/model-capability-certification.md](file:///home/freddy/Workspace/Desarrollo/agent-orchestrator/docs/model-capability-certification.md), [docs/agent-model-capabilities.md](file:///home/freddy/Workspace/Desarrollo/agent-orchestrator/docs/agent-model-capabilities.md), issue #148 (MarketFeed & Adaptive Fallback).

---

## 1. Problema y Contexto

Históricamente, los orquestadores de agentes han operado bajo reglas de enrutamiento estáticas y suposiciones rígidas como asumir la equivalencia binaria `agente == modelo` (ej. "Claude Code solo es Claude 3.7", "Qwen Code solo es Qwen 2.5"). Esto genera fragilidad operacional, bloqueos cuando un proveedor cae o agota cuotas, e imposibilita aprovechar agentes que soportan múltiples backends o modelos de vanguardia.

Tal como se definió en el issue [#81](https://github.com/terracenter/agent-orchestrator/issues/81):
> **Ningún agente debe ser tratado como ejecutor universal.** Cualquier agente puede aportar código, documentación, revisión, debate, validación o arquitectura si sus capacidades reales lo permiten.
> La selección debe realizarse mediante **evidencia verificable**: agente, proveedor/backend, modelo, herramientas disponibles, permisos reales, modo de ejecución, plan/costos/cuotas, riesgo, contexto del repositorio/tipo de tarea y el **historial empírico de calidad para este usuario**.

El presente RDD (*Receipt Driven Development*) define la arquitectura de autodescubrimiento dinámico (*autodiscovery*), el modelo de datos multi-proveedor desacoplado, la taxonomía de capacidades, el protocolo de seguridad ante archivos sensibles, el motor de scoring empírico y el contrato de sincronización con **SGE Observer**.

---

## 2. Checklist General de Criterios de Aceptación (Issue #81)

- [x] **Criterio 1:** Existe diseño y especificación para el discovery de agentes y modelos en CLI (`discover`, `refresh`, `doctor`, `snapshot`).
- [x] **Criterio 2:** El modelo de datos soporta agentes desacoplados con múltiples proveedores y modelos (jerarquía `usuario → agente → proveedor → modelo → capacidades → métricas`).
- [x] **Criterio 3:** Se define el protocolo estricto para separar y etiquetar fuentes de verdad: runtime local, documentación oficial y evidencia empírica.
- [x] **Criterio 4:** Se formaliza **Qwen Code** como caso piloto con datos reales de configuración, plan Bailian y modelos declarados sin asumir no verificados.
- [x] **Criterio 5:** Se documenta la taxonomía integral de capacidades multimodales, herramientas del sistema y modos operativos.
- [x] **Criterio 6:** Se define el motor de scoring multidimensional personalizado por usuario, repositorio, stack y nivel de riesgo.
- [x] **Criterio 7:** Se establece la política estricta de seguridad: sanitización de secretos, metadata segura y control ante hallazgos de guardia de raíz.
- [x] **Criterio 8:** Se diseña la interfaz y contrato JSON para enviar snapshots fechados de capacidades y modelos a **SGE Observer**.
- [x] **Criterio 9:** Se documenta la penalización matemática en scoring ante fallos de delegación (`status=not_executed`, `plan_solo`, `timeout_sin_evidencia`).

---

## 3. Especificación por Criterio de Aceptación

### 3.1. Criterio 1: Autodiscovery de Agentes y Modelos

- [x] **Checklist Criterio 1:** Especificación CLI (`orq agents discover`, `refresh`, `doctor`, `models snapshot`), schema de metadatos no secretos, integración extensiva con `orq-agent models refresh` y MarketFeed (#148).

`orq` incorpora comandos especializados para inspeccionar el entorno del sistema operativo, detectar herramientas CLI instaladas, auditar configuraciones locales y generar instantáneas (*snapshots*) inmutables de capacidades.

```text
┌──────────────────────────────────────────────────────────────────────────────────┐
│ CLI de Autodiscovery de orq                                                      │
├────────────────────────────────┬─────────────────────────────────────────────────┤
│ orq agents discover            │ Escanea binarios conocidos en PATH, configs      │
│                                │ locales y genera snapshot general de agentes.   │
├────────────────────────────────┼─────────────────────────────────────────────────┤
│ orq agents refresh <agent>     │ Fuerza la re-inspección puntual de un agente    │
│                                │ (ej. recarga de tokens, permisos o flags).      │
├────────────────────────────────┼─────────────────────────────────────────────────┤
│ orq agents doctor              │ Valida salud de binarios, dependencias de shell,│
│                                │ wrappers (rtk, vg, engram) y sandbox de agentes.│
├────────────────────────────────┼─────────────────────────────────────────────────┤
│ orq models snapshot            │ Genera y exporta el catálogo consolidado de     │
│                                │ modelos/proveedores fechado para consumo local.  │
└────────────────────────────────┴─────────────────────────────────────────────────┘
```

#### Metadatos Mínimos Registrados por Snapshot de Agente:
1. **Identificador y Binario:** Nombre canónico (`id`), comando ejecutable (`command`), ruta absoluta del binario (`binary_path`).
2. **Versión:** Versión semántica o hash de build extraído del runtime (`version`).
3. **Configuración no secreta:** Rutas de configuración activas (`config_paths`), endpoints públicos (`endpoints`), flags por defecto.
4. **Proveedores / Backends:** Lista de backends configurados (ej. `bailian`, `anthropic`, `openrouter`, `ollama`).
5. **Modelos:** Identificadores exactos de modelos soportados y su estado (`active`, `available`, `unverified`).
6. **Modos disponibles:** Modos de operación (`chat`, `agentic`, `read_only`, `edit`, `review`, `debate`, `architecture`).
7. **Herramientas y Permisos:** Tools expuestas por el runtime (`shell`, `filesystem`, `git`, `docker`, etc.) y permisos efectivos auditados.
8. **Plan / Facturación:** Tipo de plan (`standard`, `payg`, `free`), cuota/crédito restante, fecha de expiración y estado de auto-renovación.
9. **Metadatos Temporales y Fuente:** `snapshot_at` (ISO 8601 UTC) y `source` (`runtime_doctor`, `market_feed`, `manual_config`).

> [!NOTE]
> **Relación con #148 (MarketFeed):** `orq-agent models refresh` y el MarketFeed sincronizan el catálogo global del mercado (precios, promociones activas como Claude +50% y estado de proveedores externos). El nuevo `orq agents discover` **extiende** esta infraestructura al auditar el entorno local del host (`sge-panel.humanbyte.net` o estación local), cruzando qué agentes y modelos están realmente presentes y operativos en la máquina.

---

### 3.2. Criterio 2: Modelo de Datos Multi-Proveedor y Desacoplado

- [x] **Checklist Criterio 2:** Jerarquía `usuario → agente → proveedor → modelo → capacidades → métricas → historial`, extensión formal del Schema v2 de Catálogo.

Se erradica la premisa `agente == modelo`. Un agente es un arnés de ejecución (runtime / CLI / protocolo), mientras que un proveedor es el backend de cómputo que sirve uno o más modelos bajo esquemas de precios y políticas específicas.

#### Jerarquía Conceptual:
```text
Usuario (Freddy Taborda)
  └── Agente (ej. qwen-code, claude-code, hermes, agy, openclaw)
        └── Proveedor / Backend (ej. bailian, anthropic-api, openrouter, local-ollama)
              └── Modelo (ej. qwen3.8-max, claude-3-7-sonnet, glm-5)
                    ├── Capacidades (contexto, tool-use, multimodalidad, modos)
                    ├── Métricas por Tarea / Repo (rendimiento, tasa de acierto, latencia)
                    └── Historial Empírico (recibos verificables #79, veredictos)
```

#### Extensión del Schema v2 del Catálogo (`catalog.json` / SQLite):
El schema v2 introducido en el issue #148 (`fetched_at`, `cost_hint`, `promo`, `status`) se extiende estructuralmente:

```json
{
  "$schema": "https://json-schema.terracenter.net/orq/v2.1/agent-model-discovery.json",
  "snapshot_id": "snap-20260904-213500-host01",
  "fetched_at": "2026-09-04T21:35:00Z",
  "user_id": "freddy",
  "agents": [
    {
      "id": "qwen-code",
      "name": "Qwen Code CLI",
      "vendor": "Alibaba Group",
      "binary": {
        "command": "qwen",
        "path": "/home/freddy/.local/bin/qwen",
        "version": "1.4.2"
      },
      "config_metadata": {
        "settings_path": "/home/freddy/.qwen/settings.json",
        "has_credentials": true,
        "auth_type": "token_plan_api_key"
      },
      "providers": [
        {
          "id": "bailian",
          "name": "Alibaba Cloud Bailian Token Plan",
          "endpoint": "token-plan.ap-southeast-1.maas.aliyuncs.com",
          "plan": {
            "name": "Qwen Standard Plan",
            "active": true,
            "period_start": "2026-08-24T07:31:52Z",
            "period_end": "2026-09-24T12:00:00Z",
            "auto_renewal": false
          },
          "models": [
            {
              "id": "qwen3.8-max",
              "status": "active",
              "source_type": "runtime",
              "verified": true,
              "last_verified_at": "2026-09-04T21:30:00Z",
              "cost_hint": {
                "unit": "token_plan_quota",
                "promo": "unlimited_in_plan_window"
              },
              "capabilities": {
                "input_modalities": ["text", "code", "files", "repo"],
                "output_modalities": ["text", "code", "patch", "markdown", "json"],
                "tools": ["shell", "filesystem", "git", "docker", "workspace_skills"],
                "modes": ["chat", "agentic", "edit", "review", "architecture"],
                "context_window_tokens": 131072,
                "max_output_tokens": 8192
              }
            },
            {
              "id": "glm-5",
              "status": "available",
              "source_type": "docs_official",
              "verified": false,
              "last_verified_at": null,
              "capabilities": {
                "input_modalities": ["text", "code"],
                "output_modalities": ["text", "code", "markdown"],
                "tools": ["shell", "filesystem"],
                "modes": ["chat", "agentic"]
              }
            }
          ]
        }
      ]
    }
  ]
}
```

---

### 3.3. Criterio 3: Separación Estricta de Fuentes de Verdad

- [x] **Checklist Criterio 3:** Definición de niveles de confianza (`runtime`, `docs_official`, `empirical_history`), metadatos de trazabilidad y reglas de degradación/fusión.

Para evitar inventarios obsoletos o alucinaciones sobre modelos no probados, `orq` clasifica cada dato en tres niveles de veracidad obligatorios:

```text
┌──────────────────────────────────────────────────────────────────────────────────┐
│ Niveles de Veracidad de Datos en orq                                             │
├────────────────────┬─────────────────────────────┬───────────────────────────────┤
│ Nivel de Fuente    │ Origen y Mecanismo          │ Grado de Confianza            │
├────────────────────┼─────────────────────────────┼───────────────────────────────┤
│ runtime            │ Salida directa de comandos  │ Alta operacional.             │
│                    │ locales (`--version`,       │ Certifica que el binario      │
│                    │ `doctor`, `models list`).   │ existe y responde en el host. │
├────────────────────┼─────────────────────────────┼───────────────────────────────┤
│ docs_official      │ Feeds de API, release notes │ Media-Alta (declarada).       │
│                    │ y documentación de vendor.  │ Requiere verificación antes   │
│                    │                             │ de priorizar en tareas clave. │
├────────────────────┼─────────────────────────────┼───────────────────────────────┤
│ empirical_history  │ Recibos verificables (#79)  │ Máxima para scoring.          │
│                    │ de tareas ejecutadas en el  │ Mide el comportamiento real   │
│                    │ workspace de Freddy.        │ sobre repositorios reales.    │
└────────────────────┴─────────────────────────────┴───────────────────────────────┘
```

#### Reglas de Etiquetado y Fusión:
1. **Atributo Obligatorio:** Todo registro de modelo y capacidad debe incluir `source_type` (`runtime` | `docs_official` | `empirical_history`), `source_ref` (comando, URL o ID de recibo) y `verified: bool`.
2. **Promoción de Estado:** Un modelo marcado inicialmente como `docs_official` (`verified: false`) solo pasa a `verified: true` cuando:
   - Se ejecuta una verificación satisfactoria vía runtime (`orq agents doctor` o llamada de prueba exitosa).
   - O genera un `DelegateReceipt` con `verdict=util` en una tarea real del workspace.
3. **Invariante Mandato #15:** Ninguna capacidad se da por sentada sin prueba ejecutable. En documentación y CLI se utiliza el marcado **[V]** (verificado localmente), **[R]** (reportado por docs/feed) o **[D]** (decisión pendiente).

---

### 3.4. Criterio 4: Qwen Code como Caso Piloto

- [x] **Checklist Criterio 4:** Parametrización real de Qwen Code, plan Bailian activo, separación entre modelo verificado (`qwen3.8-max`) y modelos declarados pendientes de validación.

Se formaliza la integración piloto observada en el workspace según la auditoría del issue #81:

#### Parámetros Reales del Caso Piloto:
- **Agente:** Qwen Code CLI (Alibaba Group).
- **Modelo Activo Verificado:** `qwen3.8-max` (marcado como `verified: true`, `source_type: "runtime"`).
- **Vía de Conexión:** Token Plan Bailian (Alibaba Cloud Model Studio).
- **Endpoint:** `token-plan.ap-southeast-1.maas.aliyuncs.com`.
- **Archivo de Configuración:** `~/.qwen/settings.json` (auditado: solo metadata segura, sin imprimir tokens).
- **Detalle de Suscripción:**
  - Plan: **Qwen Standard Plan**.
  - Inicio de vigencia: `2026-08-24 07:31:52 UTC`.
  - Fin de vigencia: `2026-09-24 12:00:00 UTC`.
  - Auto-renovación: Deshabilitada (`auto_renewal: false`).
- **Modelos Declarados en Coding Plan (Estado `unverified` / `docs_official`):**
  - `qwen3.5`
  - `qwen3.6`
  - `qwen3.7-plus`
  - `glm-5`
  - `kimi-k2.5`
  - `MiniMax-M2.5`
- **Capacidades Declaradas por Qwen Code:**
  - Lectura, escritura y edición de código en múltiples stacks.
  - Ejecución de builds, unit tests y linters.
  - Búsqueda sintáctica y semántica en codebases locales.
  - Ejecución controlada en shell Linux.
  - Operaciones de control de versiones con Git y gestión de contenedores Docker.
  - Consumo de skills compartidas del workspace (`.agents/skills/`).
  - Delegación a subagentes y gestión de planes de trabajo.
  - Memoria persistente entre sesiones operativas.

> [!IMPORTANT]
> **Política con Modelos Declarados:** Los modelos `glm-5`, `kimi-k2.5`, `MiniMax-M2.5`, etc., quedan registrados en el catálogo con `status: "available"`, `source_type: "docs_official"` y `verified: false`. El orquestador **no asumirá que funcionan** para tareas críticas hasta que `orq agents refresh qwen-code` o un primer recibo verificable certifique su interoperabilidad real.

---

### 3.5. Criterio 5: Capacidades Multimodales y Tool-Use

- [x] **Checklist Criterio 5:** Taxonomía completa de inputs, outputs, tools y modos operativos soportados por la arquitectura.

El orquestador modela las capacidades de cada par `(agente, modelo)` bajo cuatro dimensiones ortogonales:

```text
                               CAPACIDADES DEL NÚCLEO
  ┌───────────────────────┬───────────────────────┬────────────────────────┬──────────────────────┐
  │ Inputs                │ Outputs               │ Tools                  │ Modos                │
  ├───────────────────────┼───────────────────────┼────────────────────────┼──────────────────────┤
  │ • text (prompt)       │ • text (análisis)     │ • shell (bash/zsh)     │ • chat               │
  │ • code (snippets)     │ • code (archivos)     │ • filesystem (read/w)  │ • agentic (autónomo) │
  │ • image (diagramas)   │ • patch (git diff)    │ • git (rtk wrapper)    │ • read_only          │
  │ • audio (voz)         │ • markdown (docs)     │ • github_cli (gh)      │ • edit (mutación)    │
  │ • video (frames)      │ • json (estructurado) │ • docker (compose v2)  │ • review (auditoría) │
  │ • files (pdf/txt)     │ • diagrams (.html)    │ • browser (render/web) │ • debate (consensual)│
  │ • repo (árbol/AST)    │                       │ • apis (curl/rest)     │ • architecture (rdd) │
  └───────────────────────┴───────────────────────┴────────────────────────┴──────────────────────┘
```

#### Evaluación de Compatibilidad en Enrutamiento:
Antes de delegar una tarea, `orq` evalúa los requisitos de la tarea contra la matriz de capacidades:
$$\text{Compatible}(\text{Task}, \text{Agent}) = (\text{Task.Inputs} \subseteq \text{Agent.Inputs}) \land (\text{Task.ToolsRequired} \subseteq \text{Agent.Tools}) \land (\text{Task.Mode} \in \text{Agent.Modes})$$

Si un modelo carece de soporte para herramientas críticas (ej. ejecución de shell para correr tests de Go), queda descartado automáticamente para tareas de mutación de código con calidad asegurada.

---

### 3.6. Criterio 6: Scoring Personalizado por Usuario

- [x] **Checklist Criterio 6:** Dimensiones mínimas, métricas empíricas (calidad técnica, obediencia, compilabilidad, corrección humana, latencia), función de utilidad adaptativa.

El scoring no depende de tablas generales de internet (Leaderboards sintéticos), sino del desempeño comprobable en el entorno específico de **Freddy Taborda** (Linux CachyOS / Debian Proxmox, Go/Rust nativo, cero bloat NPM, cumplimiento estricto de la [LEY_PRINCIPAL.md](file:///home/freddy/Workspace/Obsidian/LEY_PRINCIPAL.md)).

#### Dimensiones Mínimas de Evaluación:
1. `user_id`: Identificador de usuario (`freddy`).
2. `repo`: Repositorio específico (`agent-orchestrator`, `sge-go`, `security-manager-ng`, `obsidian`).
3. `language_stack`: Stack tecnológico (`go`, `rust`, `shell`, `markdown`, `sql`).
4. `task_type`: Tipo de tarea (`feature`, `bugfix`, `refactor`, `documentation_rdd`, `review_security`).
5. `risk_level`: Nivel de riesgo (`bajo`, `medio`, `alto`, `critico`).
6. `agent_id`: Agente evaluado (`qwen-code`, `claude-code`, `agy`, `hermes`).
7. `provider_id`: Proveedor (`bailian`, `anthropic`, `openrouter`).
8. `model_id`: Modelo exacto (`qwen3.8-max`, `claude-3-7-sonnet`, `glm-5`).
9. `mode`: Modo de ejecución (`agentic`, `edit`, `review`).
10. `timestamp_bucket`: Ventana temporal para ponderación por decaimiento.

#### Métricas Cuantitativas Ponderadas:
- **Tasa de Éxito y Recibo Válido ($S_{rec}$):** 1.0 si generó `status=validated` con evidencia git real; 0.0 si falló.
- **Obediencia a Restricciones ($C_{law}$):** Penalización de 0.0 si intentó `sudo`, modificó manuales inmutables, generó deuda técnica o usó herramientas no autorizadas.
- **Calidad Técnica y Compilación ($Q_{tech}$):** Pasa `go test ./...` / `cargo test` / linters sin errores en el primer intento.
- **Actualización de Documentación ($D_{doc}$):** Actualiza diagramas y notas de Obsidian cuando el cambio lo exige.
- **Eficiencia de Costo y Cuota ($E_{cost}$):** Puntuación basada en el consumo relativo al presupuesto y plan activo (ej. modelos en plan flat rate vs API por token).
- **Intervención Humana Requerida ($H_{inv}$):** Medición de cuántos turnos adicionales requirió Freddy para corregir la entrega.

#### Función de Score Compuesto ($Score$):
$$Score = w_1 S_{rec} + w_2 C_{law} + w_3 Q_{tech} + w_4 D_{doc} + w_5 E_{cost} - w_6 H_{inv} - \text{Penalizaciones}$$

---

### 3.7. Criterio 7: Política de Seguridad y Control de Secretos

- [x] **Checklist Criterio 7:** Manejo de metadata segura, sanitización de credenciales, protocolo de confirmación ante archivos sensibles (`.secrets`), mención de hallazgos de guardia de raíz.

#### Directrices Estrictas de Seguridad:
1. **Metadata Segura Únicamente:** Los comandos de autodiscovery (`discover`, `refresh`, `doctor`) tienen **prohibido** leer o volcar al stdout/logs tokens de API, contraseñas, hashes privados o contenidos de archivos de variables de entorno.
2. **Sanitización de Salidas:** En los snapshots y eventos de Observer, las claves se reportan exclusivamente como booleanos (`has_token: true`) o mediante máscaras criptográficas no reversibles (`token_sha256_prefix: "a1b2..."`).
3. **Protocolo ante `.secrets` y Archivos Sensibles:**
   - Si durante el discovery o escaneo de carpetas de configuración se detecta la existencia de `.secrets`, `.env`, archivos con permisos restringidos o scripts de prueba de proveedores, el agente **detiene cualquier lectura profunda**.
   - Se requiere autorización explícita de Freddy para auditar dichos directorios.
4. **Hallazgos de Guardia de Raíz del Caso Piloto:**
   Durante la primera sesión de Qwen Code se detectaron archivos en la raíz del entorno:
   - `.hermes.md` (archivo de instrucciones)
   - `CLAUDE.md` (configuración de arnés)
   - `.rtk` (estado del wrapper)
   - `.secrets` (archivo privado sensible)
   - `test_bailian_provider.py` (script de prueba local)
   - `test_hook.txt` (archivo temporal de testing)
   - `violates.txt` (registro de auditoría de pruebas)

   **Tratamiento:** Estos archivos se tratan como metadatos de entorno. `.secrets` queda permanentemente excluido de cualquier indexación o transmisión.

---

### 3.8. Criterio 8: Contrato de Telemetría con SGE Observer

- [x] **Checklist Criterio 8:** Definición del evento `agent.discovery.snapshot`, endpoint `POST /api/events/ingest`, autenticación con `X-Host-Token`, estructura del payload fechado.

Para mantener sincronizada la torre de control central en `sge-panel.humanbyte.net`, `orq` expone y transmite los snapshots fechados hacia **SGE Observer LLM**.

#### Contrato de Ingesta:
- **Método HTTP:** `POST /api/events/ingest`
- **Autenticación:** Header `X-Host-Token: <token>` (cargado desde `~/.config/sge-observer/agent-orchestrator.host-token`).
- **Tipo de Evento:** `agent.discovery.snapshot`

#### Estructura del Payload JSON:
```json
{
  "event_type": "agent.discovery.snapshot",
  "timestamp": "2026-09-04T21:35:00Z",
  "host": "contabo-sge-panel",
  "host_ip": "127.0.0.1",
  "payload": {
    "snapshot_id": "snap-20260904-213500-host01",
    "discovered_agents_count": 4,
    "active_models_count": 12,
    "agents_summary": [
      {
        "agent_id": "qwen-code",
        "version": "1.4.2",
        "provider": "bailian",
        "active_model": "qwen3.8-max",
        "plan_status": "active_standard",
        "plan_expires_at": "2026-09-24T12:00:00Z"
      },
      {
        "agent_id": "claude-code",
        "version": "1.0.18",
        "provider": "anthropic",
        "active_model": "claude-3-7-sonnet",
        "plan_status": "payg_promo_50pct"
      }
    ],
    "verification_signature": "sha256:d4e5f6..."
  }
}
```

---

### 3.9. Criterio 9: Efecto de Fallos de Delegación en Scoring (`not_executed`)

- [x] **Checklist Criterio 9:** Mapeo con estados de #79 (`DelegateReceipt`), penalizaciones por `not_executed`, `plan_solo`, `timeout_sin_evidencia`, decaimiento en la matriz de enrutamiento.

Un agente que acepta una delegación pero no entrega resultados ejecutados genera costos innecesarios y pérdida de tiempo. `orq` vincula directamente los estados del recibo de delegación (`DelegateReceipt`, #79) con el score del par `(agente, modelo)`:

```text
┌──────────────────────────────────────────────────────────────────────────────────┐
│ Impacto en Scoring según Estado de Delegación (#79)                              │
├───────────────────────┬───────────────────────────────┬──────────────────────────┤
│ Estado del Recibo     │ Veredicto                     │ Impacto en Score         │
├───────────────────────┼───────────────────────────────┼──────────────────────────┤
│ validated             │ util                          │ +1.0 (Refuerzo positivo) │
├───────────────────────┼───────────────────────────────┼──────────────────────────┤
│ executed (sin tests)  │ indeterminado                 │ +0.2 (Neutro/revisión)   │
├───────────────────────┼───────────────────────────────┼──────────────────────────┤
│ failed: not_executed  │ non_util                      │ -2.5 (Penalización severa│
│                       │                               │ en reliability_score)    │
├───────────────────────┼───────────────────────────────┼──────────────────────────┤
│ failed: plan_solo     │ non_util                      │ -1.5 (Penalización por   │
│                       │                               │ omisión de ejecución)    │
├───────────────────────┼───────────────────────────────┼──────────────────────────┤
│ failed: timeout       │ non_util                      │ -2.0 (Penalización por   │
│                       │                               │ latencia/bloqueo)        │
└───────────────────────┴───────────────────────────────┴──────────────────────────┘
```

#### Efecto en el Router Dinámico:
1. **Degradación de Prioridad:** Cuando un par `(agente, modelo)` acumula dos fallos consecutivos de tipo `not_executed` en un `(repo, task_type)`, el router lo remueve temporalmente del slot primario y activa el **Fallback Adaptativo** (#148) hacia el siguiente candidato elegible.
2. **Cuarentena Automática:** Si la tasa de fallos de ejecución supera el 30% en una ventana de 24 horas, el modelo entra en estado `quarantine_needs_doctor` hasta que se ejecute exitosamente `orq agents doctor` o se valide un nuevo recibo.

---

## 4. Relación con Otras Iniciativas del Orquestador

### 4.1. Relación con el Issue #148 (Model-Market Sync & Adaptive Fallback)
El issue #148 estableció la base del catálogo dinámico, el soporte para promociones de mercado (ej. Claude +50%) y el fallback adaptativo cuando un proveedor falla. Este RDD #81 **completa y cierra el ciclo** al:
- Pasar de un feed externo a un **discovery local bidireccional** (`orq agents discover`).
- Incorporar planes con vigencia temporal fija (como el Qwen Standard Plan de 30 días).
- Medir la fiabilidad empírica para alimentar el fallback con métricas reales y no solo disponibilidad de API.

### 4.2. Relación con el Issue #79 (Receipt Driven Development & `DelegateReceipt`)
El issue #79 formalizó que todo trabajo debe dejar evidencia comprobable (hashes de commits, branches, PRs, diffs). El sistema de aprendizaje de este RDD utiliza los recibos emitidos como la **fuente primaria de verdad histórica** para calcular los scores sin intervención subjetiva.

---

## 5. Fuera de Alcance Inicial

Para garantizar seguridad y foco, se mantienen explícitamente fuera del alcance inicial:
1. **Auditoría no autorizada de `.secrets`:** No se inspeccionarán archivos protegidos o privados sin confirmación expresa de Freddy en el turno operativo.
2. **Despliegues o mutaciones en producción:** El discovery es puramente de lectura y diagnóstico; no realiza despliegues a servidores remotos ni altera infraestructura.
3. **Asunción de inventarios definitivos:** No se marcarán modelos como verificados sin evidencia directa de runtime o recibo de ejecución exitoso.
4. **Rollouts masivos sin PR piloto:** Toda implementación del motor de discovery o scoring se desarrollará bajo PRs individuales y probados de forma aislada.

---

## 6. Follow-ups e Implementación Futura

1. **Fase 1 (CLI & Discovery Engine):** Implementar en Rust/Go los subcomandos `orq agents discover`, `orq agents refresh <agent>` y `orq agents doctor` con parsers para Qwen Code, Claude Code y Hermes.
2. **Fase 2 (Motor de Scoring & Persistencia):** Implementar la capa `internal/score` y `internal/discovery` con persistencia en SQLite para registrar métricas de ejecución por repo/tarea.
3. **Fase 3 (Integración con Observer):** Conectar el emisor de eventos de `orq` con el endpoint `POST /api/events/ingest` de SGE Observer para telemetría centralizada de snapshots.
