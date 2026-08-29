# Uso de `orq`

`orq` es el CLI local de `agent-orchestrator`.

> Ojo: el comando es `orq`, con **q** de orquestador. No es `org`.

## Validar que está instalado

```bash
orq --help
```

Si tu shell no lo encuentra, usa la ruta completa:

```bash
/home/freddy/.local/bin/orq --help
```

En esta máquina el binario instalado vive en:

```txt
/home/freddy/.local/bin/orq
```

`~/.zshrc` ya debe incluir `~/.local/bin` en el `PATH`. Si una sesión no lo hereda, recarga el shell:

```bash
source ~/.zshrc
```

## Comandos básicos

### Ver guías versionadas del binario

```bash
orq docs usage
orq docs orchestration
```

Estas guías salen del propio binario para evitar documentación operativa desfasada.

### Clasificar una tarea

```bash
orq classify "corregir una referencia rota"
```

Salida esperada:

```txt
mecanico
```

### Recomendar agente/modelo

```bash
orq route "rotar token de producción"
```

Salida esperada:

```txt
agent=claude-code model=sonnet level=3 category=seguridad reason=seguridad sobrescribe costo
```

### Simular ejecución sin ejecutar agentes

```bash
orq run "auditar proyecto" --dry-run
```

Salida esperada:

```txt
dry-run=true executed=false agent=local-or-cheap model=lowest-sufficient
```

### Registrar evidencia en el ledger

```bash
orq record --task test --agent pi --model gpt-5.5 --status ok
```

Por defecto escribe en:

```txt
~/.local/state/orq/ledger.jsonl
```

### Ver estado del ledger

```bash
orq status
```

### Ver agentes/modelos permitidos

Cada asignación debe indicar **agente y modelo**. No basta decir “usa Claude” o “usa AGY”.

```bash
orq agents
orq agents --format json
```

### Detectar agentes instalados en el sistema

Inspecciona de forma segura la presencia y rutas de runners locales (`openclaw`, `agy`, `hermes`, `claude`, etc.) sin leer ni exponer credenciales:

```bash
orq agents detect
orq agents detect --format json
```

### Diagnóstico del entorno (`orq doctor`)

Verifica la disponibilidad de herramientas clave (`rtk`, `git`, `gh`, `orq`, `vg`) y agentes configurados en el host. Para `vg`, la detección se realiza en orden: variable `ORQ_VG_PATH`, `$PATH` y rutas conocidas del workspace (ej. `Workspace/Obsidian/10.Tooling/vault-graph/vg` o `Workspace/Obsidian/Tooling/vault-graph/scripts/vg`):

```bash
orq doctor
orq doctor --format json
```

La política actual es:

- Toda asignación debe usar par exacto `agente/modelo`.
- No se asigna un modelo marcado como `verified=false` hasta probarlo en una tarea real pequeña.
- NVIDIA API `openai/gpt-oss-20b` y `openai/gpt-oss-120b` quedaron validados con `/home/freddy/.config/orq/providers.env` para pruebas baratas sin gastar OpenAI de Pi.
- AGY/open-model `gpt-oss-120b-medium` queda validado para prompts baratos, resúmenes y tareas mecánicas.
- AGY/Google `gemini-3.5-flash-low` queda validado para tareas mecánicas, clasificación y validaciones baratas.
- Pi/OpenAI `cheap-or-fast` para tareas mecánicas/documentales cuando esté disponible.
- Pi/OpenAI `gpt-5.5` para orquestación principal y síntesis, pero se debe ahorrar cuando sea posible.
- Pi/NVIDIA o AGY/NVIDIA quedan registrados como candidatos baratos, pero deben validarse antes de usarse.
- AGY/Gemini Flash High para código y análisis técnico medio cuando se valide el modelo exacto disponible.
- AGY/Gemini Pro para análisis más fuerte si se valida disponibilidad.
- Claude/Sonnet como revisión crítica, seguridad o bloqueo.
- Claude/Opus solo para arquitectura compleja o decisión mayor.

### Revisar presupuesto de contexto/cuota

