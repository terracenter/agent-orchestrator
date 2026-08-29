package delegate

import (
	"strings"
	"testing"

	"github.com/terracenter/agent-orchestrator/internal/route"
)

func TestPromptIncludesCheapRouting(t *testing.T) {
	decision := route.Decide("ordenar informacion del vault relacionada con GLPI")
	prompt := Prompt(decision.Task, decision)
	for _, want := range []string{"OBLIGATORIO", "prefijado con rtk", "vg", "categoria=documentacion", "Evita escalar"} {
		if !strings.Contains(prompt, want) {
			t.Fatalf("prompt missing %q:\n%s", want, prompt)
		}
	}
}

func TestPlanForPiUnexecutedRequiresStop(t *testing.T) {
	res := Plan("ordenar informacion del vault", "pi", false)
	if res.Status != "not_executed" {
		t.Fatalf("Status = %q, want not_executed", res.Status)
	}
	if !res.MustStopForDelegation {
		t.Fatal("MustStopForDelegation = false, want true")
	}
	if !res.SupervisorOnly {
		t.Fatal("SupervisorOnly = false, want true")
	}
	if res.ExecutionAgentAllowed {
		t.Fatal("ExecutionAgentAllowed = true, want false")
	}
}

func TestPlanForPiExecutedAllowsReceipt(t *testing.T) {
	res := Plan("ordenar informacion del vault", "pi", true)
	if res.Status != "executed_unverified" {
		t.Fatalf("Status = %q, want executed_unverified", res.Status)
	}
	if res.MustStopForDelegation {
		t.Fatal("MustStopForDelegation = true, want false when already executed")
	}
	if !res.SupervisorOnly {
		t.Fatal("SupervisorOnly = false, want true")
	}
	if !res.ExecutionAgentAllowed {
		t.Fatal("ExecutionAgentAllowed = false, want true")
	}
}

func TestPlanForExternalAgentAllowsExecution(t *testing.T) {
	res := Plan("ordenar informacion del vault", "agy", false)
	if res.Status != "not_executed" {
		t.Fatalf("Status = %q, want not_executed", res.Status)
	}
	if res.MustStopForDelegation {
		t.Fatal("MustStopForDelegation = true, want false for agy")
	}
	if res.SupervisorOnly {
		t.Fatal("SupervisorOnly = true, want false for agy")
	}
	if !res.ExecutionAgentAllowed {
		t.Fatal("ExecutionAgentAllowed = false, want true for agy")
	}
}
