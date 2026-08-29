# Inspiración técnica desde Orca

Fuente auditada: <https://github.com/stablyai/orca>.

Orca es MIT y resuelve varias piezas que `agent-orchestrator` necesita, especialmente para operar flotas de agentes en paralelo. No vamos a copiar código sin revisión, pero sí adoptaremos patrones compatibles con nuestro stack local-first.

## Patrones que sí vamos a tomar

### 1. Worktrees como unidad de aislamiento

Orca promueve ejecutar agentes en worktrees separados. En `orq`, esto se traduce en:

- una tarea mutante debe tener dueño;
- cada agente trabaja en su branch/worktree;
- antes de tocar un repo compartido se ejecuta guardia de colisión;
- nunca se asume que una sesión paralela terminó si no hay evidencia en git/ledger.

Comando existente relacionado:

```bash
orq guard-collision --path /home/freddy/Workspace/Obsidian
```

### 2. CLI como superficie estable para agentes

Orca documenta que los agentes deben operar por CLI con salida JSON cuando sea posible. En `orq`:

- cada comando nuevo debe tener `--format json` si será consumido por otro agente;
- los errores deben ser explícitos y con exit code útil;
- la documentación debe evitar depender del chat como fuente de verdad.

Comandos existentes relacionados:

```bash
orq route "ordenar vault GLPI" --format json
orq delegate "ordenar vault GLPI" --format json
orq vault-order --vault /home/freddy/Workspace/Obsidian --query glpi --format json
```

### 3. Separar handoff simple de orquestación supervisada

Orca diferencia entre handoff de propiedad completa y coordinación supervisada. En `orq` usaremos dos modos:

- `delegate`: genera prompt/handoff para que otro agente tome la tarea completa;
- `supervise` futuro: crea subtareas, espera estados, recoge resultados y decide merge/escalación.

### 4. Estados verificables de workers

Orca maneja estados de agentes/workers. En `orq` debemos registrar estados simples y auditables:

```txt
planned -> assigned -> running -> blocked -> done -> verified -> merged
```

Nada se considera terminado solo por texto en chat: debe existir evidencia en git, ledger, CI, o salida de comando.

### 5. Evitar colisiones entre sesiones

Orca usa worktrees para evitar que agentes pisen el mismo checkout. En `orq`, esto queda como regla de producto:

- si hay cambios sin commitear: stop;
- si hay más de un worktree activo: pedir confirmación o asignar worktree nuevo;
- si el destino es el vault, primero consultar `vg` y luego revisar git.

### 6. Guías versionadas por el binario

Orca sirve guías desde el CLI para evitar documentación desfasada. En `orq` esto se implementa con:

```bash
orq docs usage
orq docs orchestration
```

para imprimir guías versionadas del propio binario.

## Patrones que NO vamos a tomar tal cual

- UI Electron completa: fuera del MVP actual.
- Mobile companion: útil a futuro, no para fase 1.
- Control de navegador/computer-use: fuera del objetivo inmediato.
- Dependencia de runtime Orca: `orq` debe seguir siendo local-first y usable sin Orca.

## Roadmap mínimo inspirado en Orca

1. `orq delegate` genera prompt barato y seguro. **Hecho.**
2. `orq guard-collision` bloquea colisiones. **Hecho.**
3. `orq task create/list/status` registra tareas con estado verificable.
4. `orq handoff create` escribe handoffs en archivo, no en chat.
5. `orq worker start/status/done` modela agentes baratos/caros con presupuesto.
6. `orq supervise` coordina varias subtareas y espera evidencia antes de cerrar.
7. `orq docs` imprime documentación versionada desde el binario.

## Regla de diseño

Todo avance inspirado en Orca debe preservar estas prioridades:

1. costo mínimo suficiente;
2. no colisionar sesiones;
3. evidencia antes que opinión;
4. dry-run antes de mutación;
5. adapters opcionales para Obsidian, vg, rtk, Engram y otros agentes.
