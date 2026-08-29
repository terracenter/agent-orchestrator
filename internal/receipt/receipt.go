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
	Task                          string    `json:"task"`
	Agent                         string    `json:"agent"`
	Provider                      string    `json:"provider"`
	Model                         string    `json:"model"`
	PR                            int       `json:"pr,omitempty"`
	Risk                          string    `json:"risk"`
	FilesChanged                  []string  `json:"files_changed"`
	Commands                      []Command `json:"commands"`
	SecurityNotes                 []string  `json:"security_notes"`
	HumanEditsRequired            bool      `json:"human_edits_required"`
	HumanEditsRequiredValue       string    `json:"human_edits_required_value,omitempty"`
	CorreccionesHumanasRequeridas bool      `json:"correcciones_humanas_requeridas"`
	HumanEditsNotes               []string  `json:"human_edits_notes,omitempty"`
	Rollback                      string    `json:"rollback"`
	Evidence                      []string  `json:"evidence"`
	CreatedAt                     time.Time `json:"created_at"`
}

// Command stores one validation command and its declared result.
type Command struct {
	Cmd    string `json:"cmd"`
	Result string `json:"result"`
}

// ValidCommandResult reports whether result is an accepted explicit command outcome.
func ValidCommandResult(result string) bool {
	switch result {
	case "passed", "failed", "skipped", "recorded":
		return true
	default:
		return false
	}
}

func ValidHumanEditsRequiredValue(value string) bool {
	value = strings.TrimSpace(value)
	if value == "unknown" {
		return true
	}
	if value == "" {
		return false
	}
	for _, r := range value {
		if r < '0' || r > '9' {
			return false
		}
	}
	return true
}

// PRInfo stores the small subset of pull request metadata needed for RDD.
type PRInfo struct {
	Number      int
	Title       string
	URL         string
	HeadRef     string
	BaseRef     string
	MergeCommit string
	Files       []string
	Checks      []string
}

// New builds a receipt with safe defaults.
func New(task, agent, provider, model, risk string, pr int) Receipt {
	return Receipt{Task: task, Agent: agent, Provider: provider, Model: model, PR: pr, Risk: risk, CreatedAt: time.Now().UTC()}
}

// FromPR builds a receipt from pull request metadata.
func FromPR(info PRInfo, agent, provider, model, risk string) Receipt {
	r := New(info.Title, agent, provider, model, risk, info.Number)
	r.FilesChanged = append([]string(nil), info.Files...)
	r.Commands = []Command{{Cmd: "gh pr checks", Result: strings.Join(info.Checks, ", ")}}
	r.Rollback = fmt.Sprintf("revert PR #%d", info.Number)
	r.Evidence = []string{fmt.Sprintf("PR #%d", info.Number)}
	if info.URL != "" {
		r.Evidence = append(r.Evidence, info.URL)
	}
	if info.MergeCommit != "" {
		r.Evidence = append(r.Evidence, "merge commit "+info.MergeCommit)
	}
	return r
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
			continue
		}
		if !ValidCommandResult(cmd.Result) && !strings.Contains(cmd.Result, "SUCCESS") && !strings.Contains(cmd.Result, "FAILURE") {
			findings = append(findings, fmt.Sprintf("commands[%d] result invalido", i))
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
	if strings.TrimSpace(r.HumanEditsRequiredValue) != "" && !ValidHumanEditsRequiredValue(r.HumanEditsRequiredValue) {
		findings = append(findings, "human_edits_required_value debe ser entero no negativo o unknown")
	}
	if r.HumanEditsRequired || r.CorreccionesHumanasRequeridas {
		if !r.HumanEditsRequired || !r.CorreccionesHumanasRequeridas {
			findings = append(findings, "marcadores de correcciones humanas deben estar sincronizados")
		}
		if len(r.HumanEditsNotes) == 0 {
			findings = append(findings, "human_edits_notes requerido cuando hay correcciones humanas")
		}
	}
	if len(r.HumanEditsNotes) > 0 && (!r.HumanEditsRequired || !r.CorreccionesHumanasRequeridas) {
		findings = append(findings, "human_edits_notes requiere human_edits_required")
	}
	return findings
}
