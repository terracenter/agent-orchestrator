package handoff

import (
	"fmt"
	"strings"
	"time"

	"github.com/terracenter/agent-orchestrator/internal/route"
	"github.com/terracenter/agent-orchestrator/internal/task"
)

var templateRoles = map[string]string{
	"reviewer-4r":       "Revisor 4R: evalúa Legibilidad, Robustez, Riesgo y Seguridad.",
	"security-reviewer": "Revisor de seguridad: valida entradas, permisos, secretos y acciones peligrosas.",
	"implementer":       "Implementador: aplica cambios mínimos, verificables y reversibles.",
	"documenter":        "Documentador: mantiene ES-VE principal y EN-US secundario cuando aplique.",
	"architect":         "Arquitecto: analiza decisiones, trade-offs, riesgos y compatibilidad.",
}

func TemplateNames() []string {
	return []string{"reviewer-4r", "security-reviewer", "implementer", "documenter", "architect"}
}

func DraftWithTemplate(item task.Item, template string) (string, error) {
	if template == "" || template == "default" {
		return Draft(item), nil
	}
	role, ok := templateRoles[template]
	if !ok {
		return "", fmt.Errorf("unknown handoff template %q", template)
	}
	decision := route.Decide(item.Title)
	agent := value(item.Agent, decision.RecommendedAgent)
	model := value(item.Model, decision.RecommendedModel)
	provider := providerForAgent(agent)
	return strings.TrimSpace(fmt.Sprintf(`# HANDOFF TEMPLATE: %s

<contexto_estatico>
Rol: %s
Reglas permanentes:
- Seguridad primero; funcionamiento/optimización segundo; UX/visual tercero.
- orq es autoridad para guardias, seguridad, recibos y validación.
- Usar rtk para shell/git/fs; usar vg para vault/grafo.
- No usar --dangerously-skip-permissions.
- No tocar producción, secretos, DB, DNS, firewall ni acciones irreversibles sin aprobación humana explícita.
- RDD: no declarar como validado un comando que no fue ejecutado.
</contexto_estatico>

<contexto_estable>
Task ID: %s
Estado: %s
Agente: %s
Provider: %s
Modelo: %s
Host: %s
</contexto_estable>

<contexto_dinamico>
Generado: %s
Título: %s
Branch/PR/archivos cambiados: completar con evidencia actual antes de ejecutar.
Errores actuales: completar solo si existen.
</contexto_dinamico>

<tarea>
%s

Formato de salida requerido:
- Resumen breve.
- Comandos ejecutados y resultado real.
- Evidencia verificable.
- Riesgos/bloqueos.
- Rollback propuesto.
</tarea>
`, template, role, item.ID, item.State, agent, provider, model, item.Host, time.Now().UTC().Format(time.RFC3339), item.Title, item.Title)) + "\n", nil
}

func providerForAgent(agent string) string {
	switch strings.ToLower(agent) {
	case "agy":
		return "google/nvidia/local según modelo configurado"
	case "pi":
		return "pi"
	case "claude", "claude-code":
		return "anthropic"
	case "hermes":
		return "hermes"
	case "nvidia-api":
		return "nvidia"
	default:
		return "local"
	}
}
