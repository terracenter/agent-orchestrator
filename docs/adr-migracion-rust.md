# ADR: Migración completa de agent-orchestrator a Rust

## Estado

Aceptada.

## Contexto

El orquestador actual está implementado principalmente en Go y ya tiene routing, ledger, auditoría, detección, receipts y documentación operativa. Sin embargo, el requisito nuevo es más exigente: ejecutar agentes reales, controlar procesos, aplicar timeouts, generar recibos verificables y evitar que Pi sea cuello de botella.

Rust ya demostró valor operativo en el punto más delicado del sistema: ejecución real de agentes, timeouts, process groups, ring buffer acotado, receipts JSON, política de agentes y smoke real con Qwen.

Mantener una arquitectura permanente `orq` Go -> `orq-agent` Rust -> runner/agente agrega capas, contratos duplicados, modelos de error duplicados y riesgo de desincronización. Esa capa intermedia solo es aceptable como compatibilidad temporal.

## Decisión

El proyecto pasa a visión **Rust-first**: el binario objetivo será `orq` en Rust. El código Go queda en modo mantenimiento y referencia de paridad mientras se migra por slices verificables.

No se hará un big-bang rewrite ciego: se portarán comandos por prioridad, con tests y receipts por slice. Pero el cambio de dirección es inmediato: todo desarrollo funcional nuevo debe nacer en Rust salvo fixes críticos de compatibilidad en Go.

El historial operativo previo se conserva en git, pero el changelog público futuro debe iniciar la nueva etapa Rust-first. El objetivo no es justificar costo hundido, sino reducir complejidad final y tener un orquestador local-first de un solo binario.

## Opciones

### Opción A — Mantener Go y agregar fixes

Pros:

- Menor trabajo inicial.
- Aprovecha código existente.
- Menos riesgo de regresión inmediata.

Contras:

- Mantiene el cuello de botella actual si no se rediseña bien.
- Control de procesos/adapters puede crecer como parche.
- Sigue la deuda de handoffs no ejecutados.

### Opción B — Rust solo para `orq-agent`, Go legacy para el resto

Pros:

- Permite probar Rust donde más valor aporta: ejecución real, adapters, timeouts y receipts.
- Reduce riesgo porque no borra Go de inmediato.
- Facilita migración progresiva.

Contras:

- Dos binarios/lenguajes durante transición.
- Hay que definir frontera limpia entre `orq` y `orq-agent`.

### Opción C — Migración completa a Rust

Pros:

- Un solo lenguaje y binario objetivo.
- Tipado fuerte para estados, errores, receipts y políticas.
- Mejor base para concurrencia, procesos hijos y distribución local-first.
- Menos ambigüedad entre agente/provider/modelo.

Contras:

- Más trabajo.
- Riesgo de regresiones en comandos ya existentes.
- Requiere matriz de paridad y migración de tests.
- Builds pueden ser pesados en Pi; minipc ayuda pero no siempre estará disponible por electricidad.

## Criterios para decidir migración completa

Matriz de paridad: [`matriz-paridad-rust.md`](matriz-paridad-rust.md)

Se aprueba migración completa solo si el MVP Rust demuestra:

- ejecución real de `pi`, `openclaw` y `agy` con receipts;
- detección segura sin leer secretos;
- manejo correcto de agentes sin adapter;
- timeouts y errores clasificados;
- compatibilidad con flujos críticos actuales;
- tests suficientes;
- build liviano viable en Pi y build pesado opcional en minipc.

## Consecuencia operativa

- `orq-agent` deja de ser considerado helper experimental y pasa a ser el núcleo inicial de `orq` Rust.
- Go no debe recibir features nuevas salvo las necesarias para mantener compatibilidad durante la migración.
- Cada slice Rust debe incluir tests, actualización de docs y, cuando aplique, receipt o evidencia Orq.
- La frontera Go -> Rust se eliminará al alcanzar paridad suficiente.

## Plan aprobado

Avanzar con Opción C mediante slices:

1. Renombrar/reestructurar el crate Rust para producir binario `orq` además de `orq-agent` o absorber `orq-agent` como módulo interno.
2. Portar primero los comandos de ejecución y descubrimiento: `run`, `agents detect`, `models`, `smoke`, `certify`.
3. Portar estado local: `task`, `ledger`, `record`, `status`.
4. Portar routing/política: `classify`, `route`, capacidad certificada y selección por evidencia.
5. Portar integraciones: `observer`, `receipt`, `session`, `trace`, `safety`, `audit`.
6. Congelar y retirar Go cuando la matriz de paridad esté cerrada.
