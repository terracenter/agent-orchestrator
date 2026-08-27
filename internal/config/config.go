package config

import (
	"bufio"
	"fmt"
	"io"
	"os"
	"strings"
)

type Config struct {
	Project Project `json:"project"`
	SSoT    Adapter `json:"ssot"`
	Graph   Adapter `json:"graph"`
	Memory  Adapter `json:"memory"`
	Shell   Adapter `json:"shell"`
	Policy  Policy  `json:"policy"`
}

type Project struct {
	Name string `json:"name"`
}

type Adapter struct {
	Type string `json:"type"`
	Path string `json:"path,omitempty"`
}

type Policy struct {
	DefaultMode                    string `json:"default_mode"`
	RequireConfirmationForSecurity bool   `json:"require_confirmation_for_security"`
}

func Default() Config {
	return Config{
		Project: Project{Name: "agent-orchestrator"},
		SSoT:    Adapter{Type: "local-dir", Path: "./docs"},
		Graph:   Adapter{Type: "noop"},
		Memory:  Adapter{Type: "jsonl", Path: "~/.local/state/orq/ledger.jsonl"},
		Shell:   Adapter{Type: "standard"},
		Policy:  Policy{DefaultMode: "dry-run", RequireConfirmationForSecurity: true},
	}
}

func Load(path string) (Config, error) {
	file, err := os.Open(path)
	if err != nil {
		return Config{}, err
	}
	defer file.Close()
	return Decode(file)
}

func Decode(r io.Reader) (Config, error) {
	cfg := Default()
	scanner := bufio.NewScanner(r)
	section := ""
	lineNo := 0
	for scanner.Scan() {
		lineNo++
		line := stripComment(strings.TrimSpace(scanner.Text()))
		if line == "" {
			continue
		}
		if strings.HasPrefix(line, "[") && strings.HasSuffix(line, "]") {
			section = strings.TrimSpace(strings.TrimSuffix(strings.TrimPrefix(line, "["), "]"))
			continue
		}
		key, value, ok := strings.Cut(line, "=")
		if !ok {
			return Config{}, fmt.Errorf("config line %d: expected key = value", lineNo)
		}
		key = strings.TrimSpace(key)
		value = strings.TrimSpace(value)
		if err := assign(&cfg, section, key, value); err != nil {
			return Config{}, fmt.Errorf("config line %d: %w", lineNo, err)
		}
	}
	if err := scanner.Err(); err != nil {
		return Config{}, err
	}
	return cfg, nil
}

func assign(cfg *Config, section, key, value string) error {
	switch section {
	case "project":
		if key == "name" {
			cfg.Project.Name = parseString(value)
			return nil
		}
	case "ssot":
		return assignAdapter(&cfg.SSoT, key, value)
	case "graph":
		return assignAdapter(&cfg.Graph, key, value)
	case "memory":
		return assignAdapter(&cfg.Memory, key, value)
	case "shell":
		return assignAdapter(&cfg.Shell, key, value)
	case "policy":
		switch key {
		case "default_mode":
			cfg.Policy.DefaultMode = parseString(value)
			return nil
		case "require_confirmation_for_security":
			cfg.Policy.RequireConfirmationForSecurity = parseBool(value)
			return nil
		}
	}
	return fmt.Errorf("unknown setting %s.%s", section, key)
}

func assignAdapter(adapter *Adapter, key, value string) error {
	switch key {
	case "type":
		adapter.Type = parseString(value)
	case "path":
		adapter.Path = parseString(value)
	default:
		return fmt.Errorf("unknown adapter setting %s", key)
	}
	return nil
}

func stripComment(line string) string {
	if idx := strings.Index(line, "#"); idx >= 0 {
		return strings.TrimSpace(line[:idx])
	}
	return line
}

func parseString(value string) string {
	return strings.Trim(strings.TrimSpace(value), `"`)
}

func parseBool(value string) bool {
	return strings.EqualFold(strings.TrimSpace(value), "true")
}
