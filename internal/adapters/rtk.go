package adapters

import (
	"bytes"
	"context"
	"os/exec"
)

type RTKShell struct {
	Binary string
}

func (shell RTKShell) Command(ctx context.Context, name string, args ...string) Result {
	binary := shell.Binary
	if binary == "" {
		binary = "rtk"
	}
	cmdArgs := append([]string{name}, args...)
	cmd := exec.CommandContext(ctx, binary, cmdArgs...)
	var stdout, stderr bytes.Buffer
	cmd.Stdout = &stdout
	cmd.Stderr = &stderr
	err := cmd.Run()
	result := Result{Stdout: stdout.String(), Stderr: stderr.String(), ExitCode: 0}
	if err != nil {
		if exitErr, ok := err.(*exec.ExitError); ok {
			result.ExitCode = exitErr.ExitCode()
		} else {
			result.ExitCode = -1
			result.Stderr += err.Error()
		}
	}
	return result
}
