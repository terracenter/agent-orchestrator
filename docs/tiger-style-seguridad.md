# Tiger Style aplicado al workspace

> [!DANGER]
> Seguridad primero: validar temprano, fallar claro y cerrar seguro. Un agente o modelo puede equivocarse; el sistema debe proteger al repo, al host y a los datos.

## Reglas prácticas

| Regla | Aplicación |
|---|---|
| Validar entradas | Paths, flags, JSON, modelos, providers, URLs y variables obligatorias. |
| Fail closed | Si falta auth, token, permiso, config o evidencia, no continuar con acciones sensibles. |
| Explícito sobre implícito | No adivinar modelo, rama, dominio, entorno ni intención destructiva. |
| Menor privilegio | Ejecutar con el acceso mínimo necesario. |
| Sin secretos en salida | No imprimir API keys, tokens, hashes sensibles ni `.env`. |
| Operaciones reversibles | Preferir PRs, commits pequeños, backups y rollback. |
| Estados ambiguos bloquean | Worktree sucio, colisión, CI fallido o review pendiente detienen entrega. |

## Acciones que requieren confirmación humana

- Deploy producción.
- Cambios de auth/autorización.
- Rotación o lectura de secretos.
- Borrado de datos o migraciones destructivas.
- Cambios de firewall/red pública.
- Merge cuando la política del repo no esté satisfecha.
- Uso de modelo caro o no verificado.

## Acciones autónomas permitidas si pasan guardias

- Documentación.
- Tests.
- Refactors pequeños.
- Comandos de lectura.
- Generación de reportes.
- Preparar PRs sin mergear si falta review.

## Checklist rápido

Antes de ejecutar:

- [ ] ¿Estoy en el repo/rama correcta?
- [ ] ¿Hay `git status` limpio o cambios esperados?
- [ ] ¿La acción puede borrar/modificar datos reales?
- [ ] ¿Hay secretos involucrados?
- [ ] ¿Existe rollback?
- [ ] ¿La evidencia será verificable?

## Integración con `orq`

`orq` debe favorecer validaciones deterministas sobre megaprompts:

```bash
orq guard-collision --path /repo
orq repo check --path /repo
orq review 4r --path /repo
orq budget --context-percent N --codex-5h-percent N
```

Si una guardia de seguridad falla, `orq` debe explicar el motivo y no inventar una ruta alternativa.
