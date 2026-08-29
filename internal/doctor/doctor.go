package doctor

import (
	"context"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"time"
)

type Category string

const (
	CategoryCore    Category = "core"
	CategoryTooling Category = "tooling"
	CategoryAgent   Category = "agent"
)

type Status string

const (
	StatusOK       Status = "ok"
	StatusMissing  Status = "missing"
	StatusDegraded Status = "degraded"
	StatusBlocked  Status = "blocked"
)

type ToolCheck struct {
	Name           string   `json:"name"`
	Category       Category `json:"category"`
	Status         Status   `json:"status"`
	Path           string   `json:"path,omitempty"`
	ConfigPath     string   `json:"config_path,omitempty"`
	Version        string   `json:"version,omitempty"`
	Required       bool     `json:"required"`
	Note           string   `json:"note,omitempty"`
	Recommendation string   `json:"recommendation,omitempty"`
}

type Summary struct {
	Total    int `json:"total"`
	OK       int `json:"ok"`
	Missing  int `json:"missing"`
	Degraded int `json:"degraded"`
}

type Report struct {
	Status  Status      `json:"status"`
	Tools   []ToolCheck `json:"tools"`
	Summary Summary     `json:"summary"`
}

type Options struct {
	HomeDir string
}

func Run(ctx context.Context, opts Options) Report {
	home := opts.HomeDir
	if home == "" {
		h, err := os.UserHomeDir()
		if err == nil {
			home = h
		}
	}

	var checks []ToolCheck

	// 1. RTK (Core)
	checks = append(checks, checkRTK(ctx, home))

	// 2. Git (Core)
	checks = append(checks, checkGit(ctx, home))

	// 3. GitHub CLI (Core)
	checks = append(checks, checkGH(ctx, home))

	// 4. ORQ (Core)
	checks = append(checks, checkOrq(ctx, home))

	// 5. Vault Graph (Tooling)
	checks = append(checks, checkVG(ctx, home))

	// 6. OpenClaw (Agent)
	checks = append(checks, checkOpenClaw(ctx, home))

	// 7. AGY (Agent)
	checks = append(checks, checkAGY(ctx, home))

	// 8. Hermes (Agent)
	checks = append(checks, checkHermes(ctx, home))

	// 9. Claude Code (Agent)
	checks = append(checks, checkClaude(ctx, home))

	var total, okCount, missingCount, degradedCount int
	reportStatus := StatusOK
	hasRequiredMissing := false

	for _, c := range checks {
		total++
		switch c.Status {
		case StatusOK:
			okCount++
		case StatusMissing:
			missingCount++
			if c.Required {
				hasRequiredMissing = true
			}
		case StatusDegraded:
			degradedCount++
		}
	}

	if hasRequiredMissing {
		reportStatus = StatusBlocked
	} else if missingCount > 0 || degradedCount > 0 {
		reportStatus = StatusDegraded
	}

	return Report{
		Status: reportStatus,
		Tools:  checks,
		Summary: Summary{
			Total:    total,
			OK:       okCount,
			Missing:  missingCount,
			Degraded: degradedCount,
		},
	}
}

func isExecutable(path string) bool {
	fi, err := os.Stat(path)
	if err != nil || fi.IsDir() {
		return false
	}
	return fi.Mode()&0o111 != 0
}

func findExecutable(name string, extraDirs []string) string {
	for _, dir := range extraDirs {
		candidate := filepath.Join(dir, name)
		if isExecutable(candidate) {
			return candidate
		}
	}
	if p, err := exec.LookPath(name); err == nil && p != "" {
		return p
	}
	return ""
}

func checkDirectory(path string) string {
	if fi, err := os.Stat(path); err == nil && fi.IsDir() {
		return path
	}
	return ""
}

func runVersion(ctx context.Context, bin string, args ...string) string {
	ctxTimeout, cancel := context.WithTimeout(ctx, 2*time.Second)
	defer cancel()
	cmd := exec.CommandContext(ctxTimeout, bin, args...)
	out, err := cmd.Output()
	if err != nil {
		return ""
	}
	lines := strings.Split(strings.TrimSpace(string(out)), "\n")
	if len(lines) > 0 {
		return strings.TrimSpace(lines[0])
	}
	return ""
}

