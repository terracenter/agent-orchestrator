package adapters

import (
	"context"
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"time"
)

type JSONLMemory struct {
	Path string
}

type memoryRecord struct {
	Time    time.Time `json:"time"`
	Key     string    `json:"key"`
	Content string    `json:"content"`
}

func (memory JSONLMemory) Save(_ context.Context, key string, content string) error {
	path := expandHome(memory.Path)
	if path == "" {
		path = expandHome("~/.local/state/orq/memory.jsonl")
	}
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return err
	}
	file, err := os.OpenFile(path, os.O_CREATE|os.O_APPEND|os.O_WRONLY, 0o644)
	if err != nil {
		return err
	}
	defer file.Close()
	return json.NewEncoder(file).Encode(memoryRecord{Time: time.Now().UTC(), Key: key, Content: content})
}

func (memory JSONLMemory) Search(_ context.Context, query string) ([]string, error) {
	path := expandHome(memory.Path)
	data, err := os.ReadFile(path)
	if err != nil {
		if os.IsNotExist(err) {
			return nil, nil
		}
		return nil, err
	}
	query = strings.ToLower(query)
	matches := []string{}
	for _, line := range strings.Split(strings.TrimSpace(string(data)), "\n") {
		if strings.Contains(strings.ToLower(line), query) {
			matches = append(matches, line)
		}
	}
	return matches, nil
}

func expandHome(path string) string {
	if strings.HasPrefix(path, "~/") {
		if home, err := os.UserHomeDir(); err == nil {
			return filepath.Join(home, strings.TrimPrefix(path, "~/"))
		}
	}
	return path
}
