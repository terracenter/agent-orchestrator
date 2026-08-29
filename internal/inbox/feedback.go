package inbox

import (
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"
)

// FeedbackResume is a compact, machine-readable view of a Hermes feedback file.
type FeedbackResume struct {
	Path         string    `json:"path"`
	File         string    `json:"file"`
	TaskID       string    `json:"task_id,omitempty"`
	Task         string    `json:"task,omitempty"`
	Agent        string    `json:"agent,omitempty"`
	Result       string    `json:"result,omitempty"`
	NeedsHuman   bool      `json:"needs_human"`
	NextForPi    bool      `json:"next_for_pi"`
	ModifiedTime time.Time `json:"modified_time"`
}

// ScanFeedbacks reads markdown feedback files from dir and returns newest first.
func ScanFeedbacks(dir string) ([]FeedbackResume, error) {
	entries, err := os.ReadDir(dir)
	if err != nil {
		return nil, err
	}
	items := make([]FeedbackResume, 0, len(entries))
	for _, entry := range entries {
		if entry.IsDir() || !strings.HasSuffix(entry.Name(), ".md") {
			continue
		}
		name := entry.Name()
		if !strings.Contains(name, "feedback") && !strings.Contains(name, "validation") && !strings.Contains(name, "diagnostico") {
			continue
		}
		path := filepath.Join(dir, name)
		data, err := os.ReadFile(path)
		if err != nil {
			return nil, err
		}
		info, err := entry.Info()
		if err != nil {
			return nil, err
		}
		items = append(items, parseFeedback(path, name, string(data), info.ModTime()))
	}
	sort.Slice(items, func(i, j int) bool { return items[i].ModifiedTime.After(items[j].ModifiedTime) })
	return items, nil
}

// NextFeedback returns the newest feedback that needs Pi/Claude or human attention.
func NextFeedback(items []FeedbackResume) (FeedbackResume, bool) {
	for _, item := range items {
		if item.NeedsHuman || item.NextForPi {
			return item, true
		}
	}
	return FeedbackResume{}, false
}

func parseFeedback(path, file, content string, mod time.Time) FeedbackResume {
	item := FeedbackResume{Path: path, File: file, ModifiedTime: mod}
	for _, line := range strings.Split(content, "\n") {
		trimmed := strings.TrimSpace(line)
		if strings.HasPrefix(trimmed, "- Tarea:") {
			item.Task = strings.TrimSpace(strings.TrimPrefix(trimmed, "- Tarea:"))
		}
		if strings.HasPrefix(trimmed, "- Task ID:") {
			item.TaskID = strings.TrimSpace(strings.TrimPrefix(trimmed, "- Task ID:"))
		}
		if strings.HasPrefix(trimmed, "- Agente:") {
			item.Agent = strings.TrimSpace(strings.TrimPrefix(trimmed, "- Agente:"))
		}
		if strings.Contains(trimmed, "DEPLOY_LOCAL_OK") || strings.Contains(trimmed, "DRY_RUN_OK") || strings.Contains(trimmed, "INTEGRACION_LOCAL_OK") || strings.Contains(trimmed, "BLOCKED_HUMAN") || strings.Contains(trimmed, "NEEDS_ORQ_CHANGE") || strings.Contains(trimmed, "FAILED") {
			item.Result = strings.TrimPrefix(trimmed, "- ")
		}
	}
	lower := strings.ToLower(content)
	item.NeedsHuman = strings.Contains(content, "BLOCKED_HUMAN") || strings.Contains(content, "FAILED") || strings.Contains(lower, "requiere intervención humana") || strings.Contains(lower, "pendiente para freddy")
	item.NextForPi = strings.Contains(content, "NEEDS_ORQ_CHANGE") || strings.Contains(lower, "pi/claude") || strings.Contains(lower, "recomendaciones para mejorar agent-orchestrator")
	return item
}
