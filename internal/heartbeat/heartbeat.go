package heartbeat

import (
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"
)

// Report summarizes a safe, read-only security intelligence heartbeat.
type Report struct {
	Workspace   string       `json:"workspace"`
	GeneratedAt time.Time    `json:"generated_at"`
	Projects    []Project    `json:"projects"`
	Sources     []Source     `json:"sources"`
	Actions     []ActionItem `json:"actions"`
}

// Project is a repository or project detected in the workspace.
type Project struct {
	Path      string   `json:"path"`
	Name      string   `json:"name"`
	Manifests []string `json:"manifests"`
}

// Source is an intelligence source to review outside automated prod changes.
type Source struct {
	Name string `json:"name"`
	URL  string `json:"url"`
}

// ActionItem is a deterministic next step emitted by the heartbeat.
type ActionItem struct {
	Priority string `json:"priority"`
	Text     string `json:"text"`
}

var manifestNames = map[string]bool{
	"go.mod":             true,
	"package.json":       true,
	"pnpm-lock.yaml":     true,
	"package-lock.json":  true,
	"Cargo.toml":         true,
	"requirements.txt":   true,
	"pyproject.toml":     true,
	"Dockerfile":         true,
	"docker-compose.yml": true,
}

// Run scans the workspace without network or writes and returns security review targets.
func Run(workspace string) (Report, error) {
	report := Report{Workspace: workspace, GeneratedAt: time.Now().UTC(), Sources: DefaultSources()}
	projects := map[string]*Project{}
	err := filepath.WalkDir(workspace, func(path string, entry os.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if entry.IsDir() {
			name := entry.Name()
			if name == ".git" || name == "node_modules" || name == "vendor" || name == ".next" || name == "dist" || name == "build" {
				return filepath.SkipDir
			}
			return nil
		}
		if !manifestNames[entry.Name()] {
			return nil
		}
		root := filepath.Dir(path)
		project := projects[root]
		if project == nil {
			project = &Project{Path: root, Name: filepath.Base(root)}
			projects[root] = project
		}
		project.Manifests = append(project.Manifests, entry.Name())
		return nil
	})
	if err != nil {
		return Report{}, err
	}
	for _, project := range projects {
		sort.Strings(project.Manifests)
		report.Projects = append(report.Projects, *project)
	}
	sort.Slice(report.Projects, func(i, j int) bool { return report.Projects[i].Path < report.Projects[j].Path })
	report.Actions = buildActions(report.Projects)
	return report, nil
}

// DefaultSources returns public sources that should be reviewed by a human/agent.
func DefaultSources() []Source {
	return []Source{
		{Name: "GitHub Security Advisories", URL: "https://github.com/advisories"},
		{Name: "NVD CVE feed", URL: "https://nvd.nist.gov/vuln"},
		{Name: "Go vulnerability database", URL: "https://pkg.go.dev/vuln"},
		{Name: "Docker security announcements", URL: "https://docs.docker.com/security/security-announcements/"},
	}
}

func buildActions(projects []Project) []ActionItem {
	actions := []ActionItem{{Priority: "alta", Text: "revisar advisories/CVE contra los manifiestos detectados antes de actualizar dependencias"}}
	for _, project := range projects {
		joined := strings.Join(project.Manifests, ",")
		if strings.Contains(joined, "go.mod") {
			actions = append(actions, ActionItem{Priority: "media", Text: "ejecutar revisión Go segura en " + project.Path + " usando contenedor/CI, no host directo"})
		}
		if strings.Contains(joined, "package.json") {
			actions = append(actions, ActionItem{Priority: "media", Text: "revisar dependencias npm en " + project.Path + " sin aplicar cambios automáticos en producción"})
		}
	}
	return actions
}
