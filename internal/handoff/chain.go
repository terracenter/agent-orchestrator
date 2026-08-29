package handoff

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"
)

// ChainRequest describes a follow-up handoff derived from a previous artifact.
type ChainRequest struct {
	From      string `json:"from"`
	To        string `json:"to"`
	Task      string `json:"task"`
	NextAgent string `json:"next_agent"`
}

// ChainResult is the deterministic output of a handoff chain operation.
type ChainResult struct {
	From      string `json:"from"`
	To        string `json:"to"`
	Task      string `json:"task"`
	NextAgent string `json:"next_agent"`
	Bytes     int    `json:"bytes"`
}

// BuildChain creates a handoff that carries prior context forward without asking a human to be the messenger.
func BuildChain(req ChainRequest, previous string) (string, error) {
	if strings.TrimSpace(req.From) == "" {
		return "", fmt.Errorf("from requerido")
	}
	if strings.TrimSpace(req.To) == "" {
		return "", fmt.Errorf("to requerido")
	}
	if strings.TrimSpace(req.Task) == "" {
		return "", fmt.Errorf("task requerido")
	}
	if strings.TrimSpace(req.NextAgent) == "" {
		return "", fmt.Errorf("next-agent requerido")
	}
	previous = strings.TrimSpace(previous)
	if previous == "" {
		return "", fmt.Errorf("handoff previo vacio")
	}
	return strings.TrimSpace(fmt.Sprintf(`# HANDOFF CHAIN: %s

Fecha: %s
Origen: %s
Destino: %s
Siguiente agente: %s

## Contexto heredado

%s

## Reglas de continuidad

- No pedirle a Freddy que copie mensajes entre agentes.
- Ejecutar guardias antes de modificar código: orq guard-collision, orq repo check, orq safety check.
- No tocar producción, secretos, DB, DNS, firewall ni acciones irreversibles sin aprobación humana explícita.
- Registrar evidencia verificable: comandos ejecutados, PR/checks o recibo RDD.
- Si hay bloqueo, dejarlo escrito en el handoff de salida con causa y evidencia.
`, req.Task, time.Now().UTC().Format(time.RFC3339), req.From, req.To, req.NextAgent, previous)) + "\n", nil
}

// Chain reads a previous handoff and writes the next one. It never overwrites output.
func Chain(req ChainRequest) (ChainResult, error) {
	data, err := os.ReadFile(req.From)
	if err != nil {
		return ChainResult{}, err
	}
	body, err := BuildChain(req, string(data))
	if err != nil {
		return ChainResult{}, err
	}
	if _, err := os.Stat(req.To); err == nil {
		return ChainResult{}, fmt.Errorf("output already exists: %s", req.To)
	} else if !os.IsNotExist(err) {
		return ChainResult{}, err
	}
	if err := os.MkdirAll(filepath.Dir(req.To), 0o755); err != nil {
		return ChainResult{}, err
	}
	if err := os.WriteFile(req.To, []byte(body), 0o644); err != nil {
		return ChainResult{}, err
	}
	return ChainResult{From: req.From, To: req.To, Task: req.Task, NextAgent: req.NextAgent, Bytes: len(body)}, nil
}
