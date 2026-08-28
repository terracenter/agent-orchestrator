package repostandard

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

type InitResult struct {
	Root    string   `json:"root"`
	Created []string `json:"created"`
	Skipped []string `json:"skipped"`
}

type TemplateData struct {
	ProjectName string
}

func InitRepo(root string, data TemplateData) (InitResult, error) {
	if root == "" {
		root = "."
	}
	if strings.TrimSpace(data.ProjectName) == "" {
		data.ProjectName = filepath.Base(root)
	}
	result := InitResult{Root: root}
	files := map[string]string{
		"README.md":                         readme(data.ProjectName),
		"README.en.md":                      readmeEN(data.ProjectName),
		"SECURITY.md":                       securityDoc(),
		"CONTRIBUTING.md":                   contributingDoc(),
		"RELEASES.md":                       releasesDoc(),
		"LICENSE":                           "Definir licencia antes de publicar.\n",
		".env.example":                      "# Variables locales sin secretos reales\n",
		".gitignore":                        ".env\n*.log\nbin/\n",
		".github/pull_request_template.md":  prTemplate(),
		".github/ISSUE_TEMPLATE/feature.md": featureTemplate(),
		".github/workflows/ci.yml":          ciWorkflow(),
		"docs/diagramas/README.md":          "# Diagramas\n\nUsar diagram-design. No Mermaid/draw.io como formato objetivo.\n",
		"Makefile":                          makefile(),
	}
	for path, content := range files {
		full := filepath.Join(root, path)
		if _, err := os.Stat(full); err == nil {
			result.Skipped = append(result.Skipped, path)
			continue
		} else if !os.IsNotExist(err) {
			return result, err
		}
		if err := os.MkdirAll(filepath.Dir(full), 0o755); err != nil {
			return result, err
		}
		if err := os.WriteFile(full, []byte(content), 0o644); err != nil {
			return result, err
		}
		result.Created = append(result.Created, path)
	}
	return result, nil
}

func readme(project string) string {
	return fmt.Sprintf(`# %s

> [!IMPORTANT]
> Estado actual: completar antes de publicar.

Descripción breve en Español Venezuela: qué resuelve, para quién y por qué existe.

## Arquitectura

| Capa | Tecnología | Motivo |
|---|---|---|
| Backend | Go + Chi/net/http | Nativo, simple, auditable |
| UI | Go Templates + HTMX/Alpine + Tailwind | Sin SPA pesada en producción |
| Deploy | Binario Go / contenedor mínimo | Menor superficie y rollback simple |

## Seguridad

> [!WARNING]
> No commitear secretos, tokens, .env reales ni dumps.

## Desarrollo local

~~~bash
make dev
~~~

## Validación

~~~bash
make test
make security
make build
~~~
`, project)
}

func readmeEN(project string) string {
	return fmt.Sprintf("# %s\n\nShort description in American English.\n", project)
}
func securityDoc() string {
	return "# Seguridad\n\nLa seguridad tiene prioridad sobre features, rendimiento y visual.\n"
}
func contributingDoc() string {
	return "# Contribución\n\nUsar Español Venezuela en documentación principal y checklist 4R en PRs.\n"
}
func releasesDoc() string {
	return "# Historial de Cambios\n\n## 0.1.0 — YYYY-MM-DD\n\n### Agregado\n### Cambiado\n### Corregido\n### Seguridad\n### Operación\n### Compatibilidad\n### Rollback\n"
}
func prTemplate() string {
	return "## Resumen\n\n## Validación\n\n## Revisión 4R\n\n- [ ] Legibilidad\n- [ ] Robustez\n- [ ] Riesgo\n- [ ] Seguridad\n"
}
func featureTemplate() string {
	return "---\nname: Feature\ndescription: Mejora funcional acotada.\n---\n\n## Objetivo\n\n## Seguridad\n\n## Criterios de aceptación\n"
}
func makefile() string {
	return ".PHONY: dev test security build\n\ndev:\n\tgo run ./cmd/server\n\ntest:\n\tgo test ./...\n\tgo vet ./...\n\nsecurity:\n\tgovulncheck ./...\n\tgosec ./...\n\nbuild:\n\tCGO_ENABLED=0 go build ./...\n"
}
func ciWorkflow() string {
	return "name: CI\n\non:\n  pull_request:\n  push:\n    branches: [main]\n\njobs:\n  validate-and-test:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@v4\n      - uses: actions/setup-go@v5\n        with:\n          go-version-file: go.mod\n      - run: go test ./...\n      - run: go vet ./...\n      - run: CGO_ENABLED=0 go build ./...\n"
}
