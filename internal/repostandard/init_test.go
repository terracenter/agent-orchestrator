package repostandard

import "testing"

func TestInitRepoCreatesTemplateFiles(t *testing.T) {
	dir := t.TempDir()
	result, err := InitRepo(dir, TemplateData{ProjectName: "Demo"})
	if err != nil {
		t.Fatal(err)
	}
	if len(result.Created) == 0 {
		t.Fatal("expected created files")
	}
	report := CheckRepo(dir)
	if !report.Passed {
		t.Fatalf("expected generated repo to pass required checks: %+v", report.Checks)
	}
}

func TestInitRepoSkipsExistingFiles(t *testing.T) {
	dir := t.TempDir()
	if _, err := InitRepo(dir, TemplateData{ProjectName: "Demo"}); err != nil {
		t.Fatal(err)
	}
	result, err := InitRepo(dir, TemplateData{ProjectName: "Demo"})
	if err != nil {
		t.Fatal(err)
	}
	if len(result.Skipped) == 0 {
		t.Fatal("expected skipped files on second init")
	}
}
