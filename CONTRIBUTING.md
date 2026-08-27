# Contributing

Gracias por querer contribuir a `agent-orchestrator`.

## Idiomas

- Idioma principal del proyecto: Español de Venezuela.
- Documentación pública secundaria: Inglés americano.
- Cambios de documentación pública deben mantener `README.md` y `README.en.md` alineados cuando aplique.

## Flujo de trabajo

1. Crear una rama `dev-<tarea>` desde `main` actualizado.
2. Hacer commits pequeños y verificables.
3. Ejecutar pruebas antes de abrir o actualizar un PR:

```bash
docker compose run --rm dev go test ./...
```

4. Abrir Pull Request contra `main`.
5. Esperar que el check `go-test` pase.

## Seguridad

- No subir secretos, tokens, `.env`, claves SSH/GPG ni outputs sensibles.
- `main` está protegido: no force push, no borrado, y cambios vía PR.
- La ejecución automática de agentes no forma parte del MVP inicial.

## Licencia

Las contribuciones se aceptan bajo AGPLv3 o posterior.
