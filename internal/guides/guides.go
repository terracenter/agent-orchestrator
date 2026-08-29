package guides

import "fmt"

var texts = map[string]string{
	"usage": `# orq usage

Comandos base:

- orq route <task> [--format json]
- orq doctor [--format json]
- orq agents detect [--format json]
- orq task list [--format json]
- orq handoff draft --task-id <id> --template reviewer-4r
- orq receipt create --task text --command text --evidence text --rollback text
- orq audit prs|issues|models|worktrees
- orq safety check --path .

Reglas operativas:

- orq es la autoridad operacional.
- Usar rtk para shell, git y filesystem.
- Usar vg para vault/grafo.
- No usar --dangerously-skip-permissions.
- No tocar produccion, secretos, DB, DNS ni firewall sin aprobacion humana explicita.
`,
	"orchestration": `# orq orchestration

Flujo recomendado:

1. Verificar estado: orq guard-collision --path .
2. Evaluar ruta: orq route "<tarea>" --format json
3. Crear o asignar tarea: orq task create/assign/update
4. Delegar con handoff cacheable cuando convenga.
5. Validar con tests, repo check, safety check y review 4R.
6. Crear recibo RDD cuando aplique.
7. Actualizar Engram al cerrar PRs o decisiones importantes.

Seguridad:

- Las acciones peligrosas requieren confirmacion y aprobacion humana.
- Los modelos review_only no son asignables para ejecucion.
- Preferir modelos baratos cuando la tarea no requiere escalamiento.
`,
}

func Names() []string {
	return []string{"usage", "orchestration"}
}

func Text(name string) (string, error) {
	text, ok := texts[name]
	if !ok {
		return "", fmt.Errorf("unknown docs guide %q", name)
	}
	return text, nil
}
