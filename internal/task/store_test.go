package task

import (
	"errors"
	"os"
	"path/filepath"
	"testing"
)

func TestCreateUpdateList(t *testing.T) {
	path := filepath.Join(t.TempDir(), "tasks.jsonl")
	item, err := Create(path, "ordenar vault glpi")
	if err != nil {
		t.Fatalf("Create() error = %v", err)
	}
	updated, err := Update(path, item.ID, Assigned, "pi", "cheap", "minipc", "")
	if err != nil {
		t.Fatalf("Update() error = %v", err)
	}
	if updated.State != Assigned || updated.Agent != "pi" {
		t.Fatalf("updated = %+v", updated)
	}
	items, err := List(path)
	if err != nil {
		t.Fatalf("List() error = %v", err)
	}
	if len(items) != 1 || items[0].State != Assigned {
		t.Fatalf("items = %+v", items)
	}
}

func TestUpdateMissingTask(t *testing.T) {
	_, err := Update(filepath.Join(t.TempDir(), "tasks.jsonl"), "missing", Assigned, "", "", "", "")
	if !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("Update() error = %v, want ErrNotExist", err)
	}
}

func TestCreateIDsAreUnique(t *testing.T) {
	path := filepath.Join(t.TempDir(), "tasks.jsonl")
	if _, err := Create(path, "a"); err != nil {
		t.Fatal(err)
	}
	if _, err := Create(path, "b"); err != nil {
		t.Fatal(err)
	}
	items, err := List(path)
	if err != nil {
		t.Fatal(err)
	}
	if len(items) != 2 || items[0].ID == items[1].ID {
		t.Fatalf("items = %+v", items)
	}
}

func TestListReportsCorruptLine(t *testing.T) {
	path := filepath.Join(t.TempDir(), "tasks.jsonl")
	if err := os.WriteFile(path, []byte("not-json\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	if _, err := List(path); err == nil {
		t.Fatal("List() error = nil, want corrupt line error")
	}
}