Cuando Pi muestre porcentajes altos en el footer, usa esos números para pedir una recomendación mecánica:

```bash
orq budget --context-percent 70 --codex-5h-percent 20 --weekly-percent 16
```

Acciones posibles:

- `continuar`: seguir, pero priorizando modelos baratos.
- `compactar_pronto`: preparar compactación.
- `compactar`: ejecutar `/compact` antes de seguir.
- `delegar_barato`: evitar Pi/OpenAI y usar NVIDIA/AGY.
- `pausar`: esperar reset del límite corto Codex.

Si la acción incluye compactación, `orq` imprime un prompt `/compact` listo para copiar.

### Delegación autónoma hacia AGY CLI (`orq delegate`)

Genera un plan de delegación y el comando CLI autónomo listo para ejecutar en AGY (Antigravity CLI), evitando prompts interactivos innecesarios y garantizando aislamiento de contexto:

```bash
orq delegate --handoff /home/freddy/Workspace/.agents/handoffs/tarea.md
orq delegate "implementar nuevo endpoint" --agent agy
orq delegate --handoff /home/freddy/Workspace/.agents/handoffs/tarea.md --format json
```

Propiedades del comando generado para AGY:
- **Aislamiento de contexto:** Incluye `"Olvida el historial anterior. Lee y ejecuta <handoff>"` para evitar arrastre de contexto o bloqueos de turnos previos.
- **Permisos no interactivos por sesión:** Emite `--dangerously-skip-permissions` a nivel de invocación CLI para comandos seguros autorizados por el handoff, sin persistir permisos globales en `settings.json`.
- **Rutas acotadas:** Limita `--add-dir` al repositorio de trabajo y al directorio `.agents`.
- **Compatibilidad de modelos:** No emite `--effort` por defecto para evitar conflictos con modelos fijos como `gemini-3.7-flash-high` o `gpt-oss-120b-medium`.
- **Ejecución obligatoria con wrapper:** Prefija la invocación con `rtk` (`rtk agy`).

### Telemetría hacia SGE Observer LLM

`orq` puede enviar eventos al Observer usando el endpoint existente `POST /api/events/ingest`.

Configuración local segura:

```bash
export ORQ_OBSERVER_URL="http://127.0.0.1:4000"
export ORQ_OBSERVER_HOST_TOKEN_FILE="$HOME/.config/sge-observer/agent-orchestrator.host-token"
```

Prueba sintética:

```bash
orq observer send-test --project agent-orchestrator --agent nvidia-api --model openai/gpt-oss-20b
```

Registro automático desde el ledger:

```bash
orq record --task "validar Observer" --agent nvidia-api --model openai/gpt-oss-20b --status done
```

Si el token está configurado, `orq record` guarda el ledger local y además envía un evento `orq_record` no bloqueante al Observer. Si Observer no está disponible o no hay token, el ledger local sigue funcionando.

El token no se guarda en git. `orq observer send-test` falla de forma clara si no hay token; `orq record` no falla por problemas de Observer.

### Validar estándar operativo de repos

```bash
orq repo check --path /ruta/al/repo
orq repo check --path /ruta/al/repo --format json
```

Crear archivos base faltantes en un repo nuevo o incompleto:

```bash
orq repo init-template --path /ruta/al/repo --name nombre-proyecto
```

El comando revisa presencia de archivos base del estándar: README, SECURITY, CONTRIBUTING, LICENSE, CI, docs, diagramas, Makefile y plantillas. Si falta algo obligatorio, devuelve error.

### Auditar PRs abiertos

Auditoría read-only de PRs abiertos. No aprueba, no mergea y no modifica el repo.

```bash
orq audit prs --path /ruta/al/repo
orq audit prs --path /ruta/al/repo --format json
```

Reporta checks, estado mergeable y bloqueos como review requerida por una identidad distinta con permisos.

### Auditar issues abiertos

Auditoría read-only de issues abiertos. No cierra, no etiqueta y no modifica el repo.

```bash
orq audit issues --path /ruta/al/repo
orq audit issues --path /ruta/al/repo --format json
```

