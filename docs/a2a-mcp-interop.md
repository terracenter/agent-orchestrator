# Interop A2A/MCP — qué tomamos y qué no

Fuentes auditadas:
- Codelab "Intro to A2A Purchasing Concierge" (Google, 2026)
- Documentación oficial A2A y MCP referida desde el codelab

## Veredicto

A2A **no es la visión central** de `orq`. A2A estandariza interop **remota** entre agentes
(vía HTTP, AgentCard en `/.well-known/agent.json`, ciclo mensaje → tarea → artefacto). `orq` es
orquestación **local-first** de procesos con **costo mínimo + evidencia verificable**. Son
problemas complementarios, no equivalentes.

Sí adoptamos patrones puntuales, con la regla de siempre: adoptar lo probado, adaptar lo útil,
descartar lo pesado.

## Qué es cada protocolo

| Protocolo | Problema que resuelve | Equivalente en `orq` |
|---|---|---|
| MCP (Model Context Protocol) | Conectar el LLM con herramientas y datos | Capa de tools: `rtk`, `vg`, shell, hooks |
| A2A (Agent2Agent) | Conectar agentes entre sí como agentes, no como tools | Capa de delegación: `orq delegate`, adapters |

La división oficial es **"MCP para herramientas, A2A para agentes"** — el borde arquitectónico que
`orq` debe hacer explícito.

## Ideas a adoptar

| Idea | Motivo | Aplicación en `orq` |
|---|---|---|
| AgentCard (tarjeta de capacidades autodescriptiva) | Descubrir qué sabe/permite un agente sin hardcodear | Evolucionar `agent-profiles.json` hacia capacidades autodescriptivas; conecta con #81 (discovery dinámico) |
| Ciclo de vida de tarea (mensaje → tarea → artefacto) | Modelo claro de entrada/proceso/salida con session id | Ya existe (handoff → exec → receipt); formalizar `correlation_id`/session como contrato |
| MCP para la capa de herramientas | Estandariza el acceso de agentes a tools, en vez de hooks ad-hoc | `rtk`/`vg` expuestos como MCP servers; el borde agente/tool queda limpio |

## Ideas a adaptar

| Idea | Adaptación |
|---|---|
| Descubrimiento por URL `/.well-known/agent.json` | En `orq` local no hay HTTP; usar catálogo local SQLite + `orq agents detect` (ya existe) |
| Notificaciones push de tareas A2A | `orq` es pull/síncrono con receipts; no agregar push hasta necesitarlo |

## Ideas a descartar para este workspace

| Idea | Razón |
|---|---|
| Mesh de agentes remotos vía HTTP (Cloud Run / Agent Engine) | `orq` es local-first; la interop remota no es el objetivo central |
| Escenario "compra/venta" multi-vendedor del codelab | Ilustrativo; no aplica al dominio sysadmin/vault/repos |
| A2A como protocolo de red central | Rompe local-first y agrega superficie de red/autenticación sin necesidad actual |

## Impacto en ROADMAP

- Reforzar la fase de discovery (#81) con formato tipo AgentCard.
- Evaluar `rtk`/`vg` como MCP servers (herramientas), manteniendo agentes por delegación.
- Formalizar `correlation_id`/session en receipts como contrato de ciclo de tarea.

## Regla de síntesis

`orq` no se vuelve un nodo A2A. Toma el borde **MCP = herramientas / A2A = agentes** y la idea de
**capacidades autodescriptivas**, sin renunciar a local-first, evidencia y costo mínimo.
