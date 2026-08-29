package audit

import "testing"

func TestParseWorktrees(t *testing.T) {
	input := []byte("worktree /repo\nHEAD abc123\nbranch refs/heads/main\n\nworktree /repo-wt\nHEAD def456\ndetached\nprunable gitdir file points to non-existent location\n")
	items := ParseWorktrees(input)
	if len(items) != 2 {
		t.Fatalf("expected 2 worktrees, got %d", len(items))
	}
	if items[0].Path != "/repo" || items[0].Branch != "main" || items[0].Detached {
		t.Fatalf("unexpected first worktree: %+v", items[0])
	}
	if !items[1].Detached || !items[1].Prunable || items[1].Reason == "" {
		t.Fatalf("unexpected second worktree: %+v", items[1])
	}
}

func TestParseWorktreesEmpty(t *testing.T) {
	if items := ParseWorktrees(nil); len(items) != 0 {
		t.Fatalf("expected empty, got %+v", items)
	}
}