func checkRTK(ctx context.Context, home string) ToolCheck {
	extraDirs := []string{filepath.Join(home, ".local", "bin")}
	p := findExecutable("rtk", extraDirs)
	if p == "" {
		return ToolCheck{
			Name:           "rtk",
			Category:       CategoryCore,
			Status:         StatusMissing,
			Required:       true,
			Recommendation: "instalar rtk en ~/.local/bin o agregarlo al PATH",
		}
	}
	v := runVersion(ctx, p, "--version")
	return ToolCheck{
		Name:     "rtk",
		Category: CategoryCore,
		Status:   StatusOK,
		Path:     p,
		Version:  v,
		Required: true,
	}
}

func checkGit(ctx context.Context, home string) ToolCheck {
	p := findExecutable("git", []string{"/usr/bin", "/usr/local/bin"})
	if p == "" {
		return ToolCheck{
			Name:           "git",
			Category:       CategoryCore,
			Status:         StatusMissing,
			Required:       true,
			Recommendation: "instalar git",
		}
	}
	v := runVersion(ctx, p, "--version")
	return ToolCheck{
		Name:     "git",
		Category: CategoryCore,
		Status:   StatusOK,
		Path:     p,
		Version:  v,
		Required: true,
	}
}

func checkGH(ctx context.Context, home string) ToolCheck {
	p := findExecutable("gh", []string{"/usr/bin", "/usr/local/bin"})
	if p == "" {
		return ToolCheck{
			Name:           "gh",
			Category:       CategoryCore,
			Status:         StatusMissing,
			Required:       false,
			Recommendation: "instalar GitHub CLI (gh)",
		}
	}
	v := runVersion(ctx, p, "--version")
	return ToolCheck{
		Name:     "gh",
		Category: CategoryCore,
		Status:   StatusOK,
		Path:     p,
		Version:  v,
		Required: false,
	}
}

func checkOrq(ctx context.Context, home string) ToolCheck {
	extraDirs := []string{filepath.Join(home, ".local", "bin")}
	p := findExecutable("orq", extraDirs)
	if p == "" {
		if ex, err := os.Executable(); err == nil && strings.Contains(filepath.Base(ex), "orq") {
			p = ex
		}
	}
	if p == "" {
		return ToolCheck{
			Name:           "orq",
			Category:       CategoryCore,
			Status:         StatusMissing,
			Required:       true,
			Recommendation: "ejecutar 'make install' en agent-orchestrator",
		}
	}
	return ToolCheck{
		Name:     "orq",
		Category: CategoryCore,
		Status:   StatusOK,
		Path:     p,
		Required: true,
	}
}

func checkVG(ctx context.Context, home string) ToolCheck {
	if envPath := strings.TrimSpace(os.Getenv("ORQ_VG_PATH")); envPath != "" {
		if isExecutable(envPath) {
			return ToolCheck{
				Name:     "vg",
				Category: CategoryTooling,
				Status:   StatusOK,
				Path:     envPath,
				Required: false,
				Note:     "detectado via ORQ_VG_PATH",
			}
		}
	}

	if p, err := exec.LookPath("vg"); err == nil && p != "" {
		return ToolCheck{
			Name:     "vg",
			Category: CategoryTooling,
			Status:   StatusOK,
			Path:     p,
			Required: false,
		}
	}

	knownPaths := []string{
		filepath.Join(home, "Workspace", "Obsidian", "10.Tooling", "vault-graph", "vg"),
		filepath.Join(home, "Workspace", "Obsidian", "Tooling", "vault-graph", "scripts", "vg"),
		filepath.Join(home, "Workspace", "Tooling", "vault-graph", "scripts", "vg"),
		filepath.Join(home, ".local", "bin", "vg"),
	}
	for _, candidate := range knownPaths {
		if isExecutable(candidate) {
			return ToolCheck{
				Name:           "vg",
				Category:       CategoryTooling,
				Status:         StatusOK,
				Path:           candidate,
				Required:       false,
				Note:           "detectado en ruta conocida del workspace",
				Recommendation: "agregar al PATH o configurar ORQ_VG_PATH para acceso directo",
			}
		}
	}

	return ToolCheck{
		Name:           "vg",
		Category:       CategoryTooling,
		Status:         StatusMissing,
		Required:       false,
		Recommendation: "agregar Tooling/vault-graph/scripts al PATH o configurar ORQ_VG_PATH para consultas de vault",
	}
}

