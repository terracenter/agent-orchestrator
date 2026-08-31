package roadmap

import (
	"os"
	"path/filepath"
	"testing"
)

func TestCheckPhaseBlocksFuturePhaseWhenPriorOpenItemsExist(t *testing.T) {
	path := writeRoadmap(t, `# ROADMAP

## Fase 1 — Uno

- [x] Hecho
- [ ] Pendiente previo

## Fase 2 — Dos

- [ ] Pendiente actual
`)

	report, err := CheckPhase(path, 2, "")
	if err != nil {
		t.Fatalf("CheckPhase returned error: %v", err)
	}
	if report.Allowed {
		t.Fatalf("expected phase to be blocked: %+v", report)
	}
	if len(report.BlockingOpenItems) != 1 || report.BlockingOpenItems[0].Phase != 1 {
		t.Fatalf("unexpected blockers: %+v", report.BlockingOpenItems)
	}
}

func TestCheckPhaseAllowsExplicitSecurityOrOptimizationOverride(t *testing.T) {
	path := writeRoadmap(t, `# ROADMAP

## Fase 1 — Uno

- [ ] Pendiente previo
`)

	for _, reason := range []string{"security", "optimization", "cost"} {
		report, err := CheckPhase(path, 7, reason)
		if err != nil {
			t.Fatalf("CheckPhase(%s) returned error: %v", reason, err)
		}
		if !report.Allowed {
			t.Fatalf("expected override %s to allow phase: %+v", reason, report)
		}
	}
}

func writeRoadmap(t *testing.T, content string) string {
	t.Helper()
	path := filepath.Join(t.TempDir(), "ROADMAP.md")
	if err := os.WriteFile(path, []byte(content), 0o644); err != nil {
		t.Fatalf("write roadmap: %v", err)
	}
	return path
}
