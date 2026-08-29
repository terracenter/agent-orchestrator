# Control móvil seguro con Telegram y WireGuard

Este documento define el alcance inicial para operar `orq` desde teléfono sin exponer Pi/OpenAI ni publicar endpoints abiertos.

## Principios

- `orq` sigue siendo la autoridad operativa.
- Telegram es solo UI de comandos, no un shell remoto.
- Exposición recomendada: bot/servicio accesible solo desde red local o WireGuard.
- No guardar secretos en git.
- `TELEGRAM_BOT_TOKEN` debe venir de variable local o archivo con permiso `0600`.
- Cualquier acción de escritura requiere confirmación explícita.

## Comandos read-only permitidos

Estos comandos no ejecutan acciones peligrosas:

- `/status`
- `/agents`
- `/route <tarea>`
- `/task_list`
- `/task_show <id>`

## Comandos de escritura con confirmación

Estos comandos solo pueden preparar una acción y deben responder con una solicitud de confirmación antes de ejecutar:

- `/task_create ...`
- `/task_update ...`
- `/task_assign ...`
- `/handoff_draft ...`
- `/supervise_start ...`
- `/supervise_stop ...`

## Autenticación y red

Capas obligatorias/recomendadas:

1. Allowlist de `telegram user_id`.
2. Token local del bot por env o archivo `0600`.
3. Endpoint restringido por WireGuard o loopback detrás de túnel controlado.
4. Auditoría de cada mensaje recibido, autorizado o no.

## Auditoría mínima

Cada comando debe registrar:

- usuario (`user_id`)
- comando
- si fue autorizado
- si requiere escritura/confirmación
- modelo asignado
- costo estimado
- resultado
- timestamp

## Modelos por defecto

Para comandos iniciados desde móvil se prefieren modelos baratos:

- `nvidia-api/openai/gpt-oss-20b`
- `nvidia-api/openai/gpt-oss-120b`
- `agy/gpt-oss-120b-medium`
- `agy/gemini-3.5-flash-low`

No usar Pi/OpenAI salvo override explícito y auditado.

## Estado actual

El paquete `internal/mobile` implementa el router seguro y testeable de Telegram: allowlist, comandos read-only, compuerta de confirmación para escritura y auditoría estructurada. La integración real con la API de Telegram y `orq serve` queda como siguiente etapa.
