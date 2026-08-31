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

Para validaciones críticas de deploy/producción, CI/CD o posibles falsos positivos del asistente, el orquestador debe priorizar Opus como segundo par de ojos antes de actuar:

```bash
orq route "validar posible falso positivo en Deploy CWP falla: SSH exec request failed"
```

Salida esperada:

```txt
agent=claude-code model=opus level=4 category=revision_critica reason=revision critica de produccion/deploy/CI o posible falso positivo: priorizar Opus como validador experto antes de actuar
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

Registra el resultado de una invocación de agente de forma auditable:

```bash
orq record --task test --agent pi --model gpt-5.5 --status ok
```

Con soporte completo de telemetría (timestamps RFC3339, duración calculada o explícita, modelo de fallback y conteo de tokens):

```bash
orq record \
  --task "refactorizar modulo auth" \
  --agent agy \
  --model gemini-3.7-flash-high \
  --status ok \
  --started-at "2026-08-30T10:00:00Z" \
  --finished-at "2026-08-30T10:00:15Z" \
  --fallback-agent pi \
  --fallback-model gpt-5.5 \
  --tokens-in 1200 \
  --tokens-out 450 \
  --notes "completado exitosamente"
```

> [!NOTE]
> **Política de consumo Claude CLI (`claude-code`)**: Dado que la CLI de Claude no expone conteo estructurado de tokens por invocación, su consumo se registra como `unknown` (`tokens_in = 0, tokens_out = 0` y nota de no-medido). Queda prohibido inventar o estimar tokens para Claude CLI. La métrica cuantificable en host es su **duración (`duration_ms`)**, **status** y **código de salida**.

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
- Claude/Sonnet como revisión crítica general, seguridad o bloqueo.
- Claude/Opus tiene prioridad para `revision_critica`: deploy/producción, workflows CI/CD, diagnósticos dudosos, posibles falsos positivos del asistente, incidentes y decisiones donde actuar con baja certeza pueda romper producción.

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

Genera un plan de delegación y el comando CLI autónomo listo para ejecutar en AGY (Antigravity CLI), evitando prompts interactivos innecesarios y garantizando aislamiento de contexto. Además permite materializar el handoff y un receipt inicial en archivos:

```bash
orq delegate --handoff /home/freddy/Workspace/.agents/handoffs/tarea.md
orq delegate "implementar nuevo endpoint" --agent agy
orq delegate "implementar nuevo endpoint" --agent agy --write-handoff /home/freddy/Workspace/.agents/handoffs/tarea.md --write-receipt /tmp/receipt.json
orq delegate --handoff /home/freddy/Workspace/.agents/handoffs/tarea.md --format json
```

Opciones de escritura de archivos:
- `--write-handoff <path>`: Genera y escribe el archivo Markdown de handoff listo para el agente ejecutor, incluyendo objetivo, protocolo operativo (RTK, sin sudo, sin secretos, sin push a main), instrucción de contexto limpio (`Olvida el historial anterior`) y criterios de validación.
- `--write-receipt <path>`: Genera y escribe un archivo JSON de receipt/plan de ejecución inicial compatible con `orq receipt`.
- `--force`: Permite sobrescribir archivos existentes en las rutas indicadas (por defecto, `orq delegate` rechaza sobrescribir si el archivo ya existe).

Propiedades del comando generado para AGY:
- **Aislamiento de contexto:** Incluye `"Olvida el historial anterior. Lee y ejecuta <handoff>"` para evitar arrastre de contexto o bloqueos de turnos previos.
- **Permisos no interactivos por sesión:** Emite `--dangerously-skip-permissions` a nivel de invocación CLI para comandos seguros autorizados por el handoff, sin persistir permisos globales en `settings.json`.
- **Rutas acotadas:** Limita `--add-dir` al repositorio de trabajo y al directorio `.agents`.
- **Compatibilidad de modelos:** No emite `--effort` por defecto para evitar conflictos con modelos fijos como `gemini-3.7-flash-high` o `gpt-oss-120b-medium`.
- **Ejecución obligatoria con wrapper:** Prefija la invocación con `rtk` (`rtk agy`).

### Telemetría hacia SGE Observer LLM

`orq` puede enviar eventos al Observer usando el endpoint existente `POST /api/events/ingest`.

Configuración persistente segura:

```bash
mkdir -p "$HOME/.config/sge-observer"
chmod 700 "$HOME/.config/sge-observer"
install -m 0600 /dev/null "$HOME/.config/sge-observer/client.env"
```

Contenido esperado de `~/.config/sge-observer/client.env`:

```bash
ORQ_OBSERVER_URL="http://127.0.0.1:4000"
ORQ_OBSERVER_HOST_TOKEN_FILE="$HOME/.config/sge-observer/agent-orchestrator.host-token"
```

También se aceptan variables de entorno para uso temporal:

```bash
export ORQ_OBSERVER_URL="http://127.0.0.1:4000"
export ORQ_OBSERVER_HOST_TOKEN_FILE="$HOME/.config/sge-observer/agent-orchestrator.host-token"
```

Nunca guardes el token en git. Prefiere `ORQ_OBSERVER_HOST_TOKEN_FILE` sobre `ORQ_OBSERVER_HOST_TOKEN`.

Diagnóstico sin exponer secretos:

```bash
orq observer status
orq observer status --format json
```

Prueba sintética:

```bash
orq observer send-test --project agent-orchestrator --agent nvidia-api --model openai/gpt-oss-20b
```

Snapshot manual de capacidad/cuota para Observer LLM:

```bash
orq observer send-capacity --agent claude-code --provider-group anthropic --model-group haiku --remaining-percent 80 --window daily
```

Usar capacidad agregada para ajustar routing no crítico desde un archivo JSON explícito:

```bash
orq route --capacity-file /ruta/capacity.json "tarea mecánica simple"
```

El archivo debe contener un arreglo de snapshots agregados con campos como `agent`, `provider_group`, `model_group`, `remaining_percent`, `window`, `source` y `captured_at`. Si una decisión tiene `security_override=true`, la seguridad prevalece sobre costo/cuota.

Registro automático desde el ledger:

```bash
orq record --task "validar Observer" --agent nvidia-api --model openai/gpt-oss-20b --status done
```

Si el token está configurado, `orq record` guarda el ledger local y además envía un evento `orq_record` no bloqueante al Observer. Si Observer no está disponible o no hay token, el ledger local sigue funcionando.

Sincronización del ledger histórico:

```bash
orq observer sync --dry-run
orq observer sync
orq observer sync --format json
```

Verificación local de que el último evento de un agente quedó marcado como sincronizado:

```bash
orq observer verify-last --agent claude-code
orq observer verify-last --agent claude-code --format json
```

Por defecto, `orq observer sync` lee `~/.local/state/orq/ledger.jsonl` y guarda el estado de deduplicación en `~/.local/state/orq/observer-sync.json`. Ambos paths pueden cambiarse con `--ledger` y `--state`.

El token no se guarda en git. `orq observer send-test`, `orq observer send-capacity` y `orq observer sync` fallan de forma clara si no hay token; `orq record` no falla por problemas de Observer.

Limitación actual: `orq record` y `orq observer sync` reportan eventos de delegación/ledger, pero no capturan automáticamente tokens ni costo real del Claude CLI. Para validaciones críticas se debe registrar el modelo Anthropic exacto cuando se conozca; el router usa `claude-opus-4-1-20250805` para `revision_critica`.

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

### Auditar sesión de trace

Audita una sesión creada con `orq trace` y emite findings con códigos estables y severidad.

```bash
orq audit session --session-id <id>
orq audit session --session-id <id> --format json
orq audit session --file ~/.local/state/orq/traces/<id>.session.json
```

Detecta comandos sin `rtk` cuando aplica `rtk_required=true`, ejecución directa en agente caro/supervisor y mutaciones destructivas sin `--dry-run` o confirmación humana.

### Generar borrador de issue desde auditoría

Genera un borrador revisable; no crea issues remotos automáticamente.

```bash
orq audit issue-from-session --session-id <id>
orq audit issue-from-session --session-id <id> --format json
```

El borrador incluye comportamiento esperado, comportamiento actual, evidencia, criterios de aceptación y checklist de revisión humana para cambios de guardrails.

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
