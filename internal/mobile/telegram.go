package mobile

import (
	"strings"
	"time"

	"github.com/terracenter/agent-orchestrator/internal/route"
)

type Message struct {
	UserID int64
	Text   string
	At     time.Time
}

type AuditEntry struct {
	UserID        int64     `json:"user_id"`
	Command       string    `json:"command"`
	Authorized    bool      `json:"authorized"`
	RequiresWrite bool      `json:"requires_write"`
	ModelAssigned string    `json:"model_assigned"`
	CostEstimated string    `json:"cost_estimated"`
	Result        string    `json:"result"`
	CreatedAt     time.Time `json:"created_at"`
}

type Response struct {
	Text                 string
	ReadOnly             bool
	RequiresConfirmation bool
	Rejected             bool
	Audit                AuditEntry
}

type Router struct {
	AllowedUserIDs map[int64]bool
	Now            func() time.Time
}

func NewRouter(allowed []int64) Router {
	m := make(map[int64]bool, len(allowed))
	for _, id := range allowed {
		m[id] = true
	}
	return Router{AllowedUserIDs: m, Now: func() time.Time { return time.Now().UTC() }}
}

func (r Router) Handle(msg Message) Response {
	now := msg.At
	if now.IsZero() {
		if r.Now != nil {
			now = r.Now()
		} else {
			now = time.Now().UTC()
		}
	}
	cmd := commandName(msg.Text)
	audit := AuditEntry{UserID: msg.UserID, Command: cmd, CreatedAt: now, CostEstimated: "low"}
	if !r.AllowedUserIDs[msg.UserID] {
		audit.Result = "rejected_unauthorized"
		return Response{Text: "rechazado: usuario no autorizado", Rejected: true, Audit: audit}
	}
	audit.Authorized = true
	if isReadOnly(cmd) {
		audit.Result = "allowed_read_only"
		return Response{Text: readOnlyResponse(cmd), ReadOnly: true, Audit: audit}
	}
	if isWrite(cmd) {
		audit.RequiresWrite = true
		decision := route.Decide(msg.Text)
		audit.ModelAssigned = cheapMobileModel(decision)
		audit.Result = "confirmation_required"
		return Response{Text: "confirmacion requerida: responder /confirm para ejecutar", RequiresConfirmation: true, Audit: audit}
	}
	audit.Result = "rejected_unknown_command"
	return Response{Text: "comando no permitido en modo movil seguro", Rejected: true, Audit: audit}
}

func commandName(text string) string {
	fields := strings.Fields(strings.TrimSpace(text))
	if len(fields) == 0 {
		return ""
	}
	return strings.ToLower(fields[0])
}

func isReadOnly(cmd string) bool {
	switch cmd {
	case "/status", "/agents", "/route", "/task_list", "/task_show":
		return true
	default:
		return false
	}
}

func isWrite(cmd string) bool {
	switch cmd {
	case "/task_create", "/task_update", "/task_assign", "/handoff_draft", "/supervise_start", "/supervise_stop":
		return true
	default:
		return false
	}
}

func readOnlyResponse(cmd string) string {
	return "ok read-only: " + strings.TrimPrefix(cmd, "/")
}

func cheapMobileModel(decision route.Decision) string {
	for _, allowed := range decision.AllowedAgents {
		if strings.HasPrefix(allowed, "nvidia-api/openai/gpt-oss-20b") || strings.HasPrefix(allowed, "agy/gpt-oss-120b-medium") || strings.HasPrefix(allowed, "agy/gemini-3.5-flash-low") {
			return allowed
		}
	}
	return "nvidia-api/openai/gpt-oss-20b"
}
