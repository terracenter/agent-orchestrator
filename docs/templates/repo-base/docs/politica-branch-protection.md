# Política de protección de ramas

## Objetivo

Mantener entregas seguras sin inventar aprobaciones falsas.

## Modo recomendado para repos single-maintainer

Cuando el repositorio tenga un solo maintainer real, la protección de `main` debe configurarse así:

| Regla | Valor |
|---|---|
| Require a pull request before merging | Activado |
| Required approvals | `0` |
| Require status checks to pass | Activado |
| Require branches to be up to date before merging | Activado si el flujo lo permite |
| Require conversation resolution | Activado |
| Block force pushes | Activado |
| Block deletions | Activado |

## Checks mínimos requeridos

- CI/pruebas del proyecto.
- `make test` o equivalente.
- `make security` o equivalente.
- Checklist 4R documentado en el PR.
- Recibo RDD cuando el repo ya tenga soporte para recibos.

## Reglas de seguridad

- No desactivar protecciones temporalmente para mergear rápido.
- No usar cuentas secundarias como sello sin revisión real.
- No autoaprobar PRs creados por la misma identidad.
- Cambios de alto riesgo requieren confirmación explícita del maintainer aunque los checks estén verdes.

## Cambios de alto riesgo

Pausar y confirmar antes de mergear si el PR toca:

- auth/autorización;
- secretos/tokens;
- firewall/red pública;
- deploy/infraestructura;
- migraciones SQL;
- dependencias críticas;
- borrados masivos o cambios irreversibles.
