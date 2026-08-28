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

### Enviar prueba de telemetría a SGE Observer LLM

`orq` puede enviar un evento sintético al Observer usando el endpoint existente `POST /api/events/ingest`.

Configuración local segura:

```bash
export ORQ_OBSERVER_URL="http://127.0.0.1:4000"
export ORQ_OBSERVER_HOST_TOKEN_FILE="$HOME/.config/sge-observer/agent-orchestrator.host-token"
```

Prueba:

```bash
orq observer send-test --project agent-orchestrator --agent nvidia-api --model openai/gpt-oss-20b
```

El token no se guarda en git. Si no está configurado, `orq` debe fallar de forma clara y continuar permitiendo el resto del flujo.

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

### Registrar tareas para futuro dashboard móvil

```bash
orq task create "ordenar vault GLPI"
orq task list
orq task assign <id> --agent pi --model cheap-or-fast --host minipc
orq handoff draft --task-id <id>
orq task update <id> --state running
orq task update <id> --state done --evidence "PR o commit validado"
```

Por defecto las tareas se guardan en:

```txt
~/.local/state/orq/tasks.jsonl
```

Estos estados alimentarán el futuro dashboard/PWA por WireGuard.

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
