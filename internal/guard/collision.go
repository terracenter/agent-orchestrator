package guard

import (
	"bytes"
	"context"
	"os/exec"
	"strings"
)

type CollisionStatus struct {
	Path            string `json:"path"`
	Branch          string `json:"branch"`
	Dirty           bool   `json:"dirty"`
	Worktree        string `json:"worktree"`
	ActiveWorktrees int    `json:"active_worktrees"`
	ShouldStop      bool   `json:"should_stop"`
	Reason          string `json:"reason"`
}

func GitCollision(ctx context.Context, path string) CollisionStatus {
	status := CollisionStatus{Path: path}
	branch := runGit(ctx, path, "branch", "--show-current")
	status.Branch = strings.TrimSpace(branch)
	status.Worktree = strings.TrimSpace(runGit(ctx, path, "worktree", "list"))
	if status.Worktree != "" {
		status.ActiveWorktrees = len(strings.Split(status.Worktree, "\n"))
	}
	dirty := strings.TrimSpace(runGit(ctx, path, "status", "--short"))
	status.Dirty = dirty != ""
	status.ShouldStop = status.Dirty || status.ActiveWorktrees > 1
	if status.Dirty {
		status.Reason = "repo tiene cambios sin commitear; posible colision con otra sesion/agente"
	} else if status.ActiveWorktrees > 1 {
		status.Reason = "repo tiene mas de un worktree activo; confirmar dueño de la rama antes de tocar"
	} else {
		status.Reason = "repo limpio y sin worktrees paralelos"
	}
	return status
}

func runGit(ctx context.Context, path string, args ...string) string {
	cmd := exec.CommandContext(ctx, "git", append([]string{"-C", path}, args...)...)
	var stdout, stderr bytes.Buffer
	cmd.Stdout = &stdout
	cmd.Stderr = &stderr
	if err := cmd.Run(); err != nil {
		return stderr.String()
	}
	return stdout.String()
}
