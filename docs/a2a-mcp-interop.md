# Interop A2A/MCP — qué tomamos y qué no

Fuentes auditadas:
- Codelab "Intro to A2A Purchasing Concierge" (Google, 2026)
- Documentación oficial A2A y MCP referida desde el codelab

## Veredicto

A2A **no es la visión central** de `orq`. El eje que los separa no es el transporte (A2A no exige
internet; puede correr en loopback) sino **opacidad y control del peer**: A2A asume peers opacos y
autónomos que tú no lanzaste (negociación de modalidades, auth, superficie de red). `orq` lanza sus
propios procesos y controla su ciclo de vida y su gasto.

El interop agente↔agente de `orq` **ya existe** y es local: `.agents/handoffs/*.md` + git. Es nuestro
"A2A de facto", y por ahora es suficiente.

## Qué es cada protocolo

| Protocolo | Problema que resuelve | Equivalente en `orq` |
|---|---|---|
| MCP | Conectar el LLM con herramientas, recursos y prompts | Capa de tools/resources: `rtk` (tools), `vg`/vault (resources), hooks |
| A2A | Conectar agentes entre sí como agentes | Delegación local: `.agents/handoffs` + git, `orq delegate` + adapters |

Titular adoptado: **"MCP para herramientas, A2A para agentes"**. Criterio operativo que lo decide
(verificable, no filosófico): **¿quién decide el costo?** Si el callee elige modelo, itera o
re-delega, es agente (necesita presupuesto + receipt + breaker). Si el gasto es determinístico del
caller, es tool.

## Ideas a adoptar

| Idea | Aplicación en `orq` |
|---|---|
| **AgentCard** (capacidades autodescriptivas, con `skills[]` + versionado) | Fuente en el pipeline Rust de discovery (#81): `detect` + `models snapshot` + certstore + SQLite — NO en `agent-profiles.json` (curación manual). La card versiona; el catálogo cachea; `detect` revalida (si la versión cacheada no coincide, refresca y marca drift). |
| **Máquina de estados de tarea explícita** | Estados tipados `submitted / working / input-required / completed / failed / canceled` en el receipt; formalizar `session_id` además de `correlation_id`. Estados terminales llevan `reason` (`canceled` distingue breaker-trip de cancelación manual). `session_id` agrupa N `correlation_id` (una sesión, varios runs). |

## Ideas a adaptar

| Idea | Adaptación |
|---|---|
| **MCP para la capa de herramientas** | El CLI sigue siendo la vía canónica (`rtk` vía hook, overhead ~0). MCP server solo para clientes que NO pueden ejecutar el binario (Codex, cliente ajeno). No envolver `rtk` como MCP por defecto: sería retroceso neto en tokens. |
| Descubrimiento por URL `/.well-known/agent.json` | Catálogo local SQLite + `orq agents detect` (ya existe). |
| **MCP no son solo tools** | Distinguir *Tools* (mutación: `rtk`, shell) de *Resources* (lectura pasiva: `vg`, vault) y *Prompts*. Mejor control de permisos/auditoría; evita forzar documentos como "tools". |
| **Artifact vs Message** | `artifact` (salida durable) ↔ receipt; `message` (efímero) ↔ stdout. Define qué persiste el receipt store. |
| Notificaciones push A2A | No; `orq` es pull/síncrono con receipts. |

## Ideas a descartar

| Idea | Razón |
|---|---|
| Mesh de agentes remotos vía HTTP (Cloud Run / Agent Engine) | `orq` es local-first; la interop remota no es el objetivo central. |
| Escenario "compra/venta" multi-vendedor del codelab | Ilustrativo; no aplica al dominio. |
| A2A como protocolo de red central | Rompe local-first y agrega superficie de red/autenticación sin necesidad actual. |

**Disparador de reapertura:** se reabre A2A cuando `orq` deba coordinar un agente que **no lanzó**
(remoto/ajeno); antes de cualquier mesh ad-hoc, evaluar `orq` como *cliente* A2A.

## Impacto en ROADMAP

- Reforzar la fase de discovery (#81) con formato tipo AgentCard (skills, versionado, permisos).
- Evaluar `rtk`/`vg` como MCP servers **solo** para clientes que no ejecuten el binario; el CLI sigue canónico.
- Formalizar estados de tarea + `session_id` en receipts (contrato de ciclo de vida).

## Regla de síntesis

`orq` no se vuelve un nodo A2A. Toma el borde **MCP = herramientas/recursos / A2A = agentes**,
la idea de **capacidades autodescriptivas versionadas** y la **máquina de estados de tarea**,
sin renunciar a local-first, evidencia y costo mínimo.
