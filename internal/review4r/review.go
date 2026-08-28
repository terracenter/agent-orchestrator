package review4r

import (
	"bytes"
	"os/exec"
	"path/filepath"
	"strings"
)

type Item struct {
	Area     string   `json:"area"`
	Question string   `json:"question"`
	Focus    []string `json:"focus"`
}

type Report struct {
	Root         string   `json:"root"`
	ChangedFiles []string `json:"changed_files"`
	Items        []Item   `json:"items"`
}

func Build(root string) Report {
	if root == "" {
		root = "."
	}
	files := changedFiles(root)
	return Report{Root: root, ChangedFiles: files, Items: []Item{
		{Area: "Legibilidad", Question: "¿El cambio se entiende sin explicación externa?", Focus: focus(files, "docs", "nombres", "estructura")},
		{Area: "Robustez", Question: "¿Falla de forma controlada y tiene pruebas suficientes?", Focus: focus(files, "errores", "tests", "edge cases")},
		{Area: "Riesgo", Question: "¿Qué puede romper y cómo se revierte?", Focus: focus(files, "compatibilidad", "migraciones", "rollback")},
		{Area: "Seguridad", Question: "¿Aumenta superficie o toca secretos/datos/permisos?", Focus: focus(files, "auth", "tokens", "inputs", "logs")},
	}}
}

func changedFiles(root string) []string {
	cmd := exec.Command("git", "diff", "--name-only", "HEAD")
	cmd.Dir = root
	out, err := cmd.Output()
	if err != nil {
		return nil
	}
	lines := bytes.Split(bytes.TrimSpace(out), []byte("\n"))
	var files []string
	for _, line := range lines {
		if len(line) > 0 {
			files = append(files, string(line))
		}
	}
	return files
}

func focus(files []string, fallback ...string) []string {
	seen := map[string]bool{}
	add := func(v string) {
		if v != "" {
			seen[v] = true
		}
	}
	for _, file := range files {
		ext := strings.TrimPrefix(filepath.Ext(file), ".")
		add(ext)
		parts := strings.Split(file, string(filepath.Separator))
		if len(parts) > 0 {
			add(parts[0])
		}
	}
	if len(seen) == 0 {
		for _, v := range fallback {
			add(v)
		}
	}
	var out []string
	for k := range seen {
		out = append(out, k)
	}
	return out
}
