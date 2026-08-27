package task

import (
	"bufio"
	"encoding/json"
	"os"
	"path/filepath"
	"time"
)

type Item struct {
	ID        string    `json:"id"`
	Title     string    `json:"title"`
	State     State     `json:"state"`
	Agent     string    `json:"agent,omitempty"`
	Model     string    `json:"model,omitempty"`
	Host      string    `json:"host,omitempty"`
	Evidence  string    `json:"evidence,omitempty"`
	CreatedAt time.Time `json:"created_at"`
	UpdatedAt time.Time `json:"updated_at"`
}

type Event struct {
	Time time.Time `json:"time"`
	Item Item      `json:"item"`
}

func DefaultPath() string {
	if stateHome := os.Getenv("XDG_STATE_HOME"); stateHome != "" {
		return filepath.Join(stateHome, "orq", "tasks.jsonl")
	}
	if home, err := os.UserHomeDir(); err == nil {
		return filepath.Join(home, ".local", "state", "orq", "tasks.jsonl")
	}
	return "tasks.jsonl"
}

func Create(path string, title string) (Item, error) {
	now := time.Now().UTC()
	item := Item{ID: now.Format("20060102-150405"), Title: title, State: Planned, CreatedAt: now, UpdatedAt: now}
	return item, appendEvent(path, item)
}

func Update(path string, id string, next State, agent string, model string, host string, evidence string) (Item, error) {
	items, err := List(path)
	if err != nil {
		return Item{}, err
	}
	var found Item
	for _, item := range items {
		if item.ID == id {
			found = item
			break
		}
	}
	if found.ID == "" {
		return Item{}, os.ErrNotExist
	}
	if err := ValidateTransition(found.State, next); err != nil {
		return Item{}, err
	}
	found.State = next
	if agent != "" {
		found.Agent = agent
	}
	if model != "" {
		found.Model = model
	}
	if host != "" {
		found.Host = host
	}
	if evidence != "" {
		found.Evidence = evidence
	}
	found.UpdatedAt = time.Now().UTC()
	return found, appendEvent(path, found)
}

func List(path string) ([]Item, error) {
	file, err := os.Open(path)
	if err != nil {
		if os.IsNotExist(err) {
			return nil, nil
		}
		return nil, err
	}
	defer file.Close()
	latest := map[string]Item{}
	scanner := bufio.NewScanner(file)
	for scanner.Scan() {
		var event Event
		if err := json.Unmarshal(scanner.Bytes(), &event); err != nil {
			return nil, err
		}
		latest[event.Item.ID] = event.Item
	}
	if err := scanner.Err(); err != nil {
		return nil, err
	}
	items := make([]Item, 0, len(latest))
	for _, item := range latest {
		items = append(items, item)
	}
	return items, nil
}

func appendEvent(path string, item Item) error {
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return err
	}
	file, err := os.OpenFile(path, os.O_CREATE|os.O_APPEND|os.O_WRONLY, 0o644)
	if err != nil {
		return err
	}
	defer file.Close()
	return json.NewEncoder(file).Encode(Event{Time: time.Now().UTC(), Item: item})
}
