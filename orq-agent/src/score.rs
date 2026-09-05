use crate::receipt::{DelegateStatus, DelegateVerdict};
use crate::state::{EmpiricalRecordInput, StateStore};
use color_eyre::eyre::Result;
use serde::{Deserialize, Serialize};

/// 10 minimum evaluation dimensions as specified in RDD §3.6.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScoreInput {
    pub user_id: String,
    pub repo: String,
    pub language_stack: String,
    pub task_type: String,
    pub risk_level: String,
    pub agent_id: String,
    pub provider_id: String,
    pub model_id: String,
    pub mode: String,
    pub timestamp_bucket: String,
}

/// Quantitative weights for composite score computation:
///
/// Formula:
/// `Score = w1*S_rec + w2*C_law + w3*Q_tech + w4*D_doc + w5*E_cost - w6*H_inv - Penalties`
///
/// Documented defaults:
/// - `w1 = 0.30`: S_rec (validated receipt rate with verifiable git evidence)
/// - `w2 = 0.20`: C_law (compliance with workspace laws and no unauthorized operations)
/// - `w3 = 0.20`: Q_tech (technical quality, clean builds and passing unit tests/lints)
/// - `w4 = 0.10`: D_doc (updated documentation, diagrams, or notes in Obsidian/vault)
/// - `w5 = 0.20`: E_cost (cost and quota efficiency relative to active plan)
/// - `w6 = 0.10`: H_inv (human intervention penalty factor)
///
/// Constraint: `w1 + w2 + w3 + w4 + w5 == 1.0` (normalized), `w6 ∈ [0.0, 1.0]`.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct ScoreWeights {
    pub w1: f64,
    pub w2: f64,
    pub w3: f64,
    pub w4: f64,
    pub w5: f64,
    pub w6: f64,
}

impl Default for ScoreWeights {
    fn default() -> Self {
        Self {
            w1: 0.30,
            w2: 0.20,
            w3: 0.20,
            w4: 0.10,
            w5: 0.20,
            w6: 0.10,
        }
    }
}

impl ScoreWeights {
    pub fn validate(&self) -> Result<(), String> {
        let sum_pos = self.w1 + self.w2 + self.w3 + self.w4 + self.w5;
        if (sum_pos - 1.0).abs() > 1e-5 {
            return Err(format!(
                "positive weights w1..w5 must sum to 1.0 (got {:.4})",
                sum_pos
            ));
        }
        if !(0.0..=1.0).contains(&self.w6) {
            return Err(format!("w6 must be within [0.0, 1.0] (got {:.4})", self.w6));
        }
        Ok(())
    }
}

/// 6 quantitative metrics (§3.6), each defined in `[0.0, 1.0]`.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct ScoreMetrics {
    pub s_rec: f64,
    pub c_law: f64,
    pub q_tech: f64,
    pub d_doc: f64,
    pub e_cost: f64,
    pub h_inv: f64,
}

impl ScoreMetrics {
    pub fn clamped(&self) -> Self {
        Self {
            s_rec: self.s_rec.clamp(0.0, 1.0),
            c_law: self.c_law.clamp(0.0, 1.0),
            q_tech: self.q_tech.clamp(0.0, 1.0),
            d_doc: self.d_doc.clamp(0.0, 1.0),
            e_cost: self.e_cost.clamp(0.0, 1.0),
            h_inv: self.h_inv.clamp(0.0, 1.0),
        }
    }
}

/// Computed score output with metrics breakdown.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Score {
    pub score: f64,
    pub raw_score: f64,
    pub metrics: ScoreMetrics,
    pub weights: ScoreWeights,
    pub penalties: f64,
}

/// Computes the composite score from metrics, weights, and delegation penalties:
/// `Score = w1*S_rec + w2*C_law + w3*Q_tech + w4*D_doc + w5*E_cost - w6*H_inv - Penalties`.
pub fn compute_score(metrics: &ScoreMetrics, weights: &ScoreWeights, penalties: f64) -> Score {
    let m = metrics.clamped();
    let raw_score = weights.w1 * m.s_rec
        + weights.w2 * m.c_law
        + weights.w3 * m.q_tech
        + weights.w4 * m.d_doc
        + weights.w5 * m.e_cost
        - weights.w6 * m.h_inv;
    let score = raw_score - penalties;
    Score {
        score,
        raw_score,
        metrics: m,
        weights: *weights,
        penalties,
    }
}

