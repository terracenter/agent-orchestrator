package agent

import (
	"encoding/json"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"time"
)

// ConfigureRequest representa una solicitud de configuración de agente
type ConfigureRequest struct {
	Agent   string `json:"agent"`
	DryRun  bool   `json:"dry_run"`
	AutoYes bool   `json:"auto_yes"`
}

// ConfigureResult representa el resultado de configurar un agente
type ConfigureResult struct {
	Agent       string   `json:"agent"`
	Status      string   `json:"status"` // supported, unsupported, configured, dry_run
	RTKRequired bool     `json:"rtk_required"`
	ConfigPath  string   `json:"config_path,omitempty"`
	BackupPath  string   `json:"backup_path,omitempty"`
	Actions     []string `json:"actions,omitempty"`
	Notes       string   `json:"notes,omitempty"`
}

// Configure configura un agente con guardrails rtk_required
func Configure(req ConfigureRequest) (ConfigureResult, error) {
	// Validar que el agente existe
	detection := findAgentDetection(req.Agent)
	if detection == nil {
		return ConfigureResult{}, fmt.Errorf("agente desconocido: %s", req.Agent)
	}

	// Determinar si el agente soporta configuración automática
	supported := isConfigurationSupported(req.Agent)

	if !supported {
		return ConfigureResult{
			Agent:  req.Agent,
			Status: "unsupported",
			Notes:  getManualInstructions(req.Agent),
		}, nil
	}

	result := ConfigureResult{
		Agent:       req.Agent,
		RTKRequired: true,
		Actions:     []string{},
	}

	// Si es dry-run, solo mostrar lo que se haría
	if req.DryRun {
		result.Status = "dry_run"
		result.Actions = getConfigurationActions(req.Agent, detection.ConfigPath)
		result.Notes = "Vista previa: ningún archivo será modificado"
		return result, nil
	}

	// Si no hay --yes, mostrar plan sin ejecutar
	if !req.AutoYes {
		result.Status = "needs_confirmation"
		result.Actions = getConfigurationActions(req.Agent, detection.ConfigPath)
		result.Notes = "Use --yes para aplicar cambios. Los archivos de configuración existentes se respaldarán automáticamente."
		return result, nil
	}

	// Ejecutar configuración real
	backup, err := applyConfiguration(req.Agent, detection.ConfigPath)
	if err != nil {
		return ConfigureResult{}, fmt.Errorf("error aplicando configuración: %w", err)
	}

	result.Status = "configured"
	result.ConfigPath = detection.ConfigPath
	result.BackupPath = backup
	result.Actions = []string{
		"Configuración de rtk_required aplicada",
		fmt.Sprintf("Backup creado en %s", backup),
	}

	return result, nil
}

// ConfigureAll configura todos los agentes soportados
func ConfigureAll(req ConfigureRequest) ([]ConfigureResult, error) {
	detections := DetectAgents()
	var results []ConfigureResult

	for _, detection := range detections {
		agentReq := ConfigureRequest{
			Agent:   detection.Agent,
			DryRun:  req.DryRun,
			AutoYes: req.AutoYes,
		}

		result, err := Configure(agentReq)
		if err != nil {
			// Continuar con otros agentes en caso de error
			result = ConfigureResult{
				Agent:  detection.Agent,
				Status: "error",
				Notes:  err.Error(),
			}
		}

		results = append(results, result)
	}

	return results, nil
}

func findAgentDetection(agentName string) *AgentDetection {
	detections := DetectAgents()
	for i := range detections {
		if detections[i].Agent == agentName {
			return &detections[i]
		}
	}
	return nil
}

func isConfigurationSupported(agent string) bool {
	// Solo agentes con directorio de configuración conocido
	supported := map[string]bool{
		"openclaw":    true,
		"agy":         true,
		"hermes":      true,
		"claude-code": true,
		"codex":       false, // No implementado aún
		"qwen-code":   false, // Prohibido leer ~/.qwen/settings.json
		"pi":          false, // Supervisión, no requiere configuración automática
		"nvidia-api":  false, // API pura, sin config local
	}
	return supported[agent]
}

func getManualInstructions(agent string) string {
	instructions := map[string]string{
		"codex":      "Codex: agregar prompt de sistema recordando rtk_required=true en el directorio de trabajo. Ubicación de config pendiente de documentar.",
		"qwen-code":  "Qwen Code: no se modifica ~/.qwen/settings.json automáticamente (prohibido). Configurar rtk_required manualmente editando prompts en el proyecto o via CLI de qwen si lo expone.",
		"pi":         "Pi: supervisión principal. Configurar rtk_required via BOOTSTRAP.md o archivo de configuración del orquestador, no via config de Pi directamente.",
		"nvidia-api": "NVIDIA API: servicio remoto sin configuración local. Configurar rtk_required en el wrapper o script que lo invoque.",
	}

	msg, ok := instructions[agent]
	if !ok {
		return "Configuración manual no documentada. Consultar docs/uso.md"
	}
	return msg
}

