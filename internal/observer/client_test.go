package observer

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"
)

func TestIngestSendsHostTokenAndEvents(t *testing.T) {
	var gotToken string
	var gotEvents []Event
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path != "/api/events/ingest" {
			t.Fatalf("unexpected path: %s", r.URL.Path)
		}
		gotToken = r.Header.Get("X-Host-Token")
		if err := json.NewDecoder(r.Body).Decode(&gotEvents); err != nil {
			t.Fatalf("decode request: %v", err)
		}
		_ = json.NewEncoder(w).Encode(IngestResult{OK: true, Inserted: 1})
	}))
	defer server.Close()

	client := New(server.URL, "secret")
	result, err := client.Ingest(context.Background(), []Event{SyntheticEvent("agent-orchestrator", "nvidia-api", "openai/gpt-oss-20b", 10, 2)})
	if err != nil {
		t.Fatalf("ingest: %v", err)
	}
	if gotToken != "secret" {
		t.Fatalf("token = %q", gotToken)
	}
	if len(gotEvents) != 1 || gotEvents[0].Project != "agent-orchestrator" {
		t.Fatalf("events = %+v", gotEvents)
	}
	if !result.OK || result.Inserted != 1 {
		t.Fatalf("result = %+v", result)
	}
}

func TestFromEnvMissingTokenIsDisabled(t *testing.T) {
	t.Setenv("ORQ_OBSERVER_HOST_TOKEN", "")
	t.Setenv("ORQ_OBSERVER_HOST_TOKEN_FILE", "/path/that/does/not/exist")
	_, ok, err := FromEnv()
	if err != nil {
		t.Fatalf("FromEnv error: %v", err)
	}
	if ok {
		t.Fatal("expected observer disabled without token")
	}
}
