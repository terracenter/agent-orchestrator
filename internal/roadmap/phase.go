package roadmap

import (
	"bufio"
	"fmt"
	"os"
	"regexp"
	"sort"
)

type PhaseReport struct {
	Path              string     `json:"path"`
	RequestedPhase    int        `json:"requested_phase"`
	Allowed           bool       `json:"allowed"`
	OverrideReason    string     `json:"override_reason,omitempty"`
	BlockingOpenItems []OpenItem `json:"blocking_open_items,omitempty"`
}

type OpenItem struct {
	Phase int    `json:"phase"`
	Line  int    `json:"line"`
	Text  string `json:"text"`
}

var phaseHeader = regexp.MustCompile(`^## Fase ([0-9]+)`) // Spanish roadmap contract.

func CheckPhase(path string, requestedPhase int, overrideReason string) (PhaseReport, error) {
	if requestedPhase <= 0 {
		return PhaseReport{}, fmt.Errorf("phase must be greater than zero")
	}
	report := PhaseReport{Path: path, RequestedPhase: requestedPhase, OverrideReason: overrideReason}
	if overrideReason == "security" || overrideReason == "optimization" || overrideReason == "cost" {
		report.Allowed = true
		return report, nil
	}

	items, err := openItemsBefore(path, requestedPhase)
	if err != nil {
		return report, err
	}
	report.BlockingOpenItems = items
	report.Allowed = len(items) == 0
	return report, nil
}

func openItemsBefore(path string, requestedPhase int) ([]OpenItem, error) {
	file, err := os.Open(path)
	if err != nil {
		return nil, fmt.Errorf("open roadmap: %w", err)
	}
	defer file.Close()

	currentPhase := 0
	var items []OpenItem
	scanner := bufio.NewScanner(file)
	line := 0
	for scanner.Scan() {
		line++
		text := scanner.Text()
		if match := phaseHeader.FindStringSubmatch(text); len(match) == 2 {
			_, _ = fmt.Sscanf(match[1], "%d", &currentPhase)
			continue
		}
		if currentPhase > 0 && currentPhase < requestedPhase && len(text) >= 6 && text[:6] == "- [ ] " {
			items = append(items, OpenItem{Phase: currentPhase, Line: line, Text: text[6:]})
		}
	}
	if err := scanner.Err(); err != nil {
		return nil, fmt.Errorf("scan roadmap: %w", err)
	}
	sort.Slice(items, func(i, j int) bool { return items[i].Line < items[j].Line })
	return items, nil
}
