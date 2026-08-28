package safety

import "testing"

func TestUnsafePath(t *testing.T) {
	bad, reason := UnsafePath("../secret")
	if !bad || reason == "" {
		t.Fatalf("expected unsafe path")
	}
}

func TestUnsafeCommand(t *testing.T) {
	bad, reason := UnsafeCommand("go test ./... && rm -rf /tmp/x")
	if !bad || reason == "" {
		t.Fatalf("expected unsafe command")
	}
}

func TestClassifySensitiveFiles(t *testing.T) {
	cases := []string{"go.mod", "migrations/001_init.sql", ".env", "internal/auth/handler.go"}
	for _, tc := range cases {
		got := classifyFile(tc)
		if len(got) == 0 || got[0].Level != LevelHigh {
			t.Fatalf("expected high risk for %s, got %#v", tc, got)
		}
	}
}

func TestClassifyDeployFilesMedium(t *testing.T) {
	got := classifyFile("docs/deploy-cwp.md")
	if len(got) != 1 || got[0].Level != LevelMedium {
		t.Fatalf("expected medium risk, got %#v", got)
	}
}
