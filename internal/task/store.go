package task

import (
	"bufio"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"syscall"
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
	CostLevel int       `json:"cost_level,omitempty"`
	Reason    string    `json:"reason,omitempty"`
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
	unlock, err := lockStore(path)
	if err != nil {
		return Item{}, err
	}
	defer unlock()
	now := time.Now().UTC()
	item := Item{ID: now.Format("20060102-150405.000000000"), Title: title, State: Planned, CreatedAt: now, UpdatedAt: now}
	return item, appendEventUnlocked(path, item)
}

func Get(path string, id string) (Item, error) {
	items, err := List(path)
	if err != nil {
		return Item{}, err
	}
	for _, item := range items {
		if item.ID == id {
			return item, nil
		}
	}
	return Item{}, os.ErrNotExist
}

func Assign(path string, id string, agent string, model string, host string, costLevel int, reason string) (Item, error) {
	item, err := Update(path, id, Assigned, agent, model, host, "")
	if err != nil {
		return Item{}, err
	}
	item.CostLevel = costLevel
	item.Reason = reason
	return appendAssigned(path, item)
}

func Update(path string, id string, next State, agent string, model string, host string, evidence string) (Item, error) {
	unlock, err := lockStore(path)
	if err != nil {
		return Item{}, err
	}
	defer unlock()
	found, err := getUnlocked(path, id)
	if err != nil {
		return Item{}, err
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
	return found, appendEventUnlocked(path, found)
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
	return readEvents(file)
}

func getUnlocked(path string, id string) (Item, error) {
	items, err := List(path)
	if err != nil {
		return Item{}, err
	}
	for _, item := range items {
		if item.ID == id {
			return item, nil
		}
	}
	return Item{}, fmt.Errorf("task %q: %w", id, os.ErrNotExist)
}

func readEvents(r io.Reader) ([]Item, error) {
	latest := map[string]Item{}
	reader := bufio.NewReader(r)
	lineNo := 0
	for {
		line, err := reader.ReadString('\n')
		if err != nil && !errors.Is(err, io.EOF) {
			return nil, err
		}
		line = strings.TrimSpace(line)
		if line != "" {
			lineNo++
			var event Event
			if err := json.Unmarshal([]byte(line), &event); err != nil {
				return nil, fmt.Errorf("tasks store line %d: %w", lineNo, err)
			}
			latest[event.Item.ID] = event.Item
		}
		if errors.Is(err, io.EOF) {
			break
		}
	}
	items := make([]Item, 0, len(latest))
	for _, item := range latest {
		items = append(items, item)
	}
	sort.Slice(items, func(i, j int) bool { return items[i].ID < items[j].ID })
	return items, nil
}

func appendAssigned(path string, item Item) (Item, error) {
	unlock, err := lockStore(path)
	if err != nil {
		return Item{}, err
	}
	defer unlock()
	item.UpdatedAt = time.Now().UTC()
	return item, appendEventUnlocked(path, item)
}

func appendEvent(path string, item Item) error {
	unlock, err := lockStore(path)
	if err != nil {
		return err
	}
	defer unlock()
	return appendEventUnlocked(path, item)
}

func appendEventUnlocked(path string, item Item) error {
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

func lockStore(path string) (func(), error) {
	lockPath := path + ".lock"
	if err := os.MkdirAll(filepath.Dir(lockPath), 0o755); err != nil {
		return nil, err
	}
	file, err := os.OpenFile(lockPath, os.O_CREATE|os.O_RDWR, 0o644)
	if err != nil {
		return nil, err
	}
	if err := syscall.Flock(int(file.Fd()), syscall.LOCK_EX); err != nil {
		file.Close()
		return nil, err
	}
	return func() {
		_ = syscall.Flock(int(file.Fd()), syscall.LOCK_UN)
		_ = file.Close()
	}, nil
}
