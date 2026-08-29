# Prueba integral de `orq`

Esta guía valida el flujo local-first del orquestador antes de usarlo como arnés real entre Pi, Claude, AGY, Hermes y otros modelos.

## Principios

- `orq` es la autoridad para guardias, seguridad, auditoría, recibos y validación final.
- Otros modelos pueden apoyar como revisión read-only/sandbox, sin permisos peligrosos.
- No usar `--dangerously-skip-permissions`.
- No tocar producción, secretos, DB, DNS, firewall ni acciones irreversibles sin aprobación humana explícita.
- RDD: si un comando no se ejecutó, no se declara como validado.

## Flujo mínimo verificable

Desde la raíz del repo:

```bash
orq guard-collision --path .
orq repo check --path .
orq safety check --path .
orq review 4r --path .
docker compose run --rm dev go test ./...
orq heartbeat run --workspace .
orq audit worktrees --path .
orq audit prs --path .
```

Crear un handoff encadenado usando un archivo origen real:

```bash
tmpdir=$(mktemp -d)
printf '# Handoff previo\n\nresultado=ok\n' > "$tmpdir/from.md"
orq handoff chain --from "$tmpdir/from.md" --to "$tmpdir/to.md" --task "prueba integral orq" --next-agent pi
```

Crear y verificar recibo con múltiples comandos:

```bash
tmpdir=$(mktemp -d)
orq receipt create \
  --task "prueba integral orq" \
  --command "orq guard-collision --path ." --command-result passed \
  --command "orq repo check --path ." --command-result passed \
  --command "orq safety check --path ." --command-result passed \
  --command "docker compose run --rm dev go test ./..." --command-result passed \
  --evidence "salidas locales verificadas" \
  --rollback "revertir PR asociado" \
  --output "$tmpdir/receipt.json"
orq receipt verify --path "$tmpdir/receipt.json"
```

Validar sesión completa:

```bash
orq session validate \
  --guard-collision OK \
  --repo-check OK \
  --safety-check OK \
  --tests PASS \
  --receipt OK \
  --handoff OK
```

## Criterio de aceptación

La sesión solo se considera lista si:

- `guard-collision` no pide detenerse.
- `repo check` no tiene bloqueos.
- `safety check` reporta riesgo aceptable para el cambio.
- Las pruebas aplicables se ejecutaron en contenedor/CI.
- El recibo se creó y `receipt verify` devolvió OK.
- `session validate` devolvió OK.

## Ejecución de referencia

El 2026-08-29 se ejecutó el flujo en `agent-orchestrator` para documentar PR #46:

- `orq guard-collision --path .` → repo limpio/sin colisión antes de cambios.
- `orq repo check --path .` → OK; desde PR #51 el repo incluye `RELEASES.md` para no depender de una advertencia conocida.
- `orq safety check --path .` → riesgo bajo, 0 findings.
- `orq review 4r --path .` → preguntas 4R generadas.
- `orq heartbeat run --workspace .` → 1 proyecto, 4 fuentes, 3 políticas, 2 acciones.
- `orq audit worktrees --path .` → 1 worktree, 0 findings.
- `orq audit prs --path .` → 0 PRs abiertos al inicio.
- Intento inválido de `handoff chain` con origen inexistente → falló correctamente por archivo inexistente.

Las validaciones finales deben repetirse después de aplicar cada cambio, antes del merge.
