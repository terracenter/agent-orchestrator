package observer

import "strings"

// CostReport separates provider/token estimates from the user's real billing plan.
type CostReport struct {
	ReportedEstimateUSD float64 `json:"reported_estimate_usd"`
	ReportedLabel       string  `json:"reported_label"`
	MonthlyPlanUSD      float64 `json:"monthly_plan_usd"`
	PaymentFeeUSD       float64 `json:"payment_fee_usd"`
	ExpectedInvoiceUSD  float64 `json:"expected_invoice_usd"`
	BillableRealUSD     float64 `json:"billable_real_usd"`
	Warning             string  `json:"warning,omitempty"`
}

func InterpretCost(reportedEstimateUSD float64, reportedLabel string, monthlyPlanUSD, paymentFeeUSD float64) CostReport {
	expected := monthlyPlanUSD + paymentFeeUSD
	report := CostReport{
		ReportedEstimateUSD: reportedEstimateUSD,
		ReportedLabel:       reportedLabel,
		MonthlyPlanUSD:      monthlyPlanUSD,
		PaymentFeeUSD:       paymentFeeUSD,
		ExpectedInvoiceUSD:  expected,
		BillableRealUSD:     expected,
	}
	label := strings.ToLower(reportedLabel)
	if strings.Contains(label, "sub") || strings.Contains(label, "subscription") {
		report.Warning = "reported cost is a subscription/token estimate, not the real invoice"
	}
	return report
}
