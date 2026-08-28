package observer

import (
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

type IngestResult struct {
	OK       bool  `json:"ok"`
	Inserted int64 `json:"inserted"`
}

func New(baseURL, hostToken string) Client {
	return Client{BaseURL: strings.TrimRight(baseURL, "/"), HostToken: strings.TrimSpace(hostToken), HTTPClient: &http.Client{Timeout: 10 * time.Second}}
}

func FromEnv() (Client, bool, error) {
	baseURL := os.Getenv("ORQ_OBSERVER_URL")
	if baseURL == "" {
		baseURL = "http://127.0.0.1:4000"
	}
	token := os.Getenv("ORQ_OBSERVER_HOST_TOKEN")
	if token == "" {
		path := os.Getenv("ORQ_OBSERVER_HOST_TOKEN_FILE")
		if path == "" {
			path = os.ExpandEnv("$HOME/.config/sge-observer/agent-orchestrator.host-token")
		}
		data, err := os.ReadFile(path)
		if err != nil {
			return Client{}, false, nil
		}
		token = strings.TrimSpace(string(data))
	}
	if strings.TrimSpace(token) == "" {
		return Client{}, false, nil
	}
	return New(baseURL, token), true, nil
}

func SyntheticEvent(project, agent, model string, tokensIn, tokensOut int64) Event {
	now := time.Now().UTC()
	host, _ := os.Hostname()
	seed := fmt.Sprintf("%s|%s|%s|%d", project, agent, model, now.UnixNano())
	sum := sha256.Sum256([]byte(seed))
	return Event{
		EventID:    "orq-" + hex.EncodeToString(sum[:])[:24],
		Host:       host,
		Agent:      agent,
		Model:      model,
		Project:    project,
		SessionID:  "orq-manual",
		EventType:  "orq_synthetic_usage",
		TokensIn:   tokensIn,
		TokensOut:  tokensOut,
		CreatedAt:  now,
		SourcePath: "orq observer send-test",
		Raw:        "{}",
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
