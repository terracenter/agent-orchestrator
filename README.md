# agent-orchestrator

> Orquestador local-first de agentes/modelos: clasifica tareas, recomienda el modelo/agente más barato y seguro, y registra evidencia verificable antes de automatizar ejecución.

**Idioma:** Español de Venezuela principal. [English version](README.en.md).

**Licencia:** GNU AGPL-3.0-or-later. Si ejecutas una versión modificada como servicio de red, debes ofrecer el código fuente correspondiente según la licencia.

## Filosofía

Este proyecto no busca reinventar la rueda. Reutiliza ideas y patrones buenos del ecosistema AI coding —especialmente Engram, Gentle-AI, Gentleman Guardian Angel y sistemas de skills— cuando encajan con el objetivo del proyecto.

Principios:

- **Obsidian puede ser la SSoT humana**, pero el proyecto también funciona sin Obsidian.
- **Kuzu/vg puede ser el graph layer documental**, pero es un adapter opcional.
- **Engram puede ser memoria operativa cross-session**, pero no reemplaza la fuente de verdad.
- **rtk puede envolver comandos para reducir ruido**, pero terceros pueden usar ejecución estándar.
- **Hechos verificados > opiniones de modelos.**
- **Dry-run primero.** La ejecución automática no es parte del MVP inicial.

## Estado

MVP actual: **ledger + modo asesor + telemetría Observer + routing no crítico asistido por capacidad**.

Roadmap vivo: [ROADMAP.md](ROADMAP.md).

Guía de uso: [docs/uso.md](docs/uso.md).

Prueba integral del arnés: [docs/prueba-integral-orq.md](docs/prueba-integral-orq.md).

Inspiración técnica Orca: [docs/orca-inspiracion.md](docs/orca-inspiracion.md).

Comandos previstos/actuales:

```bash
orq classify "corregir una referencia rota"
orq route "rotar token de producción"
orq route --capacity-file /ruta/capacity.json "tarea mecánica simple"
orq record --task test --agent pi --model gpt-5.5 --status ok
orq status
orq run "auditar proyecto" --dry-run
orq guard --vault /ruta/al/vault --format json
orq guard-collision --path /home/freddy/Workspace/Obsidian
orq config --config examples/config.example.toml --check-adapters --format json
orq vault-order --vault /home/freddy/Workspace/Obsidian --query glpi --format json
orq delegate "ordenar información del vault relacionada con GLPI"
orq task create "ordenar vault GLPI"
orq task list
orq agents --format json
orq observer send-capacity --agent claude-code --provider-group anthropic --model-group haiku --remaining-percent 80 --window daily
```

## Instalación de desarrollo

Este repo usa Go y un entorno Docker para validar de forma reproducible.

```bash
docker compose run --rm dev go test ./...
docker compose run --rm dev go run ./cmd/orq --help
docker compose run --rm dev go run ./cmd/orq config --config examples/config.example.toml --check-adapters
```

## Instalación local del CLI

Para usar `orq` en sesiones futuras de Pi o después de `/reload`, instala el binario en `~/.local/bin`:

```bash
docker compose run --rm dev make build
mkdir -p ~/.local/bin
install -m 0755 bin/orq ~/.local/bin/orq
orq --help
```

Si estás desarrollando sin contenedor:

```bash
make install
```

## Idiomas del proyecto

- Documentación principal: Español de Venezuela.
- Documentación secundaria: Inglés americano.
- Los términos técnicos estándar se mantienen en inglés cuando corresponde.

## Seguridad del repositorio

`main` está protegido con un ruleset de GitHub: Pull Request obligatorio, bloqueo de force push/borrado y check `go-test` requerido.

Ver [SECURITY.md](SECURITY.md).

## Licencia

AGPLv3. El objetivo es que mejoras y derivados ofrecidos como servicio vuelvan a la comunidad.

## Inspiración y atribución

Este proyecto se inspira en ideas del ecosistema [Gentleman-Programming](https://github.com/Gentleman-Programming), especialmente:

- Engram — memoria persistente para agentes.
- Gentle-AI — workflows, SDD, routing por fases y ecosistema multi-agente.
- Gentleman Guardian Angel — contratos de revisión/veredicto y validación provider-agnostic.
- Gentleman-Skills — gobernanza y formato comunitario de skills.

Las ideas se evalúan y adaptan según necesidad. Cualquier código de terceros reutilizado de forma literal debe conservar su licencia y atribución.
