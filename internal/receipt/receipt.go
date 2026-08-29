package receipt

import (
	"encoding/json"
	"fmt"
	"os"
	"strings"
	"time"
)

// Receipt is a verifiable RDD record for one unit of work.
type Receipt struct {
	Task          string    `json:"task"`
	Agent         string    `json:"agent"`
	Provider      string    `json:"provider"`
	Model         string    `json:"model"`
	PR            int       `json:"pr,omitempty"`
	Risk          string    `json:"risk"`
	FilesChanged  []string  `json:"files_changed"`
	Commands      []Command `json:"commands"`
	SecurityNotes []string  `json:"security_notes"`
	Rollback      string    `json:"rollback"`
	Evidence      []string  `json:"evidence"`
	CreatedAt     time.Time `json:"created_at"`
}

// Command stores one validation command and its declared result.
type Command struct {
	Cmd    string `json:"cmd"`
	Result string `json:"result"`
}

// New builds a receipt with safe defaults.
func New(task, agent, provider, model, risk string, pr int) Receipt {
	return Receipt{Task: task, Agent: agent, Provider: provider, Model: model, PR: pr, Risk: risk, CreatedAt: time.Now().UTC()}
}

// Save writes the receipt as indented JSON.
func Save(path string, r Receipt) error {
	data, err := json.MarshalIndent(r, "", "  ")
	if err != nil {
		return err
	}
	data = append(data, '\n')
	return os.WriteFile(path, data, 0o644)
}

// Load reads a receipt from JSON.
func Load(path string) (Receipt, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return Receipt{}, err
	}
	var r Receipt
	if err := json.Unmarshal(data, &r); err != nil {
		return Receipt{}, err
	}
	return r, nil
}

// Verify validates that the receipt has enough evidence to be useful.
func Verify(r Receipt) []string {
	var findings []string
	if strings.TrimSpace(r.Task) == "" {
		findings = append(findings, "task requerido")
	}
	if strings.TrimSpace(r.Agent) == "" {
		findings = append(findings, "agent requerido")
	}
	if strings.TrimSpace(r.Provider) == "" {
		findings = append(findings, "provider requerido")
	}
	if strings.TrimSpace(r.Risk) == "" {
		findings = append(findings, "risk requerido")
	}
	if len(r.Commands) == 0 {
		findings = append(findings, "commands requerido")
	}
	for i, cmd := range r.Commands {
		if strings.TrimSpace(cmd.Cmd) == "" || strings.TrimSpace(cmd.Result) == "" {
			findings = append(findings, fmt.Sprintf("commands[%d] incompleto", i))
		}
	}
	if len(r.Evidence) == 0 {
		findings = append(findings, "evidence requerido")
	}
	if strings.TrimSpace(r.Rollback) == "" {
		findings = append(findings, "rollback requerido")
	}
	if r.Risk == "alto" && len(r.SecurityNotes) == 0 {
		findings = append(findings, "security_notes requerido para riesgo alto")
	}
	return findings
}
