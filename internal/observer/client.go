package observer

import (
	"bufio"
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"net/http"
	"os"
	"strings"
	"time"
)

type Event struct {
	EventID       string    `json:"event_id"`
	Host          string    `json:"host"`
	Agent         string    `json:"agent"`
	Model         string    `json:"model"`
	Project       string    `json:"project"`
	SessionID     string    `json:"session_id"`
	EventType     string    `json:"event_type"`
	TokensIn      int64     `json:"tokens_in"`
	TokensOut     int64     `json:"tokens_out"`
	CostEstimated float64   `json:"cost_estimated"`
	DurationMs    int64     `json:"duration_ms"`
	CreatedAt     time.Time `json:"created_at"`
	SourcePath    string    `json:"source_path"`
	Raw           string    `json:"raw"`
}

type Client struct {
	BaseURL    string
	HostToken  string
	HTTPClient *http.Client
}

type Config struct {
	BaseURL     string `json:"base_url"`
	TokenFile   string `json:"token_file"`
	TokenSource string `json:"token_source"`
	ConfigFile  string `json:"config_file"`
	Configured  bool   `json:"configured"`
	TokenLoaded bool   `json:"token_loaded"`
}

type IngestResult struct {
	OK       bool  `json:"ok"`
	Inserted int64 `json:"inserted"`
}

func New(baseURL, hostToken string) Client {
	return Client{BaseURL: strings.TrimRight(baseURL, "/"), HostToken: strings.TrimSpace(hostToken), HTTPClient: &http.Client{Timeout: 10 * time.Second}}
}

func FromEnv() (Client, bool, error) {
	cfg, token, err := LoadConfig()
	if err != nil {
		return Client{}, false, err
	}
	if !cfg.TokenLoaded || strings.TrimSpace(token) == "" {
		return Client{}, false, nil
	}
	return New(cfg.BaseURL, token), true, nil
}

func DefaultConfigPath() string {
	return os.ExpandEnv("$HOME/.config/sge-observer/client.env")
}

func DefaultTokenPath() string {
	return os.ExpandEnv("$HOME/.config/sge-observer/agent-orchestrator.host-token")
}

func LoadConfig() (Config, string, error) {
	cfg := Config{BaseURL: "http://127.0.0.1:4000", TokenFile: DefaultTokenPath(), ConfigFile: DefaultConfigPath()}
	values, configExists, err := readEnvFile(cfg.ConfigFile)
	if err != nil {
		return cfg, "", err
	}
	if configExists {
		cfg.Configured = true
	}
	if v := values["ORQ_OBSERVER_URL"]; v != "" {
		cfg.BaseURL = v
	}
	if v := values["ORQ_OBSERVER_HOST_TOKEN_FILE"]; v != "" {
		cfg.TokenFile = os.ExpandEnv(v)
	}
	token := values["ORQ_OBSERVER_HOST_TOKEN"]
	if v := os.Getenv("ORQ_OBSERVER_URL"); v != "" {
		cfg.BaseURL = v
	}
	if v := os.Getenv("ORQ_OBSERVER_HOST_TOKEN_FILE"); v != "" {
		cfg.TokenFile = os.ExpandEnv(v)
	}
	if v := os.Getenv("ORQ_OBSERVER_HOST_TOKEN"); v != "" {
		token = v
	}
	if strings.TrimSpace(token) != "" {
		cfg.TokenSource = "env"
		cfg.TokenLoaded = true
		return cfg, strings.TrimSpace(token), nil
	}
	data, err := os.ReadFile(cfg.TokenFile)
	if err != nil {
		return cfg, "", nil
	}
	token = strings.TrimSpace(string(data))
	if token != "" {
		cfg.TokenSource = "file"
		cfg.TokenLoaded = true
	}
	return cfg, token, nil
}

func readEnvFile(path string) (map[string]string, bool, error) {
	values := map[string]string{}
	file, err := os.Open(path)
	if os.IsNotExist(err) {
		return values, false, nil
	}
	if err != nil {
		return values, false, err
	}
	defer file.Close()
	scanner := bufio.NewScanner(file)
	for scanner.Scan() {
		line := strings.TrimSpace(scanner.Text())
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		key, val, ok := strings.Cut(line, "=")
		if !ok {
			continue
		}
		key = strings.TrimSpace(key)
		val = strings.Trim(strings.TrimSpace(val), "\"'")
		values[key] = os.ExpandEnv(val)
	}
	return values, true, scanner.Err()
}

func SyntheticEvent(project, agent, model string, tokensIn, tokensOut int64) Event {
	return NewEvent(project, agent, model, "orq-manual", "orq_synthetic_usage", tokensIn, tokensOut, "orq observer send-test", "{}")
}

func NewEvent(project, agent, model, sessionID, eventType string, tokensIn, tokensOut int64, sourcePath, raw string) Event {
	now := time.Now().UTC()
	host, _ := os.Hostname()
	seed := fmt.Sprintf("%s|%s|%s|%s|%d", project, agent, model, eventType, now.UnixNano())
	sum := sha256.Sum256([]byte(seed))
	return Event{
		EventID:    "orq-" + hex.EncodeToString(sum[:])[:24],
		Host:       host,
		Agent:      agent,
		Model:      model,
		Project:    project,
		SessionID:  sessionID,
		EventType:  eventType,
		TokensIn:   tokensIn,
		TokensOut:  tokensOut,
		CreatedAt:  now,
		SourcePath: sourcePath,
		Raw:        raw,
	}
}

func (c Client) Ingest(ctx context.Context, events []Event) (IngestResult, error) {
	if c.BaseURL == "" {
		return IngestResult{}, fmt.Errorf("observer base URL is required")
	}
	if c.HostToken == "" {
		return IngestResult{}, fmt.Errorf("observer host token is required")
	}
	body, err := json.Marshal(events)
	if err != nil {
		return IngestResult{}, err
	}
	req, err := http.NewRequestWithContext(ctx, http.MethodPost, c.BaseURL+"/api/events/ingest", bytes.NewReader(body))
	if err != nil {
		return IngestResult{}, err
	}
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("X-Host-Token", c.HostToken)
	client := c.HTTPClient
	if client == nil {
		client = http.DefaultClient
	}
	resp, err := client.Do(req)
	if err != nil {
		return IngestResult{}, err
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		return IngestResult{}, fmt.Errorf("observer ingest returned %s", resp.Status)
	}
	var result IngestResult
	if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
		return IngestResult{}, err
	}
	return result, nil
}