Reporta acumulaciones que sugieren una auditoría arquitectónica.

### Auditar modelos disponibles

Reporte read-only para revisión manual/mensual de modelos, costo relativo y asignabilidad.

```bash
orq audit models
orq audit models --format json
```

Los modelos no verificados o `review_only` quedan marcados como `not_assignable`.

### Validar seguridad Tiger Style

```bash
orq safety check --path /ruta/al/repo
orq safety check --path /ruta/al/repo --command "go test ./..."
orq safety check --path /ruta/al/repo --format json
```

El comando es read-only. Detecta paths inseguros, comandos con tokens peligrosos y cambios sensibles como dependencias, migraciones SQL, secretos, auth o deploy.

### Generar revisión 4R

```bash
orq review 4r --path /ruta/al/repo
orq review 4r --path /ruta/al/repo --format json
```

El comando prepara preguntas de Legibilidad, Robustez, Riesgo y Seguridad usando los archivos cambiados como foco inicial.

### Ejecutar prueba integral local

La guía completa vive en [prueba-integral-orq.md](prueba-integral-orq.md).

```bash
orq guard-collision --path .
orq repo check --path .
orq safety check --path .
orq review 4r --path .
docker compose run --rm dev go test ./...
orq heartbeat run --workspace .
orq audit worktrees --path .
orq audit prs --path .
orq audit issues --path .
orq audit models
orq session validate --guard-collision OK --repo-check OK --safety-check OK --tests PASS --receipt OK --handoff OK
```

No marques una sesión como válida si falta el recibo RDD o si una validación no fue ejecutada.

### Registrar tareas para futuro dashboard móvil

```bash
orq task create "ordenar vault GLPI"
orq task list
orq task assign <id> --agent pi --model cheap-or-fast --host minipc
orq handoff draft --task-id <id>
orq handoff draft --task-id <id> --template reviewer-4r
orq handoff draft --task-id <id> --template security-reviewer
orq handoff draft --task-id <id> --template implementer
orq handoff draft --task-id <id> --template documenter
orq handoff draft --task-id <id> --template architect
orq task update <id> --state running
orq task update <id> --state done --evidence "PR o commit validado"
```

Por defecto las tareas se guardan en:

```txt
~/.local/state/orq/tasks.jsonl
```

Estos estados alimentarán el futuro dashboard/PWA por WireGuard.

Las plantillas cacheables de `handoff draft` ordenan el prompt así: `<contexto_estatico>` → `<contexto_estable>` → `<contexto_dinamico>` → `<tarea>`. Los datos volátiles van abajo para reducir costo y evitar invalidar caché.

Validar que no haya datos volátiles en bloques superiores:

```bash
orq handoff validate-template --file handoff.md
```

### Planificar ordenamiento documental del vault

Este comando **no mueve archivos**. Solo propone acciones para crear índices y detectar documentos sin prefijo numérico.

```bash
orq vault-order --vault /home/freddy/Workspace/Obsidian --query glpi
```

Para salida procesable por otra herramienta o agente:

```bash
orq vault-order --vault /home/freddy/Workspace/Obsidian --query glpi --format json
```

### Validar configuración

Si estás parado dentro del repo:

```bash
cd /home/freddy/Workspace/Desarrollo/agent-orchestrator
orq config --config examples/config.example.toml --check-adapters --format json
```

Desde cualquier carpeta usa ruta absoluta:

```bash
orq config --config /home/freddy/Workspace/Desarrollo/agent-orchestrator/examples/config.example.toml --check-adapters --format json
```

## Estado actual del MVP

`orq` todavía está en modo asesor:

- clasifica tareas;
- recomienda agente/modelo;
- registra eventos;
- registra tareas con estado verificable;
- valida estructura mínima de repos con `repo check`;
- envía pruebas de telemetría a SGE Observer LLM;
- valida guardias básicas;
- carga configuración y adapters;
- genera planes de ordenamiento documental con `vault-order`;
- **no ejecuta agentes automáticamente todavía**.

Eso es intencional: primero se acumula evidencia verificable antes de automatizar ejecución real.
