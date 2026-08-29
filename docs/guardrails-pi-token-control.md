# Guardrails Orq — uso de Pi, delegación y loops caros

## Problema observado

Durante una sesión de organización del vault desde Pi, el operador ejecutó trabajo repetitivo directamente y repitió validaciones caras (`vg sync --full --prune`) en lugar de delegar análisis mecánico a agentes/modelos baratos recomendados por `orq budget`.

Esto debe tratarse como bug/mejora de Orq: si Orq es la autoridad operativa, debe advertir cuando la conducta real se desvía del presupuesto, del routing o de buenas prácticas.

## Errores/omisiones que Orq debe detectar

- Uso prolongado del modelo activo de Pi para trabajo mecánico o repetitivo.
- `orq budget` recomienda agentes baratos, pero la sesión sigue ejecutando directo desde Pi.
- `orq delegate` se usa solo como `--dry-run` y no como delegación real cuando aplica.
- Comandos caros repetidos sin cierre de lote, por ejemplo:
  - `vg sync --full --prune`
  - suites completas de tests/builds
  - auditorías completas del vault/grafo
- Validaciones completas antes de aplicar correcciones pequeñas.
- Falta de recomendación explícita de `/compact` cuando hay loops largos o alto consumo.
- Falta de bloqueo/advertencia antes de merge/push si la validación queda incompleta, abortada o con enlaces rotos.

## Comportamiento esperado

Orq debe emitir una advertencia accionable cuando detecte cualquiera de estos patrones:

1. **Pi prolongado:** si el modelo Pi ejecuta varias operaciones mecánicas consecutivas, recomendar delegación a `local-or-cheap`, `agy/gemini-3.5-flash-low`, `agy/gpt-oss-120b-medium` u otro agente barato permitido.
2. **Loop caro:** si se repite un comando caro dentro del mismo frente de trabajo, sugerir validación incremental y reservar la validación completa para el cierre del lote.
3. **Delegación omitida:** si `orq budget` recomienda agentes baratos pero no se delega, marcar desviación operativa.
4. **Compactación:** si el contexto o la secuencia operativa excede umbral, pedir `/compact` antes de continuar.
5. **Pre-merge:** si hay comandos abortados, validaciones rotas o links rotos, impedir recomendar merge/push salvo aprobación humana explícita.

## Criterios de aceptación

- Dado un historial con dos o más `vg sync --full --prune` en el mismo frente, cuando se ejecute `orq budget` o `orq route`, entonces Orq debe advertir `loop caro detectado` y recomendar validación incremental.
- Dado que `orq budget` recomienda agentes baratos, cuando el operador siga con trabajo mecánico desde Pi, entonces Orq debe registrar `delegación omitida`.
- Dado un comando abortado o una validación incompleta, cuando se evalúe cierre/merge/push, entonces Orq debe exigir validación limpia o aprobación explícita.
- Dado uso prolongado de Pi en tareas nivel 1/mecánicas, cuando se consulte Orq, entonces debe recomendar agente barato y compactación si aplica.

## Nota operativa

Este documento nace de feedback directo de Freddy: Orq existe para evitar consumo innecesario de tokens y coordinar agentes/modelos. Si no detecta estas desviaciones, es un bug del orquestador.
## Feedback adicional 2026-08-29 — degradación obligatoria

Freddy reportó nuevamente que Pi siguió consumiendo tokens rápido en tareas que otros agentes/modelos podían hacer. Esto refuerza que no basta con recomendar agentes baratos: Orq debe detectar la desviación en tiempo real y, cuando la tarea sea nivel 1/mecánica/documentación, debe exigir delegación o degradación antes de continuar.

### Bug adicional

- Orq permite que Pi continúe ejecutando pasos delegables después de haber recomendado agentes baratos.
- Orq no emite un corte suficientemente fuerte cuando el patrón se repite en la misma sesión.
- Orq debería registrar este evento como incumplimiento operativo y sugerir `/compact` + delegación real.

### Criterio de aceptación adicional

- Dado que `orq budget` recomienda evitar `pi/openai/gpt-5.5`, cuando se ejecuten más de N acciones delegables desde Pi en la misma tarea, entonces Orq debe responder con `degradación obligatoria` y pedir delegar a agente barato antes de seguir.
## Feedback adicional — delegate debe ser delegación real

Durante la organización del vault, `orq delegate` devolvió instrucciones/prompts pero no evidenció ejecución real por otro agente ni entrega de resultado verificable. Si el operador necesita reducir consumo de Pi, Orq debe distinguir claramente entre `prompt generado`, `dry-run` y `delegación ejecutada con recibo`.

### Criterio de aceptación adicional

- Dado un uso de `orq delegate` sin `--dry-run`, cuando Orq no ejecute realmente un agente externo o barato, entonces debe devolver estado explícito `not_executed` y un siguiente paso obligatorio para ejecutar la delegación real antes de continuar con Pi.
## Feedback adicional — costos Pi/Observer mal interpretados

Freddy reportó que Pi muestra métricas como `↑5.2M ↓550k R109M CH96.8% $97.152 (sub) 17.0%/272k (auto) openai-codex gpt-5.5 minimal`, pero su costo real es plan mensual de aproximadamente USD 20, con cobro cercano a USD 21 por PayPal. El Observer no debe interpretar ese `$97.152` como costo facturado real si corresponde a métrica interna/subscription/estimación.

### Bug adicional

- Observer/Orq necesita separar costo estimado por tokens, costo cubierto por suscripción y costo real facturado del usuario.
- Debe permitir configurar plan real (`monthly_plan_usd`) y recargo/pasarela (`payment_fee_usd` o porcentaje).
- Las alertas de presupuesto deben mostrar claramente `estimado`, `cubierto por suscripción` y `facturado real esperado`.

### Criterio de aceptación adicional

- Dado un usuario con plan mensual USD 20 y cobro real aproximado USD 21, cuando Pi reporte una métrica interna `$97.152 (sub)`, entonces Observer debe registrar esa cifra como estimación/subscription y no como cobro real, mostrando el costo real configurado por el usuario.
