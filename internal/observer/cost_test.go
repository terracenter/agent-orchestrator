package observer

import "testing"

func TestInterpretCostUsesConfiguredPlanForSubscriptionMetric(t *testing.T) {
	report := InterpretCost(97.152, "sub", 20, 1)
	if report.BillableRealUSD != 21 {
		t.Fatalf("BillableRealUSD = %v, want 21", report.BillableRealUSD)
	}
	if report.ExpectedInvoiceUSD != 21 {
		t.Fatalf("ExpectedInvoiceUSD = %v, want 21", report.ExpectedInvoiceUSD)
	}
	if report.ReportedEstimateUSD != 97.152 {
		t.Fatalf("ReportedEstimateUSD = %v, want 97.152", report.ReportedEstimateUSD)
	}
	if report.Warning == "" {
		t.Fatal("expected subscription warning")
	}
}