func getConfigurationActions(agent string, configPath string) []string {
	// Acciones específicas por agente
	actions := map[string][]string{
		"openclaw": {
			"Crear/actualizar ~/.openclaw/rtk.md con recordatorio de rtk_required=true",
			"Agregar hook pre-command que valide uso de rtk",
		},
		"agy": {
			"Crear/actualizar ~/.gemini/rtk_required.md con recordatorio de rtk_required=true",
			"Configurar prompt de sistema para recordar rtk en comandos git/docker/shell",
		},
		"hermes": {
			"Crear/actualizar ~/.hermes/rtk_required.md con recordatorio de rtk_required=true",
			"Agregar recordatorio en prompt de sistema",
		},
		"claude-code": {
			"Verificar ~/.claude/CLAUDE.md contiene instrucciones RTK",
			"El RTK.md global ya está en ~/.claude/RTK.md (no modificar)",
		},
	}

	agentActions, ok := actions[agent]
	if !ok {
		return []string{"Sin acciones definidas para este agente"}
	}

	if configPath == "" {
		return append(agentActions, "ADVERTENCIA: directorio de configuración no detectado")
	}

	return agentActions
}

func applyConfiguration(agent string, configPath string) (string, error) {
	if configPath == "" {
		return "", fmt.Errorf("directorio de configuración no detectado para %s", agent)
	}

	// Crear directorio si no existe
	if err := os.MkdirAll(configPath, 0o755); err != nil {
		return "", fmt.Errorf("error creando directorio de configuración: %w", err)
	}

	// Contenido del recordatorio RTK
	rtkContent := `# RTK Required - Token Optimization

Este agente DEBE usar el wrapper rtk para todos los comandos que lo soporten.

## Regla obligatoria

Prefija SIEMPRE los comandos con rtk:
- rtk git status
- rtk docker ps
- rtk ls
- rtk grep <pattern>
- etc.

RTK aplica filtros inteligentes para reducir tokens 60-90% en operaciones comunes.

Si RTK no tiene filtro para un comando, lo pasa sin cambios (seguro).

Ver: /home/freddy/Workspace/CLAUDE.md para lista completa de comandos soportados.

Generado automáticamente por: orq agents configure
Fecha: ` + time.Now().Format(time.RFC3339) + "\n"

	// Determinar archivo de configuración según agente
	var configFile string
	switch agent {
	case "openclaw":
		configFile = filepath.Join(configPath, "rtk.md")
	case "agy":
		configFile = filepath.Join(configPath, "rtk_required.md")
	case "hermes":
		configFile = filepath.Join(configPath, "rtk_required.md")
	case "claude-code":
		// Claude Code ya tiene RTK.md global, solo verificamos
		globalRTK := filepath.Join(configPath, "RTK.md")
		if _, err := os.Stat(globalRTK); err == nil {
			return "", nil // Ya existe, no crear backup
		}
		configFile = filepath.Join(configPath, "rtk_required.md")
	default:
		return "", fmt.Errorf("agente no soportado para configuración automática: %s", agent)
	}

	// Crear backup si el archivo ya existe
	var backupPath string
	if _, err := os.Stat(configFile); err == nil {
		backupPath = configFile + ".backup." + time.Now().Format("20060102-150405")
		if err := copyFile(configFile, backupPath); err != nil {
			return "", fmt.Errorf("error creando backup: %w", err)
		}
	}

	// Escribir configuración
	if err := os.WriteFile(configFile, []byte(rtkContent), 0o644); err != nil {
		return "", fmt.Errorf("error escribiendo configuración: %w", err)
	}

	return backupPath, nil
}

func copyFile(src, dst string) error {
	sourceFile, err := os.Open(src)
	if err != nil {
		return err
	}
	defer sourceFile.Close()

	destFile, err := os.Create(dst)
	if err != nil {
		return err
	}
	defer destFile.Close()

	_, err = io.Copy(destFile, sourceFile)
	return err
}

// FormatResults formatea resultados como JSON o texto
func FormatResults(results []ConfigureResult, format string) error {
	if format == "json" {
		return json.NewEncoder(os.Stdout).Encode(results)
	}

	// Formato texto
	for _, r := range results {
		fmt.Printf("agent=%s status=%s rtk_required=%t\n", r.Agent, r.Status, r.RTKRequired)
		if r.ConfigPath != "" {
			fmt.Printf("  config_path=%s\n", r.ConfigPath)
		}
		if r.BackupPath != "" {
			fmt.Printf("  backup=%s\n", r.BackupPath)
		}
		if len(r.Actions) > 0 {
			fmt.Println("  actions:")
			for _, action := range r.Actions {
				fmt.Printf("    - %s\n", action)
			}
		}
		if r.Notes != "" {
			fmt.Printf("  notes: %s\n", r.Notes)
		}
		fmt.Println()
	}

	return nil
}
