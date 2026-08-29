package session

import "strings"

// Report summarizes whether a local orchestration session has enough evidence to continue safely.
type Report struct {
	Valid    bool      `json:"valid"`
	Checks   []Check   `json:"checks"`
	Findings []Finding `json:"findings"`
}

type Check struct {
	Name   string `json:"name"`
	Passed bool   `json:"passed"`
	Detail string `json:"detail"`
}

type Finding struct {
	Severity string `json:"severity"`
	Message  string `json:"message"`
}

type Input struct {
	RepoCheck        string
	SafetyCheck      string
	GuardCollision   string
	Tests            string
	Receipt          string
	Handoff          string
	HumanApproval    bool
	TouchesDangerous bool
}

// Validate applies deterministic local-first checklist rules.
func Validate(in Input) Report {
	checks := []Check{
		checkContains("guard_collision", in.GuardCollision, []string{"OK", "should_stop=false"}),
		checkContains("repo_check", in.RepoCheck, []string{"OK", "valid=true"}),
		checkContains("safety_check", in.SafetyCheck, []string{"OK", "valid=true"}),
		checkContains("tests", in.Tests, []string{"OK", "PASS", "passed"}),
		checkContains("receipt", in.Receipt, []string{"OK", "valid=true"}),
	}
	if strings.TrimSpace(in.Handoff) != "" {
		checks = append(checks, Check{Name: "handoff", Passed: true, Detail: "handoff registrado"})
	}
	findings := []Finding{}
	valid := true
	for _, c := range checks {
		if !c.Passed {
			valid = false
			findings = append(findings, Finding{Severity: "blocker", Message: c.Name + " requerido o no verificable"})
		}
	}
	if in.TouchesDangerous && !in.HumanApproval {
		valid = false
		findings = append(findings, Finding{Severity: "blocker", Message: "accion peligrosa requiere aprobacion humana explicita"})
	}
	return Report{Valid: valid, Checks: checks, Findings: findings}
}

func checkContains(name string, value string, accepted []string) Check {
	trimmed := strings.TrimSpace(value)
	if trimmed == "" {
		return Check{Name: name, Passed: false, Detail: "sin evidencia"}
	}
	for _, token := range accepted {
		if strings.Contains(trimmed, token) {
			return Check{Name: name, Passed: true, Detail: trimmed}
		}
	}
	return Check{Name: name, Passed: false, Detail: trimmed}
}
