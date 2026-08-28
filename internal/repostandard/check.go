package repostandard

import (
	"os"
	"path/filepath"
)

type Check struct {
	Name     string `json:"name"`
	Path     string `json:"path"`
	Required bool   `json:"required"`
	Passed   bool   `json:"passed"`
	Reason   string `json:"reason"`
}

type Report struct {
	Root   string  `json:"root"`
	Passed bool    `json:"passed"`
	Checks []Check `json:"checks"`
}

type rule struct {
	name     string
	path     string
	required bool
}

var baseRules = []rule{
	{name: "README principal ES-VE", path: "README.md", required: true},
	{name: "README secundario EN-US", path: "README.en.md", required: false},
	{name: "seguridad", path: "SECURITY.md", required: true},
	{name: "contribucion", path: "CONTRIBUTING.md", required: true},
	{name: "licencia", path: "LICENSE", required: true},
	{name: "env example", path: ".env.example", required: true},
	{name: "gitignore", path: ".gitignore", required: true},
	{name: "template PR 4R", path: ".github/pull_request_template.md", required: true},
	{name: "CI", path: ".github/workflows", required: true},
	{name: "docs", path: "docs", required: true},
	{name: "diagramas", path: "docs/diagramas", required: true},
	{name: "Makefile", path: "Makefile", required: true},
	{name: "releases/changelog", path: "RELEASES.md", required: false},
}

func CheckRepo(root string) Report {
	if root == "" {
		root = "."
	}
	report := Report{Root: root, Passed: true}
	for _, r := range baseRules {
		path := filepath.Join(root, r.path)
		_, err := os.Stat(path)
		check := Check{Name: r.name, Path: r.path, Required: r.required, Passed: err == nil}
		if err != nil {
			if os.IsNotExist(err) {
				check.Reason = "no existe"
			} else {
				check.Reason = err.Error()
			}
			if r.required {
				report.Passed = false
			}
		} else {
			check.Reason = "ok"
		}
		report.Checks = append(report.Checks, check)
	}
	return report
}
