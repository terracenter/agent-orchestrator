# NOMBRE_DEL_PROYECTO

> [!IMPORTANT]
> Estado actual: describir si el proyecto está en diseño, desarrollo, producción o mantenimiento.

Descripción breve en Español Venezuela: qué resuelve, para quién y por qué existe.

## Arquitectura

| Capa | Tecnología | Motivo |
|---|---|---|
| Backend | Go + Chi/net/http | Nativo, simple, auditable |
| UI | Go Templates + HTMX/Alpine + Tailwind | Sin SPA pesada en producción |
| Datos | PostgreSQL/SQLite | Según necesidad operativa |
| Deploy | Binario Go / contenedor mínimo | Menos superficie, fácil rollback |

## Seguridad

> [!WARNING]
> No commitear secretos, tokens, `.env` reales ni dumps de base de datos.

Mínimos obligatorios:

- secretos fuera de git;
- archivos sensibles con permisos `600`;
- auth/autorización documentada;
- logs sin credenciales;
- validación de entradas;
- rollback documentado antes de producción.

## Requisitos

```bash
go version
```

Agregar aquí dependencias del sistema si aplican.

## Desarrollo local

```bash
make dev
```

## Validación

```bash
make test
make security
make build
```

## Primeros pasos

```bash
cp .env.example .env
chmod 600 .env
make dev
curl -fsS http://127.0.0.1:PUERTO/api/health
```

## Referencia rápida

| Comando | Uso |
|---|---|
| `make dev` | Levanta entorno local |
| `make test` | Ejecuta pruebas |
| `make security` | Ejecuta checks de seguridad |
| `make build` | Compila artefacto |

## Archivos de configuración

| Archivo | Permisos | Descripción |
|---|---:|---|
| `.env` | `0600` | Variables locales, no se commitea |
| `.env.example` | `0644` | Plantilla sin secretos |

## Documentación

- [Seguridad](SECURITY.md)
- [Contribución](CONTRIBUTING.md)
- [Historial](RELEASES.md)
- [Diagramas](docs/diagramas/)

## Licencia

Ver `LICENSE`.
