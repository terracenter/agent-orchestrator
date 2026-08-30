# Estándar operativo de repositorios

> [!IMPORTANT]
> Este documento define el estándar mínimo para repositorios del workspace. La meta es construir software seguro, funcional, fácil de operar y entendible por otras personas.

## Principios

| Prioridad | Principio | Regla práctica |
|---|---|---|
| 1 | Seguridad | Si no es seguro, no se publica. Si hay duda, se pregunta o se escala a otro modelo/humano. |
| 2 | Funcionamiento y optimización | Debe compilar, probarse, desplegarse y operar con observabilidad mínima. |
| 3 | Facilidad de uso y visual | README claro, comandos copiables, UX simple y visual consistente. |
| Permanente | Documentación al día | Todo cambio operativo debe actualizar README/docs/changelog en el mismo PR. |
| Cultura | Suma positiva / cero ego | Adoptar lo probado, adaptar lo útil, descartar lo pesado. |

## Idioma

| Contexto | Idioma |
|---|---|
| README principal, docs, issues, PRs, planes | Español Venezuela |
| README.en.md y documentación pública secundaria | Inglés Americano |
| Código/API | Inglés técnico cuando sea convención del ecosistema |
| Mensajes CLI para el equipo | Español cuando el usuario final sea el workspace |

## Estructura mínima

```txt
README.md
README.en.md              # si el proyecto es público/importante
CHANGELOG.md o RELEASES.md
SECURITY.md
CONTRIBUTING.md
LICENSE
.env.example
.gitignore
.github/pull_request_template.md
.github/workflows/ci.yml
docs/
docs/diagramas/
Makefile
```

Para Go:

```txt
cmd/
internal/
pkg/                       # solo si expone API reutilizable
migrations/                 # si hay base de datos
```

## README visual estilo Security-Manager

Todo README principal debe priorizar lectura rápida:

- alertas GitHub (`[!IMPORTANT]`, `[!WARNING]`, `[!DANGER]`);
- tabla de arquitectura;
- requisitos explícitos;
- build/test/deploy con comandos copiables;
- comandos remotos de servidores, bases de datos o paneles de deploy documentados sin `rtk`, salvo que el documento indique explícitamente que `rtk` está instalado en ese host;
- primeros pasos en orden recomendado;
- referencia rápida de comandos o endpoints;
- archivos de configuración y permisos;
- enlace a documentación completa.

## Testing estándar

Los quality gates deben mezclar tres fuentes:

1. documentación oficial del lenguaje;
2. estándar de industria;
3. cyberseguridad.

### Go mínimo

```bash
go test ./...
go test -race ./...
go vet ./...
go build ./...
govulncheck ./...
gosec ./...
```

Si hay Docker:

```bash
docker compose config
docker compose build
```

Si hay API/UI:

```bash
curl -fsS http://127.0.0.1:PUERTO/api/health
curl -fsS http://127.0.0.1:PUERTO/
```

### Seguridad mínima

- sin secretos en git;
- `.env` y tokens con permisos `600` cuando aplique;
- auth y autorización probadas;
- CORS explícito;
- cookies `HttpOnly`, `Secure`, `SameSite` cuando aplique;
- no exponer tokens, hashes ni stack traces;
- validar inputs;
- revisar SQL injection, path traversal y SSRF si aplica;
- logs sin credenciales.

## Revisión 4R

Cada PR debe responder:

| R | Pregunta |
|---|---|
| Legibilidad | ¿El código y la documentación se entienden sin explicación externa? |
| Robustez | ¿Falla de forma controlada y tiene pruebas suficientes? |
| Riesgo | ¿Qué puede romper y cómo se revierte? |
| Seguridad | ¿Aumenta superficie, expone secretos o debilita controles? |

## Changelog / releases estilo Asterisk

Usar `CHANGELOG.md` o `RELEASES.md` con secciones claras:

```md
## 1.4.0 — 2026-08-28

### Agregado
### Cambiado
### Corregido
### Seguridad
### Operación
### Compatibilidad
### Rollback
```

Las notas de release deben decir qué cambió, cómo validar y cómo volver atrás.

## Anti-acumulación de issues/PRs

Si aparecen muchos issues o PRs relacionados:

1. no seguir parcheando síntomas;
2. agrupar problemas por causa raíz;
3. hacer auditoría arquitectónica;
4. documentar decisión;
5. cerrar/renombrar issues para que sean accionables;
6. implementar en PRs pequeños y verificables.

## Uso de modelos

- Modelo barato: revisión mecánica, resumen, consistencia.
- Modelo medio: refactor, análisis técnico moderado.
- Modelo fuerte: seguridad, arquitectura, decisiones irreversibles.
- Humano: aprobación cuando hay riesgo real, secretos, deploy o ambigüedad.

Regla: si no sabemos, preguntamos. No hay ego.

## Plantillas base incluidas

Este repo incluye plantillas iniciales en `docs/templates/repo-base/`:

| Archivo | Uso |
|---|---|
| `README.md` | README principal visual en Español Venezuela |
| `README.en.md` | README secundario en Inglés Americano |
| `SECURITY.md` | Política base de seguridad |
| `CONTRIBUTING.md` | Flujo de contribución y 4R |
| `RELEASES.md` | Changelog/release notes estilo Asterisk |
| `Makefile` | Quality gate local para Go |
| `ci.yml` | Workflow base de CI |

## Integración futura con `orq`

Comandos objetivo:

```bash
orq repo check
orq repo init-template
orq review 4r
orq audit issues
orq audit prs
```

El orquestador debe detectar deriva del estándar sin bloquear trabajo urgente de forma ciega: advertir, sugerir y escalar cuando el riesgo sea alto.
