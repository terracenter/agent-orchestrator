# Seguridad

> [!DANGER]
> La seguridad tiene prioridad sobre features, rendimiento y visual.

## Reporte de vulnerabilidades

No publicar secretos ni pruebas explotables en issues públicos. Reportar por canal privado definido por el equipo.

## Reglas obligatorias

- No commitear secretos.
- No imprimir tokens/API keys en logs.
- No exponer hashes/tokens en UI.
- Validar entradas externas.
- Revisar permisos de archivos sensibles (`0600`).
- Mantener dependencias auditadas.
- Documentar rollback para cambios de producción.

## Quality gate de seguridad

```bash
make security
```

Debe incluir, según stack:

- escaneo de vulnerabilidades de dependencias;
- SAST;
- secret scan si aplica;
- revisión de auth/autorización;
- smoke test de endpoints sensibles.

## Checklist 4R de seguridad

- ¿Aumenta la superficie expuesta?
- ¿Cambia auth/autorización?
- ¿Toca secretos, tokens o credenciales?
- ¿Los errores podrían filtrar datos internos?
- ¿Hay rollback claro?