/// Calculates the delegation penalty/adjustment for scoring as defined in RDD §3.9.
///
/// Returns the penalty value `P` subtracted in `Score = RawScore - P`.
/// - `validated` + `util` -> -1.0 (so `- (-1.0) = +1.0`, positive reinforcement)
/// - `executed` (sin tests) + `indeterminado` -> -0.2 (so `- (-0.2) = +0.2`, neutral/review)
/// - `failed: not_executed` + `non_util` -> 2.5 (so `- 2.5 = -2.5`, severe reliability penalty)
/// - `failed: plan_solo` + `non_util` -> 1.5 (so `- 1.5 = -1.5`, execution omission penalty)
/// - `failed: timeout` (reason `timeout_sin_evidencia`) + `non_util` -> 2.0 (so `- 2.0 = -2.0`, latency/blocking penalty)
pub fn delegation_penalty(
    status: &DelegateStatus,
    verdict: &DelegateVerdict,
    reason: Option<&str>,
) -> f64 {
    match (status, verdict) {
        (DelegateStatus::Validated, DelegateVerdict::Util) => -1.0,
        (DelegateStatus::Executed, DelegateVerdict::Indeterminado) => -0.2,
        (DelegateStatus::Failed, DelegateVerdict::NonUtil) => {
            let r = reason.unwrap_or_default().trim();
            if r == "not_executed" || r == "no_executed" {
                2.5
            } else if r == "plan_solo" || r == "plan_only" {
                1.5
            } else if r == "timeout" || r == "timeout_sin_evidencia" || r.contains("timeout") {
                2.0
            } else {
                2.5
            }
        }
        _ => 0.0,
    }
}

/// Formatted report of score weights and formula for the `score weights` CLI command.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ScoreWeightsReport {
    pub schema_version: u8,
    pub formula: String,
    pub weights: ScoreWeights,
    pub metrics_description: Vec<MetricDescription>,
    pub delegation_penalties: Vec<PenaltyRuleDescription>,
    pub secrets_read: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MetricDescription {
    pub code: String,
    pub name: String,
    pub weight: f64,
    pub description: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PenaltyRuleDescription {
    pub status: String,
    pub verdict: String,
    pub reason: String,
    pub impact: f64,
    pub description: String,
}

pub fn get_score_weights_report() -> ScoreWeightsReport {
    let weights = ScoreWeights::default();
    ScoreWeightsReport {
        schema_version: 1,
        formula: "Score = w1*S_rec + w2*C_law + w3*Q_tech + w4*D_doc + w5*E_cost - w6*H_inv - Penalties".to_string(),
        weights,
        metrics_description: vec![
            MetricDescription {
                code: "S_rec".to_string(),
                name: "Validated Receipt Rate".to_string(),
                weight: weights.w1,
                description: "1.0 if generated validated status with verifiable git commit/branch evidence; 0.0 otherwise.".to_string(),
            },
            MetricDescription {
                code: "C_law".to_string(),
                name: "Law & Constraint Compliance".to_string(),
                weight: weights.w2,
                description: "1.0 if complied with workspace laws (no sudo, secrets safe, rtk wrappers used); penalized if violated.".to_string(),
            },
            MetricDescription {
                code: "Q_tech".to_string(),
                name: "Technical Quality & Compilation".to_string(),
                weight: weights.w3,
                description: "1.0 if passes builds, unit tests, and linters on the first attempt without errors.".to_string(),
            },
            MetricDescription {
                code: "D_doc".to_string(),
                name: "Documentation Updates".to_string(),
                weight: weights.w4,
                description: "1.0 if documentation and notes in Obsidian vault are updated when required.".to_string(),
            },
            MetricDescription {
                code: "E_cost".to_string(),
                name: "Cost & Quota Efficiency".to_string(),
                weight: weights.w5,
                description: "Score based on consumption efficiency relative to active budget/plan (flat plan vs payg token).".to_string(),
            },
            MetricDescription {
                code: "H_inv".to_string(),
                name: "Human Intervention Required".to_string(),
                weight: weights.w6,
                description: "Measurement of additional corrective turns required by the user (subtracted from score).".to_string(),
            },
        ],
        delegation_penalties: vec![
            PenaltyRuleDescription {
                status: "validated".to_string(),
                verdict: "util".to_string(),
                reason: "evidence_verified".to_string(),
                impact: 1.0,
                description: "+1.0 positive reinforcement for verified execution with commit/branch evidence".to_string(),
            },
            PenaltyRuleDescription {
                status: "executed".to_string(),
                verdict: "indeterminado".to_string(),
                reason: "no_tests".to_string(),
                impact: 0.2,
                description: "+0.2 neutral/provisional score awaiting verification".to_string(),
            },
            PenaltyRuleDescription {
                status: "failed".to_string(),
                verdict: "non_util".to_string(),
                reason: "not_executed".to_string(),
                impact: -2.5,
                description: "-2.5 severe reliability penalty for accepting delegation without executing".to_string(),
            },
            PenaltyRuleDescription {
                status: "failed".to_string(),
                verdict: "non_util".to_string(),
                reason: "plan_solo".to_string(),
                impact: -1.5,
                description: "-1.5 execution omission penalty for returning only plan without code modifications".to_string(),
            },
            PenaltyRuleDescription {
                status: "failed".to_string(),
                verdict: "non_util".to_string(),
                reason: "timeout_sin_evidencia".to_string(),
                impact: -2.0,
                description: "-2.0 latency/blocking penalty for timing out without verifiable artifacts".to_string(),
            },
        ],
        secrets_read: false,
    }
}

/// Report emitted when ingesting receipts into the empirical history store.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct IngestReceiptsReport {
    pub schema_version: u8,
    pub total_receipts_scanned: usize,
    pub records_ingested: usize,
    pub secrets_read: bool,
}

