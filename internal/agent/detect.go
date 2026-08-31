package agent

import (
	"os"
	"os/exec"
	"path/filepath"
)

type AgentDetection struct {
	Agent      string `json:"agent"`
	Installed  bool   `json:"installed"`
	BinaryPath string `json:"binary_path,omitempty"`
	ConfigPath string `json:"config_path,omitempty"`
	Role       string `json:"role"`
	CostLevel  int    `json:"cost_level"`
	ReviewOnly bool   `json:"review_only"`
	Verified   bool   `json:"verified"`
	Notes      string `json:"notes,omitempty"`
}

func DetectAgents() []AgentDetection {
	home, _ := os.UserHomeDir()
	return DetectAgentsWithHome(home)
}

func DetectAgentsWithHome(home string) []AgentDetection {
	var detections []AgentDetection

	// 1. OpenClaw
	openclawBin := findBin("openclaw", []string{
		filepath.Join(home, ".local", "bin"),
		filepath.Join(home, ".local", "share", "pi-node", "node-v22.23.2-linux-x64", "bin"),
	})
	openclawCfg := findDir(filepath.Join(home, ".openclaw"), filepath.Join(home, ".config", "openclaw"))
	detections = append(detections, AgentDetection{
		Agent:      "openclaw",
		Installed:  openclawBin != "",
		BinaryPath: openclawBin,
		ConfigPath: openclawCfg,
		Role:       "runner economico para tareas mecanicas y clasificacion cerrada (Haiku)",
		CostLevel:  1,
		ReviewOnly: false,
		Verified:   true,
		Notes:      "inspeccion segura: no modifica credenciales ni estado de OpenClaw",
	})

	// 2. AGY (Antigravity CLI)
	agyBin := findBin("agy", []string{filepath.Join(home, ".local", "bin")})
	agyCfg := findDir(filepath.Join(home, ".gemini"))
	detections = append(detections, AgentDetection{
		Agent:      "agy",
		Installed:  agyBin != "",
		BinaryPath: agyBin,
		ConfigPath: agyCfg,
		Role:       "runner rapido para implementacion de codigo y analisis tecnico medio",
		CostLevel:  1,
		ReviewOnly: false,
		Verified:   true,
		Notes:      "Antigravity CLI para ejecucion de tareas tecnicas y modelos flash/open",
	})

	// 3. Hermes
	hermesBin := findBin("hermes", []string{filepath.Join(home, ".local", "bin")})
	hermesCfg := findDir(filepath.Join(home, ".hermes"))
	detections = append(detections, AgentDetection{
		Agent:      "hermes",
		Installed:  hermesBin != "",
		BinaryPath: hermesBin,
		ConfigPath: hermesCfg,
		Role:       "runner para tareas de integracion y exploracion",
		CostLevel:  1,
		ReviewOnly: false,
		Verified:   true,
	})

	// 4. Claude Code
	claudeBin := findBin("claude", []string{filepath.Join(home, ".local", "bin")})
	claudeCfg := findDir(filepath.Join(home, ".claude"))
	detections = append(detections, AgentDetection{
		Agent:      "claude-code",
		Installed:  claudeBin != "",
		BinaryPath: claudeBin,
		ConfigPath: claudeCfg,
		Role:       "revision critica, seguridad, bloqueos y refutacion de decisiones",
		CostLevel:  3,
		ReviewOnly: true,
		Verified:   true,
		Notes:      "politica estricta: solo revision, no ejecucion de tareas mecanicas",
	})

	// 5. Pi
	piBin := findBin("pi", []string{filepath.Join(home, ".local", "bin")})
	detections = append(detections, AgentDetection{
		Agent:      "pi",
		Installed:  piBin != "",
		BinaryPath: piBin,
		Role:       "orquestacion principal y sintesis de decisiones",
		CostLevel:  2,
		ReviewOnly: false,
		Verified:   true,
		Notes:      "supervisor principal; detener y delegar si el presupuesto esta tensionado",
	})

	// 6. Qwen Code
	qwenBin := findBin("qwen", []string{filepath.Join(home, ".local", "bin")})
	qwenCfg := findDir(filepath.Join(home, ".qwen"), filepath.Join(home, ".config", "qwen"))
	detections = append(detections, AgentDetection{
		Agent:      "qwen-code",
		Installed:  qwenBin != "",
		BinaryPath: qwenBin,
		ConfigPath: qwenCfg,
		Role:       "runner multi-modelo para codigo, busqueda en repos, shell/git/docker y tareas mecanicas o tecnicas",
		CostLevel:  1,
		ReviewOnly: false,
		Verified:   qwenBin != "" || qwenCfg != "",
		Notes:      "deteccion segura: solo valida binario/directorio de configuracion; no lee settings ni secretos",
	})

	// 7. Codex
	codexBin := findBin("codex", []string{filepath.Join(home, ".local", "bin")})
	detections = append(detections, AgentDetection{
		Agent:      "codex",
		Installed:  codexBin != "",
		BinaryPath: codexBin,
		Role:       "runner y asistente secundario",
		CostLevel:  2,
		ReviewOnly: false,
		Verified:   false,
	})

	// 8. NVIDIA API
	nvidiaKey := os.Getenv("NVIDIA_API_KEY")
	detections = append(detections, AgentDetection{
		Agent:      "nvidia-api",
		Installed:  nvidiaKey != "",
		Role:       "smoke tests, clasificacion barata y tareas mecanicas con API NVIDIA",
		CostLevel:  0,
		ReviewOnly: false,
		Verified:   true,
		Notes:      "costo cero / minimo para tareas mecanicas",
	})

	// TODO(minipc-local): Diseñar e implementar detección remota o registro de modelos locales en minipc.
	// Dado que el host de desarrollo actual no cuenta con binarios locales de inferencia
	// (como ollama, llama-server, etc.), la detección debe validar la existencia y conectividad
	// vía SSH/Tailscale al host documentado minipc (100.76.175.78),
	// respetando la aprobación previa y la integridad de credenciales.
	// Referencia: docs/agent-model-capabilities.md y Obsidian/03.Servidores/Humanbyte/estaciones-personales.md.

	return detections
}

func findBin(name string, extraDirs []string) string {
	for _, dir := range extraDirs {
		candidate := filepath.Join(dir, name)
		if fi, err := os.Stat(candidate); err == nil && !fi.IsDir() && fi.Mode()&0o111 != 0 {
			return candidate
		}
	}
	if p, err := exec.LookPath(name); err == nil && p != "" {
		return p
	}
	return ""
}

func findDir(paths ...string) string {
	for _, p := range paths {
		if fi, err := os.Stat(p); err == nil && fi.IsDir() {
			return p
		}
	}
	return ""
}
