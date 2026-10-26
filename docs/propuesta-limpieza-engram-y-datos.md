# Propuesta: limpieza de Engram y de datos personales

Estado: propuesta para evaluar. No cambia código.

## 1. Referencias a Engram en el código

Engram ya no forma parte del flujo de trabajo del mantenedor. Quedan referencias en:
- `internal/guard/` (`guard.go`, `guard_test.go`)
- `internal/guides/guides.go`
- `internal/adapters/adapters.go`
- `orq-agent/` (`src/compliance.rs`, `src/runtime.rs`, `src/lib.rs`, `tests/exec_cli.rs`)

La documentación (`ROADMAP.md` y `docs/`) ya se limpió en #252.

## 2. Datos personales y de red en el repositorio público

Se detectó, en archivos versionados, una dirección IP de red privada, un nombre de host interno y rutas de directorios locales del mantenedor. Archivos conocidos: `receipt.json` e `internal/agent/detect.go`. Falta revisar el historial de PRs e issues.
(Los valores no se reproducen aquí a propósito.)

## 3. Opciones a evaluar

A. Limpiar solo los archivos actuales y dejar el historial como está.
B. Limpiar los archivos y reescribir el historial (afecta a los forks y a los PRs existentes).
C. Archivar el repositorio y continuar el desarrollo en otro.

Para el punto 1: eliminar las referencias, dejarlas, o decidirlo junto con el retiro de `orq`.

## 4. Decisión pendiente

Elegir una opción por cada punto antes de hacer cambios.
