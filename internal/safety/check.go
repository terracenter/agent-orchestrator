package safety

import (
	"fmt"
	"os/exec"
	"path/filepath"
	"strings"
)

type Level string

const (
	LevelLow    Level = "bajo"
	LevelMedium Level = "medio"
	LevelHigh   Level = "alto"
)

type Finding struct {
	Level  Level  `json:"level"`
	Path   string `json:"path,omitempty"`
	Reason string `json:"reason"`
}

type Report struct {
	Root     string    `json:"root"`
	Passed   bool      `json:"passed"`
	Risk     Level     `json:"risk"`
	Findings []Finding `json:"findings,omitempty"`
}

func CheckRepo(path string) Report {
	report := Report{Root: path, Passed: true, Risk: LevelLow}
	if bad, reason := UnsafePath(path); bad {
		report.add(LevelHigh, path, reason)
		return report
	}
	files, err := changedFiles(path)
	if err != nil {
		report.add(LevelMedium, "", "no se pudo leer git diff: "+err.Error())
		return report
	}
	for _, file := range files {
		for _, finding := range classifyFile(file) {
			report.add(finding.Level, finding.Path, finding.Reason)
		}
	}
	return report
}

func UnsafePath(path string) (bool, string) {
	if strings.TrimSpace(path) == "" {
		return true, "path vacío"
	}
	if strings.Contains(path, "\x00") {
		return true, "path contiene byte nulo"
	}
	clean := filepath.Clean(path)
	parts := strings.Split(clean, string(filepath.Separator))
	for _, part := range parts {
		if part == ".." {
			return true, "path traversal no permitido"
		}
	}
	return false, ""
}

func UnsafeCommand(command string) (bool, string) {
	dangerous := []string{"\x00", "\n", "\r", ";", "&&", "||", "`", "$(", ">", "<"}
	for _, token := range dangerous {
		if strings.Contains(command, token) {
			return true, "comando contiene token peligroso: " + token
		}
	}
	return false, ""
}

func (r *Report) add(level Level, path, reason string) {
	r.Findings = append(r.Findings, Finding{Level: level, Path: path, Reason: reason})
	if level == LevelHigh {
		r.Passed = false
	}
	if rank(level) > rank(r.Risk) {
		r.Risk = level
	}
}

func rank(level Level) int {
	switch level {
	case LevelHigh:
		return 3
	case LevelMedium:
		return 2
	default:
		return 1
	}
}

func changedFiles(path string) ([]string, error) {
	cmd := exec.Command("git", "diff", "--name-only", "HEAD")
	cmd.Dir = path
	out, err := cmd.Output()
	if err != nil {
		return nil, fmt.Errorf("git diff --name-only HEAD: %w", err)
	}
	var files []string
	for _, line := range strings.Split(string(out), "\n") {
		line = strings.TrimSpace(line)
		if line != "" {
			files = append(files, line)
		}
	}
	return files, nil
}

func classifyFile(path string) []Finding {
	p := strings.ToLower(filepath.ToSlash(path))
	var out []Finding
	switch {
	case p == "go.mod" || p == "go.sum":
		out = append(out, Finding{Level: LevelHigh, Path: path, Reason: "cambio de dependencias requiere intervención humana"})
	case strings.Contains(p, "migration") && strings.HasSuffix(p, ".sql"):
		out = append(out, Finding{Level: LevelHigh, Path: path, Reason: "migración SQL requiere revisión humana"})
	case strings.Contains(p, ".env") || strings.Contains(p, "secret") || strings.Contains(p, "token"):
		out = append(out, Finding{Level: LevelHigh, Path: path, Reason: "posible secreto o configuración sensible"})
	case strings.Contains(p, "auth") || strings.Contains(p, "permission") || strings.Contains(p, "authorization"):
		out = append(out, Finding{Level: LevelHigh, Path: path, Reason: "cambio de autenticación/autorización requiere revisión humana"})
	case strings.Contains(p, "dockerfile") || strings.Contains(p, "docker-compose") || strings.Contains(p, "deploy") || strings.Contains(p, "nginx"):
		out = append(out, Finding{Level: LevelMedium, Path: path, Reason: "cambio de infraestructura/deploy requiere cautela"})
	}
	return out
}
