package trace

import (
	"bufio"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"time"

	"github.com/google/uuid"
)

// Manager maneja sesiones y eventos de trace.
type Manager struct {
	stateDir string
}

// NewManager crea un nuevo Manager con directorio de estado.
func NewManager(stateDir string) *Manager {
	if stateDir == "" {
		stateDir = DefaultStateDir()
	}
	return &Manager{stateDir: stateDir}
}

// DefaultStateDir retorna directorio default para state (~/.local/state/orq/traces).
func DefaultStateDir() string {
	state := os.Getenv("XDG_STATE_HOME")
	if state == "" {
		home, err := os.UserHomeDir()
		if err != nil || home == "" {
			return filepath.Join(".", "orq", "traces")
		}
		state = filepath.Join(home, ".local", "state")
	}
	return filepath.Join(state, "orq", "traces")
}

// Start inicia una nueva sesión de trace.
func (m *Manager) Start(metadata TraceMetadata) (*TraceSession, error) {
	sessionID := uuid.New().String()

	// Crear directorio si no existe
	if err := os.MkdirAll(m.stateDir, 0o755); err != nil {
		return nil, fmt.Errorf("crear directorio traces: %w", err)
	}

	session := &TraceSession{
		ID:        sessionID,
		Agent:     metadata.Agent,
		Host:      metadata.Host,
		Workspace: metadata.Workspace,
		Model:     metadata.Model,
		Status:    "active",
		StartedAt: time.Now().UTC(),
		EventCount: 0,
		Description: metadata.Description,
		Metadata:   metadata.Metadata,
	}

	// Guardar metadatos de sesión en archivo
	sessionPath := filepath.Join(m.stateDir, sessionID+".session.json")
	data, err := json.MarshalIndent(session, "", "  ")
	if err != nil {
		return nil, fmt.Errorf("serializar sesión: %w", err)
	}
	if err := os.WriteFile(sessionPath, data, 0o644); err != nil {
		return nil, fmt.Errorf("escribir sesión: %w", err)
	}

	return session, nil
}

// Record agrega un evento a una sesión.
func (m *Manager) Record(sessionID string, event TraceEvent) error {
	if event.SessionID == "" {
		event.SessionID = sessionID
	}
	if event.Timestamp.IsZero() {
		event.Timestamp = time.Now().UTC()
	}

	// Append a JSONL
	eventsPath := filepath.Join(m.stateDir, sessionID+".jsonl")
	f, err := os.OpenFile(eventsPath, os.O_APPEND|os.O_CREATE|os.O_WRONLY, 0o644)
	if err != nil {
		return fmt.Errorf("abrir archivo eventos: %w", err)
	}
	defer f.Close()

	data, err := json.Marshal(event)
	if err != nil {
		return fmt.Errorf("serializar evento: %w", err)
	}

	_, err = f.WriteString(string(data) + "\n")
	if err != nil {
		return fmt.Errorf("escribir evento: %w", err)
	}

	// Actualizar event count en session
	return m.updateEventCount(sessionID)
}

// Status retorna el estado de una sesión.
func (m *Manager) Status(sessionID string) (*TraceSession, []TraceEvent, error) {
	sessionPath := filepath.Join(m.stateDir, sessionID+".session.json")
	sessionData, err := os.ReadFile(sessionPath)
	if err != nil {
		return nil, nil, fmt.Errorf("leer sesión: %w", err)
	}

	var session TraceSession
	if err := json.Unmarshal(sessionData, &session); err != nil {
		return nil, nil, fmt.Errorf("parsear sesión: %w", err)
	}

	// Leer eventos
	eventsPath := filepath.Join(m.stateDir, sessionID+".jsonl")
	events, err := m.readEvents(eventsPath)
	if err != nil && !os.IsNotExist(err) {
		return nil, nil, fmt.Errorf("leer eventos: %w", err)
	}

	return &session, events, nil
}

// Stop finaliza una sesión.
func (m *Manager) Stop(sessionID string) error {
	sessionPath := filepath.Join(m.stateDir, sessionID+".session.json")
	sessionData, err := os.ReadFile(sessionPath)
	if err != nil {
		return fmt.Errorf("leer sesión: %w", err)
	}

	var session TraceSession
	if err := json.Unmarshal(sessionData, &session); err != nil {
		return fmt.Errorf("parsear sesión: %w", err)
	}

	now := time.Now().UTC()
	session.StoppedAt = &now
	session.Status = "stopped"

	data, err := json.MarshalIndent(session, "", "  ")
	if err != nil {
		return fmt.Errorf("serializar sesión: %w", err)
	}
	return os.WriteFile(sessionPath, data, 0o644)
}

// List retorna todas las sesiones.
func (m *Manager) List() ([]TraceSession, error) {
	entries, err := os.ReadDir(m.stateDir)
	if err != nil {
		if os.IsNotExist(err) {
			return []TraceSession{}, nil
		}
		return nil, fmt.Errorf("leer directorio: %w", err)
	}

	var sessions []TraceSession
	for _, entry := range entries {
		if !entry.IsDir() && filepath.Ext(entry.Name()) == ".json" && filepath.Ext(entry.Name()[:len(entry.Name())-5]) == ".session" {
			path := filepath.Join(m.stateDir, entry.Name())
			data, err := os.ReadFile(path)
			if err != nil {
				continue
			}
			var session TraceSession
			if err := json.Unmarshal(data, &session); err != nil {
				continue
			}
			sessions = append(sessions, session)
		}
	}
	return sessions, nil
}

// Private helpers

func (m *Manager) updateEventCount(sessionID string) error {
	sessionPath := filepath.Join(m.stateDir, sessionID+".session.json")
	sessionData, err := os.ReadFile(sessionPath)
	if err != nil {
		return err
	}

	var session TraceSession
	if err := json.Unmarshal(sessionData, &session); err != nil {
		return err
	}

	// Contar eventos
	eventsPath := filepath.Join(m.stateDir, sessionID+".jsonl")
	events, _ := m.readEvents(eventsPath)
	session.EventCount = len(events)

	data, err := json.MarshalIndent(session, "", "  ")
	if err != nil {
		return err
	}
	return os.WriteFile(sessionPath, data, 0o644)
}

func (m *Manager) readEvents(path string) ([]TraceEvent, error) {
	f, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer f.Close()

	var events []TraceEvent
	scanner := bufio.NewScanner(f)
	for scanner.Scan() {
		var ev TraceEvent
		if err := json.Unmarshal(scanner.Bytes(), &ev); err != nil {
			continue
		}
		events = append(events, ev)
	}
	return events, scanner.Err()
}
