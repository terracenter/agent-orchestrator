# Contribución

## Idioma

- Español Venezuela para documentación principal, issues y PRs.
- Inglés Americano para `README.en.md` y documentación secundaria pública.
- Inglés técnico en código cuando sea convención del ecosistema.

## Flujo de trabajo

1. Crear issue o vincular uno existente.
2. Crear rama corta y descriptiva.
3. Implementar cambios pequeños.
4. Actualizar documentación/changelog si aplica.
5. Ejecutar quality gate local.
6. Abrir PR con checklist 4R.

## Quality gate local

```bash
make test
make security
make build
```

## Revisión 4R

Todo PR debe cubrir:

- Legibilidad.
- Robustez.
- Riesgo.
- Seguridad.

## Anti-caos de issues/PRs

Si varios issues/PRs apuntan al mismo problema, no seguir parcheando síntomas. Crear una auditoría de raíz y replanificar.
