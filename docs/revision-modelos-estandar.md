# Revisión multi-modelo del estándar operativo

> [!IMPORTANT]
> Este documento resume revisiones externas hechas con modelos baratos. Sirve como evidencia de suma positiva/cero ego: si no sabemos o hay duda, preguntamos.

## Revisión AGY — Gemini 3.6 Flash Low

Fecha: 2026-08-28.

### Mejoras recomendadas

1. Quality gate local y CI con fase `validate-and-test` antes de build/deploy.
2. Mantener stack nativo para herramientas operativas: Go, HTMX, Alpine, Tailwind; sin Next.js/React SPA en producción salvo excepción aprobada.
3. Las pruebas deben medir la afirmación exacta y la causa raíz, no solo síntomas ni cobertura superficial.
4. Commits pequeños y trazables, con autoría unificada del workspace.
5. Integrar vault/planes/MOC cuando aplique a documentación de proyecto.
6. Changelog estilo Asterisk por versión, módulo y tipo de cambio.
7. Aislamiento con worktrees para sesiones concurrentes.
8. Cero deuda técnica cuando se toca una superficie: lo que se rompe se arregla en el mismo PR o se declara deuda explícita.
9. Auditar paquetes/licencias antes de incorporar dependencias.
10. Flujo controlado de ramas y promoción a producción.

### Riesgos de burocracia a evitar

1. Duplicar reglas en demasiados lugares; mantener SSoT y referencias claras.
2. Plantillas de issues/PRs demasiado largas.
3. Tests dummy para subir coverage sin validar comportamiento real.
4. Copiar complejidad de DeepSeek Harness o plugin overhead innecesario.
5. Traducir en exceso términos técnicos estándar: mantener `commit`, `branch`, `merge`, `dashboard`, `deploy`, etc.

## Decisión

Adoptar estas recomendaciones como criterios de diseño del estándar, con una regla: el estándar debe automatizarse y reducir fricción, no convertirse en trámite manual.