/// Ingests all records from `delegate_receipts` table into `empirical_history` idempotently.
pub fn ingest_from_delegate_receipts(store: &StateStore) -> Result<IngestReceiptsReport> {
    let receipts = store.list_delegate_receipts()?;
    let total_receipts_scanned = receipts.len();
    let mut records_ingested = 0;
    let weights = ScoreWeights::default();

    for receipt in &receipts {
        let s_rec = if receipt.status == DelegateStatus::Validated
            && !receipt.evidence.trim().is_empty()
            && receipt.evidence.trim() != "none"
        {
            1.0
        } else {
            0.0
        };

        let penalties =
            delegation_penalty(&receipt.status, &receipt.verdict, receipt.reason.as_deref());

        let c_law = if !receipt.secrets_read { 1.0 } else { 0.0 };
        let q_tech = match receipt.exit_code {
            Some(0) => 1.0,
            Some(_) => 0.0,
            None => {
                if receipt.status == DelegateStatus::Validated {
                    1.0
                } else if receipt.status == DelegateStatus::Executed {
                    0.5
                } else {
                    0.0
                }
            }
        };
        let d_doc = 1.0;
        let e_cost = 1.0;
        let h_inv = 0.0;

        let metrics = ScoreMetrics {
            s_rec,
            c_law,
            q_tech,
            d_doc,
            e_cost,
            h_inv,
        };

        let calculated = compute_score(&metrics, &weights, penalties);

        let user_id = std::env::var("ORQ_USER_ID")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .or_else(|| std::env::var("USER").ok().filter(|s| !s.trim().is_empty()))
            .or_else(|| {
                std::env::var("LOGNAME")
                    .ok()
                    .filter(|s| !s.trim().is_empty())
            })
            .unwrap_or_else(|| "freddy".to_string());

        let repo = "agent-orchestrator".to_string();
        let language_stack = "rust".to_string();
        let task_type = "delegate".to_string();
        let risk_level = "medio".to_string();
        let provider_id = infer_provider(&receipt.agent, &receipt.model);
        let mode = "agentic".to_string();
        let timestamp_bucket = timestamp_bucket_from_unix(receipt.started_at_unix);

        let input = EmpiricalRecordInput {
            correlation_id: receipt.correlation_id.clone(),
            user_id,
            repo,
            language_stack,
            task_type,
            risk_level,
            agent_id: receipt.agent.clone(),
            provider_id,
            model_id: receipt.model.clone(),
            mode,
            timestamp_bucket,
            s_rec,
            c_law,
            q_tech,
            d_doc,
            e_cost,
            h_inv,
            penalties,
            score: calculated.score,
            created_at_unix: Some(receipt.started_at_unix),
        };

        store.insert_empirical_record(&input)?;
        records_ingested += 1;
    }

    Ok(IngestReceiptsReport {
        schema_version: 1,
        total_receipts_scanned,
        records_ingested,
        secrets_read: false,
    })
}