func checkOpenClaw(ctx context.Context, home string) ToolCheck {
	extraDirs := []string{
		filepath.Join(home, ".local", "bin"),
		filepath.Join(home, ".local", "share", "pi-node", "node-v22.23.2-linux-x64", "bin"),
	}
	p := findExecutable("openclaw", extraDirs)
	cfgDir := checkDirectory(filepath.Join(home, ".openclaw"))
	if cfgDir == "" {
		cfgDir = checkDirectory(filepath.Join(home, ".config", "openclaw"))
	}

	if p == "" {
		return ToolCheck{
			Name:           "openclaw",
			Category:       CategoryAgent,
			Status:         StatusMissing,
			ConfigPath:     cfgDir,
			Required:       false,
			Recommendation: "instalar openclaw para delegación de bajo costo (Haiku)",
			Note:           "seguridad: no se inspeccionan credenciales",
		}
	}

	note := "inspeccion segura: presencia confirmada; credenciales y tokens no inspeccionados"
	if cfgDir != "" {
		note = "inspeccion segura: directoria de config detectada; credenciales y tokens no inspeccionados"
	}

	return ToolCheck{
		Name:       "openclaw",
		Category:   CategoryAgent,
		Status:     StatusOK,
		Path:       p,
		ConfigPath: cfgDir,
		Required:   false,
		Note:       note,
	}
}

func checkAGY(ctx context.Context, home string) ToolCheck {
	extraDirs := []string{filepath.Join(home, ".local", "bin")}
	p := findExecutable("agy", extraDirs)
	cfgDir := checkDirectory(filepath.Join(home, ".gemini"))

	if p == "" {
		return ToolCheck{
			Name:           "agy",
			Category:       CategoryAgent,
			Status:         StatusMissing,
			ConfigPath:     cfgDir,
			Required:       false,
			Recommendation: "instalar Antigravity CLI (agy)",
		}
	}
	return ToolCheck{
		Name:       "agy",
		Category:   CategoryAgent,
		Status:     StatusOK,
		Path:       p,
		ConfigPath: cfgDir,
		Required:   false,
		Note:       "Antigravity CLI para ejecucion de tareas tecnicas y modelos flash/open",
	}
}

func checkHermes(ctx context.Context, home string) ToolCheck {
	extraDirs := []string{filepath.Join(home, ".local", "bin")}
	p := findExecutable("hermes", extraDirs)
	cfgDir := checkDirectory(filepath.Join(home, ".hermes"))

	if p == "" {
		return ToolCheck{
			Name:           "hermes",
			Category:       CategoryAgent,
			Status:         StatusMissing,
			ConfigPath:     cfgDir,
			Required:       false,
			Recommendation: "instalar Hermes CLI",
		}
	}
	return ToolCheck{
		Name:       "hermes",
		Category:   CategoryAgent,
		Status:     StatusOK,
		Path:       p,
		ConfigPath: cfgDir,
		Required:   false,
	}
}

func checkClaude(ctx context.Context, home string) ToolCheck {
	extraDirs := []string{filepath.Join(home, ".local", "bin")}
	p := findExecutable("claude", extraDirs)
	cfgDir := checkDirectory(filepath.Join(home, ".claude"))

	if p == "" {
		return ToolCheck{
			Name:           "claude",
			Category:       CategoryAgent,
			Status:         StatusMissing,
			ConfigPath:     cfgDir,
			Required:       false,
			Recommendation: "instalar Claude Code CLI",
		}
	}
	return ToolCheck{
		Name:       "claude",
		Category:   CategoryAgent,
		Status:     StatusOK,
		Path:       p,
		ConfigPath: cfgDir,
		Required:   false,
		Note:       "Claude Code CLI (politica: review_only / nivel 3-4)",
	}
}
