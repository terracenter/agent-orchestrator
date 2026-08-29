package guides

import (
	"strings"
	"testing"
)

func TestTextUsageIncludesOperationalRules(t *testing.T) {
	text, err := Text("usage")
	if err != nil {
		t.Fatal(err)
	}
	for _, want := range []string{"orq es la autoridad", "rtk", "vg", "--dangerously-skip-permissions"} {
		if !strings.Contains(text, want) {
			t.Fatalf("usage guide missing %q in %q", want, text)
		}
	}
}

func TestTextUnknownGuide(t *testing.T) {
	if _, err := Text("missing"); err == nil {
		t.Fatal("expected error for unknown guide")
	}
}
