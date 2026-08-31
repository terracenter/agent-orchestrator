package score

import (
	"sort"
	"strings"

	"github.com/terracenter/agent-orchestrator/internal/ledger"
)

type Summary struct {
	Agent       string  `json:"agent"`
	Model       string  `json:"model"`
	Events      int     `json:"events"`
	Successes   int     `json:"successes"`
	Failures    int     `json:"failures"`
	NotExecuted int     `json:"not_executed"`
	Score       float64 `json:"score"`
}

func FromLedger(events []ledger.Event) []Summary {
	byKey := map[string]*Summary{}
	for _, event := range events {
		key := event.Agent + "\x00" + event.Model
		summary := byKey[key]
		if summary == nil {
			summary = &Summary{Agent: event.Agent, Model: event.Model}
			byKey[key] = summary
		}
		summary.Events++
		switch normalizedStatus(event.Status) {
		case "success":
			summary.Successes++
		case "not_executed":
			summary.NotExecuted++
		default:
			summary.Failures++
		}
	}

	out := make([]Summary, 0, len(byKey))
	for _, summary := range byKey {
		if summary.Events > 0 {
			value := (float64(summary.Successes) - float64(summary.Failures) - 0.5*float64(summary.NotExecuted)) / float64(summary.Events)
			summary.Score = value
		}
		out = append(out, *summary)
	}
	sort.Slice(out, func(i, j int) bool {
		if out[i].Score == out[j].Score {
			return out[i].Agent+out[i].Model < out[j].Agent+out[j].Model
		}
		return out[i].Score > out[j].Score
	})
	return out
}

func normalizedStatus(status string) string {
	s := strings.ToLower(strings.TrimSpace(status))
	switch s {
	case "ok", "passed", "success", "completed", "executed":
		return "success"
	case "not_executed":
		return "not_executed"
	default:
		return "failure"
	}
}
