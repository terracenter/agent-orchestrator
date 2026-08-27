package vaultorder

import (
	"io/fs"
	"os"
	"path/filepath"
	"sort"
	"strings"
)

type Plan struct {
	Query       string      `json:"query"`
	Vault       string      `json:"vault"`
	Matches     []Match     `json:"matches"`
	Directories []Directory `json:"directories"`
	Actions     []Action    `json:"actions"`
}

type Match struct {
	Path string `json:"path"`
}

type Directory struct {
	Path     string   `json:"path"`
	HasIndex bool     `json:"has_index"`
	Files    []string `json:"files"`
}

type Action struct {
	Type   string `json:"type"`
	Path   string `json:"path"`
	Reason string `json:"reason"`
}

func Build(vault string, query string) (Plan, error) {
	query = strings.ToLower(strings.TrimSpace(query))
	plan := Plan{Query: query, Vault: vault}
	dirs := map[string]*Directory{}
	err := filepath.WalkDir(vault, func(path string, d fs.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if d.IsDir() {
			name := d.Name()
			if name == ".git" || name == ".obsidian" || name == "node_modules" {
				return filepath.SkipDir
			}
			return nil
		}
		if filepath.Ext(path) != ".md" {
			return nil
		}
		data, err := os.ReadFile(path)
		if err != nil {
			return err
		}
		rel, err := filepath.Rel(vault, path)
		if err != nil {
			return err
		}
		rel = filepath.ToSlash(rel)
		text := strings.ToLower(rel + "\n" + string(data))
		if query != "" && !strings.Contains(text, query) {
			return nil
		}
		plan.Matches = append(plan.Matches, Match{Path: rel})
		dirPath := filepath.ToSlash(filepath.Dir(rel))
		dir := dirs[dirPath]
		if dir == nil {
			dir = &Directory{Path: dirPath}
			dirs[dirPath] = dir
		}
		base := filepath.Base(rel)
		if isIndex(base) {
			dir.HasIndex = true
		}
		dir.Files = append(dir.Files, base)
		return nil
	})
	if err != nil {
		return Plan{}, err
	}
	for _, dir := range dirs {
		sort.Strings(dir.Files)
		plan.Directories = append(plan.Directories, *dir)
		if !dir.HasIndex {
			plan.Actions = append(plan.Actions, Action{Type: "create-index", Path: dir.Path + "/00-index.md", Reason: "carpeta con notas relacionadas sin indice local"})
		}
		for _, file := range dir.Files {
			if isIndex(file) || numbered(file) {
				continue
			}
			plan.Actions = append(plan.Actions, Action{Type: "consider-rename", Path: dir.Path + "/" + file, Reason: "documento relacionado sin prefijo numerico"})
		}
	}
	sort.Slice(plan.Matches, func(i, j int) bool { return plan.Matches[i].Path < plan.Matches[j].Path })
	sort.Slice(plan.Directories, func(i, j int) bool { return plan.Directories[i].Path < plan.Directories[j].Path })
	sort.Slice(plan.Actions, func(i, j int) bool { return plan.Actions[i].Path < plan.Actions[j].Path })
	return plan, nil
}

func isIndex(name string) bool {
	return name == "00-index.md" || name == "00_index.md" || name == "00.index.md" || name == "index.md" || name == "README.md"
}

func numbered(name string) bool {
	if len(name) < 4 {
		return false
	}
	prefix := name[0] >= '0' && name[0] <= '9' && name[1] >= '0' && name[1] <= '9'
	separator := name[2] == '-' || name[2] == '.' || name[2] == '_'
	return prefix && separator
}