pub fn infer_provider(agent_id: &str, _model_id: &str) -> String {
    let lower_agent = agent_id.trim().to_lowercase();
    if lower_agent.contains("agy") || lower_agent.contains("antigravity") {
        "google".to_string()
    } else if lower_agent.contains("claude") {
        "anthropic".to_string()
    } else if lower_agent.contains("qwen") {
        "bailian".to_string()
    } else if lower_agent.contains("hermes") {
        "openrouter".to_string()
    } else if lower_agent.contains("openclaw") {
        "local".to_string()
    } else {
        "unknown".to_string()
    }
}

pub fn timestamp_bucket_from_unix(unix_secs: u64) -> String {
    // Bucket format: YYYY-MM-DD
    let days_since_epoch = unix_secs / 86400;
    let mut days = days_since_epoch as i64;
    let mut year = 1970;
    loop {
        let leap = if (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0) {
            366
        } else {
            365
        };
        if days < leap {
            break;
        }
        days -= leap;
        year += 1;
    }
    let is_leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
    let month_days = [
        31,
        if is_leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 1;
    for &md in &month_days {
        if days < md {
            break;
        }
        days -= md;
        month += 1;
    }
    let day = days + 1;
    format!("{year:04}-{month:02}-{day:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weights_sum_to_one_and_validate() {
        let weights = ScoreWeights::default();
        assert!(weights.validate().is_ok());
        let sum = weights.w1 + weights.w2 + weights.w3 + weights.w4 + weights.w5;
        assert!((sum - 1.0).abs() < 1e-6);
        assert!(weights.w6 >= 0.0 && weights.w6 <= 1.0);
    }

    #[test]
    fn weights_validation_fails_on_invalid_sum() {
        let weights = ScoreWeights {
            w1: 0.5,
            w2: 0.5,
            w3: 0.5,
            w4: 0.1,
            w5: 0.2,
            w6: 0.1,
        };
        assert!(weights.validate().is_err());
    }

    #[test]
    fn compute_score_perfect_metrics_yields_one() {
        let weights = ScoreWeights::default();
        let metrics = ScoreMetrics {
            s_rec: 1.0,
            c_law: 1.0,
            q_tech: 1.0,
            d_doc: 1.0,
            e_cost: 1.0,
            h_inv: 0.0,
        };
        let res = compute_score(&metrics, &weights, 0.0);
        assert!((res.score - 1.0).abs() < 1e-6);
        assert!((res.raw_score - 1.0).abs() < 1e-6);
    }

    #[test]
    fn compute_score_human_intervention_reduces_score() {
        let weights = ScoreWeights::default();
        let metrics_no_h = ScoreMetrics {
            s_rec: 1.0,
            c_law: 1.0,
            q_tech: 1.0,
            d_doc: 1.0,
            e_cost: 1.0,
            h_inv: 0.0,
        };
        let metrics_with_h = ScoreMetrics {
            s_rec: 1.0,
            c_law: 1.0,
            q_tech: 1.0,
            d_doc: 1.0,
            e_cost: 1.0,
            h_inv: 1.0,
        };
        let score1 = compute_score(&metrics_no_h, &weights, 0.0);
        let score2 = compute_score(&metrics_with_h, &weights, 0.0);
        assert_eq!(score1.score, 1.0);
        assert_eq!(score2.score, 1.0 - weights.w6);
    }

    #[test]
    fn metrics_clamping_bounds_inputs() {
        let metrics = ScoreMetrics {
            s_rec: 2.5,
            c_law: -1.0,
            q_tech: 1.5,
            d_doc: -0.5,
            e_cost: 10.0,
            h_inv: -2.0,
        };
        let clamped = metrics.clamped();
        assert_eq!(clamped.s_rec, 1.0);
        assert_eq!(clamped.c_law, 0.0);
        assert_eq!(clamped.q_tech, 1.0);
        assert_eq!(clamped.d_doc, 0.0);
        assert_eq!(clamped.e_cost, 1.0);
        assert_eq!(clamped.h_inv, 0.0);
    }

    #[test]
    fn section_3_9_delegation_penalties_exact() {
        // 1. validated + util -> impact +1.0 (penalty = -1.0)
        let p_val = delegation_penalty(&DelegateStatus::Validated, &DelegateVerdict::Util, None);
        assert_eq!(p_val, -1.0);

        // 2. executed (sin tests) + indeterminado -> impact +0.2 (penalty = -0.2)
        let p_exec = delegation_penalty(
            &DelegateStatus::Executed,
            &DelegateVerdict::Indeterminado,
            None,
        );
        assert_eq!(p_exec, -0.2);

        // 3. failed: not_executed + non_util -> impact -2.5 (penalty = 2.5)
        let p_not_exec = delegation_penalty(
            &DelegateStatus::Failed,
            &DelegateVerdict::NonUtil,
            Some("not_executed"),
        );
        assert_eq!(p_not_exec, 2.5);

        let p_no_exec = delegation_penalty(
            &DelegateStatus::Failed,
            &DelegateVerdict::NonUtil,
            Some("no_executed"),
        );
        assert_eq!(p_no_exec, 2.5);

        // 4. failed: plan_solo + non_util -> impact -1.5 (penalty = 1.5)
        let p_plan = delegation_penalty(
            &DelegateStatus::Failed,
            &DelegateVerdict::NonUtil,
            Some("plan_solo"),
        );
        assert_eq!(p_plan, 1.5);

        // 5. failed: timeout + non_util -> impact -2.0 (penalty = 2.0)
        let p_timeout = delegation_penalty(
            &DelegateStatus::Failed,
            &DelegateVerdict::NonUtil,
            Some("timeout_sin_evidencia"),
        );
        assert_eq!(p_timeout, 2.0);

        let p_timeout_generic = delegation_penalty(
            &DelegateStatus::Failed,
            &DelegateVerdict::NonUtil,
            Some("timeout"),
        );
        assert_eq!(p_timeout_generic, 2.0);
    }

    #[test]
    fn compute_score_with_each_penalty_applies_correct_impact() {
        let weights = ScoreWeights::default();
        let zero_metrics = ScoreMetrics {
            s_rec: 0.0,
            c_law: 0.0,
            q_tech: 0.0,
            d_doc: 0.0,
            e_cost: 0.0,
            h_inv: 0.0,
        };

        // Raw score is 0.0
        // +1.0 reinforcement
        let p1 = delegation_penalty(&DelegateStatus::Validated, &DelegateVerdict::Util, None);
        assert_eq!(compute_score(&zero_metrics, &weights, p1).score, 1.0);

        // +0.2 review
        let p2 = delegation_penalty(
            &DelegateStatus::Executed,
            &DelegateVerdict::Indeterminado,
            None,
        );
        assert_eq!(compute_score(&zero_metrics, &weights, p2).score, 0.2);

        // -2.5 not executed
        let p3 = delegation_penalty(
            &DelegateStatus::Failed,
            &DelegateVerdict::NonUtil,
            Some("not_executed"),
        );
        assert_eq!(compute_score(&zero_metrics, &weights, p3).score, -2.5);

        // -1.5 plan solo
        let p4 = delegation_penalty(
            &DelegateStatus::Failed,
            &DelegateVerdict::NonUtil,
            Some("plan_solo"),
        );
        assert_eq!(compute_score(&zero_metrics, &weights, p4).score, -1.5);

        // -2.0 timeout
        let p5 = delegation_penalty(
            &DelegateStatus::Failed,
            &DelegateVerdict::NonUtil,
            Some("timeout_sin_evidencia"),
        );
        assert_eq!(compute_score(&zero_metrics, &weights, p5).score, -2.0);
    }

    #[test]
    fn timestamp_bucket_calculation() {
        // Unix timestamp for 2026-09-04 12:00:00 UTC = 1788523200
        let bucket = timestamp_bucket_from_unix(1788523200);
        assert_eq!(bucket, "2026-09-04");
    }
}
