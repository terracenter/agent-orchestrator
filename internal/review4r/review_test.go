package review4r

import "testing"

func TestBuildReturnsFourItems(t *testing.T) {
	report := Build(t.TempDir())
	if len(report.Items) != 4 {
		t.Fatalf("items=%d", len(report.Items))
	}
	want := []string{"Legibilidad", "Robustez", "Riesgo", "Seguridad"}
	for i, area := range want {
		if report.Items[i].Area != area {
			t.Fatalf("item %d area=%q", i, report.Items[i].Area)
		}
	}
}
