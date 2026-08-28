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

## Política de merge single-maintainer

Si el repo tiene un solo maintainer real:

- mantener PR obligatorio;
- usar `Required approvals = 0`;
- exigir checks automáticos verdes;
- no autoaprobar ni usar cuentas sello;
- pausar cambios de alto riesgo hasta confirmación explícita del maintainer.

Ver: [Política de protección de ramas](docs/politica-branch-protection.md).

## Revisión 4R

Todo PR debe cubrir:

- Legibilidad.
- Robustez.
- Riesgo.
- Seguridad.

## Anti-caos de issues/PRs

Si varios issues/PRs apuntan al mismo problema, no seguir parcheando síntomas. Crear una auditoría de raíz y replanificar.
