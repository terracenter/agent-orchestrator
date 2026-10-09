# RDD — Subagentes y Handoffs Autocontenidos

> Estado: Propuesta / Investigación  
> Fecha: 2026-10-09  
> Origen: Lecciones aprendidas en la sesión de investigación con AGY (Google Antigravity) y Claude Code  
> Contexto: Optimización de tokens, desacoplamiento de contexto y enrutamiento económico en `agent-orchestrator`

---

## 1. Problema

Cuando un modelo de alta capacidad (ej. Claude Opus / Gemini Pro) ejecuta una tarea compleja:
1. **Saturación de contexto:** Las subtareas mecánicas (inventarios, regex, conteo de tokens, extracción de citas, filtrado de archivos) arrastran el historial completo de la conversación.
2. **Desperdicio de presupuesto:** Se pagan tokens de entrada/salida de modelos costosos para tareas que no requieren razonamiento abstracto.
3. **Pérdida de foco:** A medida que la ventana de contexto crece con outputs voluminosos de comandos, la precisión del modelo en la tarea principal se degrada.

## 2. Patrón de Referencia (AGY & Claude Code)

Tanto AGY (`invoke_subagent`, `define_subagent`) como Claude Code implementan dos principios fundamentales:

1. **Subagentes en Segundo Plano:** El agente supervisor puede generar agentes hijos efímeros o especializados, aislados en su propio espacio de memoria/contexto.
2. **Asignación Asimétrica de Modelos:**
   - **Supervisor / Arquitecto:** Modelo de alto razonamiento (`pro` / Claude 3.5 Sonnet / Opus).
   - **Worker / Subagente Mecánico:** Modelo rápido, barato y de bajo consumo (`flash`, `flash-lite`, o modelos locales en `minipc` vía Ollama/llama.cpp).

## 3. Principio del Handoff Autocontenido

Para que la delegación a un subagente o modelo secundario sea 100% eficiente y reproducible:

- **Aislamiento total:** El worker **no** debe recibir el chat histórico.
- **Estructura del Handoff (`handoff-<tarea>.md`):**
  1. **Objetivo:** Una sola oración clara.
  2. **Inputs estrictos:** Rutas exactas de archivos o comandos `rtk` a ejecutar.
  3. **Contrato de Salida:** Formato estructurado rígido (JSON, YAML o tabla Markdown) con ruta de persistencia en disco.
- **Cierre por Evidencia:** El worker escribe su salida en disco y reporta un `receipt` con status, tiempo y tokens consumidos. El supervisor solo lee el `receipt` y el resumen mínimo.

## 4. Implicaciones para `agent-orchestrator` (`orq`)

En el diseño futuro de `orq` / `orq-agent`:

1. **Primitiva `orq delegate --handoff <file.md>`:**
   - En lugar de pasar un prompt libre, `orq` debe aceptar un archivo de handoff autocontenido.
   - `orq route` evalúa la naturaleza del handoff y selecciona el runner más económico disponible (`qwen-code` local, Gemini Flash, etc.).
2. **Generación de Receipts de Subtareas:**
   - Cada subagente produce un recibo estructurado que se anexa al ledger de la sesión.
3. **Resiliencia y Local-First:**
   - Priorizar ejecución local en `minipc` para tareas de extracción y embeddings, con fallback a runners cloud si el host no está disponible.

---

## 5. Tareas de Investigación Abiertas

- [ ] Especificar el schema JSON/Markdown formal para archivos de handoff en `orq`.
- [ ] Evaluar interoperabilidad con la API de subagentes de AGY (`antigravity-sdk-python`).
- [ ] Medir el ahorro porcentual de tokens obtenido mediante handoffs aislados frente a ejecución monolítica.
