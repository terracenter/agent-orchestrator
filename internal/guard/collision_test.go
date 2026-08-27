package guard

import (
	"context"
	"os"
	"os/exec"
	"path/filepath"
	"testing"
)

func TestGitCollisionDetectsDirtyRepo(t *testing.T) {
	repo := t.TempDir()
	run(t, repo, "git", "init")
	run(t, repo, "git", "config", "user.email", "test@example.com")
	run(t, repo, "git", "config", "user.name", "Test")
	writeFile(t, filepath.Join(repo, "README.md"), "ok")
	run(t, repo, "git", "add", "README.md")
	run(t, repo, "git", "commit", "-m", "init")
	writeFile(t, filepath.Join(repo, "dirty.md"), "dirty")

	status := GitCollision(context.Background(), repo)
	if !status.Dirty || !status.ShouldStop {
		t.Fatalf("status = %+v, want dirty stop", status)
	}
}

func run(t *testing.T, dir string, name string, args ...string) {
	t.Helper()
	cmd := exec.Command(name, args...)
	cmd.Dir = dir
	if out, err := cmd.CombinedOutput(); err != nil {
		t.Fatalf("%s %v: %v\n%s", name, args, err, out)
	}
}

func writeFile(t *testing.T, path string, content string) {
	t.Helper()
	if err := os.WriteFile(path, []byte(content), 0o644); err != nil {
		t.Fatal(err)
	}
}
