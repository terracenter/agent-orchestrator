package rtkpolicy

import "testing"

func TestBinariesIncludesCrossLanguageUnion(t *testing.T) {
	// Regression for issue #180: this list must be the single source of
	// truth consumed by both the Go auditor (internal/audit) and the Rust
	// orq-agent (orq-agent/src/compliance.rs). It must contain the union of
	// what each side used to define independently, plus the previously
	// missing entries called out in the issue (gh, ssh, curl).
	want := []string{
		// previously only in Go's internal/audit/session.go
		"cargo", "cat", "curl", "df", "diff", "docker", "gh", "go", "head",
		"npm", "ps", "pytest", "sed", "tail", "tar", "tree", "wc", "zip",
		// previously only in Rust's orq-agent/src/compliance.rs RAW_BINARIES
		"fd", "egrep", "fgrep", "ag", "ack",
		// shared by both before unification
		"git", "find", "ls", "rg", "grep",
		// gap called out explicitly by issue #180 (missing from both)
		"ssh",
	}

	got := Binaries()
	present := make(map[string]bool, len(got))
	for _, b := range got {
		present[b] = true
	}

	for _, bin := range want {
		if !present[bin] {
			t.Errorf("expected rtk_required.json to include %q, got %v", bin, got)
		}
	}
}

func TestBinariesIsSortedAndDeduped(t *testing.T) {
	got := Binaries()
	if len(got) == 0 {
		t.Fatal("Binaries() returned an empty list")
	}
	seen := make(map[string]bool, len(got))
	for i, bin := range got {
		if seen[bin] {
			t.Errorf("duplicate binary %q in rtk_required.json", bin)
		}
		seen[bin] = true
		if i > 0 && got[i-1] > bin {
			t.Errorf("Binaries() not sorted: %q comes after %q", bin, got[i-1])
		}
	}
}

func TestBinariesReturnsDefensiveCopy(t *testing.T) {
	got := Binaries()
	got[0] = "mutated"

	again := Binaries()
	if again[0] == "mutated" {
		t.Fatal("Binaries() leaked its internal slice; mutation of one call's result affected another")
	}
}
