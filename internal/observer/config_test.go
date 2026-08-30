package observer

import (
	"os"
	"path/filepath"
	"testing"
)

func TestLoadConfigReadsClientEnvAndTokenFile(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)
	t.Setenv("ORQ_OBSERVER_URL", "")
	t.Setenv("ORQ_OBSERVER_HOST_TOKEN", "")
	t.Setenv("ORQ_OBSERVER_HOST_TOKEN_FILE", "")

	configDir := filepath.Join(home, ".config", "sge-observer")
	if err := os.MkdirAll(configDir, 0o700); err != nil {
		t.Fatalf("mkdir config dir: %v", err)
	}
	tokenPath := filepath.Join(configDir, "prod.host-token")
	if err := os.WriteFile(tokenPath, []byte("secret-token\n"), 0o600); err != nil {
		t.Fatalf("write token: %v", err)
	}
	config := "ORQ_OBSERVER_URL=\"https://observer.example.test\"\nORQ_OBSERVER_HOST_TOKEN_FILE=\"$HOME/.config/sge-observer/prod.host-token\"\n"
	if err := os.WriteFile(filepath.Join(configDir, "client.env"), []byte(config), 0o600); err != nil {
		t.Fatalf("write client env: %v", err)
	}

	cfg, token, err := LoadConfig()
	if err != nil {
		t.Fatalf("LoadConfig: %v", err)
	}
	if cfg.BaseURL != "https://observer.example.test" {
		t.Fatalf("BaseURL = %q", cfg.BaseURL)
	}
	if cfg.TokenFile != tokenPath {
		t.Fatalf("TokenFile = %q", cfg.TokenFile)
	}
	if token != "secret-token" {
		t.Fatalf("token was not loaded from file")
	}
	if !cfg.Configured || !cfg.TokenLoaded || cfg.TokenSource != "file" {
		t.Fatalf("cfg = %+v", cfg)
	}
}

func TestLoadConfigEnvOverridesClientEnv(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)
	t.Setenv("ORQ_OBSERVER_URL", "https://env.example.test")
	t.Setenv("ORQ_OBSERVER_HOST_TOKEN", "env-token")
	t.Setenv("ORQ_OBSERVER_HOST_TOKEN_FILE", "")

	configDir := filepath.Join(home, ".config", "sge-observer")
	if err := os.MkdirAll(configDir, 0o700); err != nil {
		t.Fatalf("mkdir config dir: %v", err)
	}
	if err := os.WriteFile(filepath.Join(configDir, "client.env"), []byte("ORQ_OBSERVER_URL=https://file.example.test\n"), 0o600); err != nil {
		t.Fatalf("write client env: %v", err)
	}

	cfg, token, err := LoadConfig()
	if err != nil {
		t.Fatalf("LoadConfig: %v", err)
	}
	if cfg.BaseURL != "https://env.example.test" || token != "env-token" || cfg.TokenSource != "env" {
		t.Fatalf("cfg=%+v token=%q", cfg, token)
	}
}
