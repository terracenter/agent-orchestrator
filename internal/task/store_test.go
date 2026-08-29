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
	updated, err := Assign(path, item.ID, "pi", "gpt-5.5", "minipc", 1, "tarea mecanica")
	if err != nil {
		t.Fatalf("Update() error = %v", err)
	}
	if updated.State != Assigned || updated.Agent != "pi" || updated.CostLevel != 1 {
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

func TestNextReturnsHighestPriorityOpenTask(t *testing.T) {
	path := filepath.Join(t.TempDir(), "tasks.jsonl")
	planned, err := Create(path, "planned task")
	if err != nil {
		t.Fatal(err)
	}
	blocked, err := Create(path, "blocked task")
	if err != nil {
		t.Fatal(err)
	}
	if _, err := Update(path, blocked.ID, Blocked, "", "", "", "needs human input"); err != nil {
		t.Fatal(err)
	}
	item, ok, err := Next(path)
	if err != nil {
		t.Fatal(err)
	}
	if !ok || item.ID != blocked.ID || item.ID == planned.ID {
		t.Fatalf("Next() = %+v ok=%v", item, ok)
	}
}

func TestNextSkipsMergedTasks(t *testing.T) {
	path := filepath.Join(t.TempDir(), "tasks.jsonl")
	item, err := Create(path, "closed task")
	if err != nil {
		t.Fatal(err)
	}
	for _, state := range []State{Assigned, Running, Done, Verified, Merged} {
		if _, err := Update(path, item.ID, state, "", "", "", "evidence"); err != nil {
			t.Fatal(err)
		}
	}
	_, ok, err := Next(path)
	if err != nil {
		t.Fatal(err)
	}
	if ok {
		t.Fatal("Next() ok = true, want false")
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
