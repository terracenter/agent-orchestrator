use crate::state::{QuotaSnapshotInput, QuotaSnapshotRecord, StateStore};
use color_eyre::eyre::{bail, eyre, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

pub const DEFAULT_KNOWN_PROVIDERS: &[&str] = &["agy", "claude-code", "codex", "qwen"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuotaReport {
    pub schema_version: u64,
    pub generated_at_unix: u64,
    pub secrets_read: bool,
    pub providers: Vec<ProviderQuotaSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderQuotaSummary {
    pub provider: String,
    pub status: String,
    pub scopes: Vec<QuotaScopeSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuotaScopeSummary {
    pub scope: String,
    pub remaining_pct: Option<f64>,
    pub used_pct: Option<f64>,
    pub status: String,
    pub reset_at_unix: Option<u64>,
    pub captured_at_unix: u64,
    pub metadata_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuotaRecordResponse {
    pub schema_version: u64,
    pub recorded_snapshots: Vec<QuotaSnapshotRecord>,
    pub secrets_read: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawSnapshotInput {
    pub provider: String,
    pub scope: String,
    #[serde(default)]
    pub remaining_pct: Option<f64>,
    #[serde(default)]
    pub used_pct: Option<f64>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub reset_at_unix: Option<u64>,
    #[serde(default)]
    pub reset_in_seconds: Option<u64>,
    #[serde(default)]
    pub captured_at_unix: Option<u64>,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
    #[serde(default)]
    pub metadata_json: Option<String>,
}

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn normalize_snapshot_input(raw: RawSnapshotInput) -> Result<QuotaSnapshotInput> {
    let provider = raw.provider.trim().to_lowercase();
    if provider.is_empty() {
        bail!("provider must not be empty");
    }
    let scope = raw.scope.trim().to_string();
    if scope.is_empty() {
        bail!("scope must not be empty");
    }

    let captured_at_unix = raw.captured_at_unix.unwrap_or_else(now_unix);

    let reset_at_unix = match (raw.reset_at_unix, raw.reset_in_seconds) {
        (Some(at), _) => Some(at),
        (None, Some(in_secs)) => Some(captured_at_unix.saturating_add(in_secs)),
        (None, None) => None,
    };

    let (remaining_pct, used_pct) = match (raw.remaining_pct, raw.used_pct) {
        (Some(rem), Some(used)) => {
            validate_pct("remaining_pct", rem)?;
            validate_pct("used_pct", used)?;
            (Some(round_pct(rem)), Some(round_pct(used)))
        }
        (Some(rem), None) => {
            validate_pct("remaining_pct", rem)?;
            (
                Some(round_pct(rem)),
                Some(round_pct((100.0 - rem).max(0.0))),
            )
        }
        (None, Some(used)) => {
            validate_pct("used_pct", used)?;
            (
                Some(round_pct((100.0 - used).max(0.0))),
                Some(round_pct(used)),
            )
        }
        (None, None) => (None, None),
    };

    let status = match raw.status.as_deref().map(str::trim) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => {
            if remaining_pct.is_none() && used_pct.is_none() {
                "quota_unknown".to_string()
            } else if remaining_pct == Some(0.0) {
                "exhausted".to_string()
            } else {
                "ok".to_string()
            }
        }
    };

    let metadata_json = if let Some(meta_val) = raw.metadata {
        serde_json::to_string(&meta_val)
            .map_err(|e| eyre!("failed to serialize metadata object: {e}"))?
    } else if let Some(meta_str) = raw.metadata_json {
        let meta_str = meta_str.trim();
        if meta_str.is_empty() {
            "{}".to_string()
        } else {
            // validate valid JSON
            let _parsed: serde_json::Value = serde_json::from_str(meta_str)
                .map_err(|e| eyre!("invalid metadata_json payload: {e}"))?;
            meta_str.to_string()
        }
    } else {
        "{}".to_string()
    };

    Ok(QuotaSnapshotInput {
        provider,
        scope,
        remaining_pct,
        used_pct,
        status: Some(status),
        reset_at_unix,
        captured_at_unix: Some(captured_at_unix),
        metadata_json: Some(metadata_json),
    })
}

fn validate_pct(name: &str, val: f64) -> Result<()> {
    if !val.is_finite() || !(0.0..=100.0).contains(&val) {
        bail!("{name} must be a number between 0.0 and 100.0 (got {val})");
    }
    Ok(())
}

fn round_pct(val: f64) -> f64 {
    (val * 100.0).round() / 100.0
}

pub fn parse_raw_snapshots_from_json(json_str: &str) -> Result<Vec<RawSnapshotInput>> {
    let trimmed = json_str.trim();
    let content = if let Some(path_str) = trimmed.strip_prefix('@') {
        fs::read_to_string(path_str)
            .map_err(|e| eyre!("failed to read json input file {path_str}: {e}"))?
    } else {
        trimmed.to_string()
    };

    let val: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| eyre!("invalid json input: {e}"))?;

    match val {
        serde_json::Value::Array(arr) => {
            let mut result = Vec::new();
            for item in arr {
                let input: RawSnapshotInput = serde_json::from_value(item)
                    .map_err(|e| eyre!("invalid snapshot item in array: {e}"))?;
                result.push(input);
            }
            Ok(result)
        }
        serde_json::Value::Object(_) => {
            let input: RawSnapshotInput =
                serde_json::from_value(val).map_err(|e| eyre!("invalid snapshot object: {e}"))?;
            Ok(vec![input])
        }
        _ => bail!("json input must be a snapshot object or array of snapshot objects"),
    }
}

pub fn generate_report(store: &StateStore, provider_filter: Option<&str>) -> Result<QuotaReport> {
    let filter_owned = provider_filter.map(|p| p.trim().to_lowercase());
    let provider_filter = filter_owned.as_deref().filter(|s| !s.is_empty());
    let snapshots = store.latest_quota_snapshots(provider_filter)?;

    let mut provider_scopes: BTreeMap<String, Vec<QuotaScopeSummary>> = BTreeMap::new();

    for snap in snapshots {
        let entry = provider_scopes.entry(snap.provider.clone()).or_default();
        entry.push(QuotaScopeSummary {
            scope: snap.scope.clone(),
            remaining_pct: snap.remaining_pct,
            used_pct: snap.used_pct,
            status: snap.status.clone(),
            reset_at_unix: snap.reset_at_unix,
            captured_at_unix: snap.captured_at_unix,
            metadata_json: snap.metadata_json.clone(),
        });
    }

    // Determine known providers to include
    let mut all_providers: BTreeSet<String> = BTreeSet::new();
    if let Some(filter) = provider_filter {
        all_providers.insert(filter.to_string());
    } else {
        for p in DEFAULT_KNOWN_PROVIDERS {
            all_providers.insert(p.to_string());
        }
        for p in provider_scopes.keys() {
            all_providers.insert(p.clone());
        }
    }

    let mut providers = Vec::new();
    for provider in all_providers {
        let scopes = provider_scopes.remove(&provider).unwrap_or_default();
        let status = if scopes.is_empty() {
            "quota_unknown".to_string()
        } else if scopes.iter().any(|s| {
            s.status == "exhausted" || s.status == "exceeded" || s.remaining_pct == Some(0.0)
        }) {
            "exhausted".to_string()
        } else if scopes.iter().any(|s| s.status == "warning") {
            "warning".to_string()
        } else if scopes.iter().any(|s| s.status == "quota_unknown") {
            "quota_unknown".to_string()
        } else {
            "ok".to_string()
        };

        providers.push(ProviderQuotaSummary {
            provider,
            status,
            scopes,
        });
    }

    Ok(QuotaReport {
        schema_version: 1,
        generated_at_unix: now_unix(),
        secrets_read: false,
        providers,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_valid_remaining_pct_and_calculates_used() {
        let raw = RawSnapshotInput {
            provider: "AGY".to_string(),
            scope: "gemini-weekly".to_string(),
            remaining_pct: Some(47.17),
            used_pct: None,
            status: None,
            reset_at_unix: None,
            reset_in_seconds: None,
            captured_at_unix: Some(1000),
            metadata: None,
            metadata_json: None,
        };
        let input = normalize_snapshot_input(raw).expect("normalize");
        assert_eq!(input.provider, "agy");
        assert_eq!(input.scope, "gemini-weekly");
        assert_eq!(input.remaining_pct, Some(47.17));
        assert_eq!(input.used_pct, Some(52.83));
        assert_eq!(input.status, Some("ok".to_string()));
        assert_eq!(input.captured_at_unix, Some(1000));
    }

    #[test]
    fn normalizes_quota_unknown_when_no_percentages() {
        let raw = RawSnapshotInput {
            provider: "QWEN".to_string(),
            scope: "general".to_string(),
            remaining_pct: None,
            used_pct: None,
            status: None,
            reset_at_unix: None,
            reset_in_seconds: None,
            captured_at_unix: None,
            metadata: None,
            metadata_json: None,
        };
        let input = normalize_snapshot_input(raw).expect("normalize");
        assert_eq!(input.provider, "qwen");
        assert_eq!(input.status, Some("quota_unknown".to_string()));
        assert_eq!(input.remaining_pct, None);
        assert_eq!(input.used_pct, None);
    }

    #[test]
    fn parses_json_array_and_calculates_exact_resets() {
        let json = r#"[
            {"provider": "agy", "scope": "gemini-weekly", "remaining_pct": 47.17},
            {"provider": "codex", "scope": "short-term", "remaining_pct": 22.0, "reset_in_seconds": 3600, "captured_at_unix": 10000}
        ]"#;
        let raws = parse_raw_snapshots_from_json(json).expect("parse json");
        assert_eq!(raws.len(), 2);

        let snap0 = normalize_snapshot_input(raws[0].clone()).unwrap();
        assert_eq!(snap0.provider, "agy");

        let snap1 = normalize_snapshot_input(raws[1].clone()).unwrap();
        assert_eq!(snap1.provider, "codex");
        assert_eq!(snap1.reset_at_unix, Some(13600));
    }

    #[test]
    fn report_aggregates_partial_quota_unknown_as_quota_unknown() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("state.sqlite");
        let store = crate::state::open(Some(&path)).expect("open state");

        // Record scope 1: ok
        let input1 = QuotaSnapshotInput {
            provider: "agy".to_string(),
            scope: "gemini-weekly".to_string(),
            remaining_pct: Some(50.0),
            used_pct: Some(50.0),
            status: Some("ok".to_string()),
            reset_at_unix: None,
            captured_at_unix: Some(1000),
            metadata_json: None,
        };
        store.insert_quota_snapshot(&input1).unwrap();

        // Record scope 2: quota_unknown (partial unknown)
        let input2 = QuotaSnapshotInput {
            provider: "agy".to_string(),
            scope: "claude-gpt-weekly".to_string(),
            remaining_pct: None,
            used_pct: None,
            status: Some("quota_unknown".to_string()),
            reset_at_unix: None,
            captured_at_unix: Some(1000),
            metadata_json: None,
        };
        store.insert_quota_snapshot(&input2).unwrap();

        let report = generate_report(&store, Some("AGY")).expect("report");
        assert_eq!(report.providers.len(), 1);
        let agy = &report.providers[0];
        assert_eq!(agy.provider, "agy");
        assert_eq!(agy.status, "quota_unknown");
        assert_eq!(agy.scopes.len(), 2);
    }

    #[test]
    fn report_aggregates_exhausted_over_quota_unknown() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("state.sqlite");
        let store = crate::state::open(Some(&path)).expect("open state");

        let input1 = QuotaSnapshotInput {
            provider: "agy".to_string(),
            scope: "gemini-weekly".to_string(),
            remaining_pct: Some(0.0),
            used_pct: Some(100.0),
            status: Some("exhausted".to_string()),
            reset_at_unix: None,
            captured_at_unix: Some(1000),
            metadata_json: None,
        };
        store.insert_quota_snapshot(&input1).unwrap();

        let input2 = QuotaSnapshotInput {
            provider: "agy".to_string(),
            scope: "claude-gpt-weekly".to_string(),
            remaining_pct: None,
            used_pct: None,
            status: Some("quota_unknown".to_string()),
            reset_at_unix: None,
            captured_at_unix: Some(1000),
            metadata_json: None,
        };
        store.insert_quota_snapshot(&input2).unwrap();

        let report = generate_report(&store, Some("agy")).expect("report");
        assert_eq!(report.providers[0].status, "exhausted");
    }

    #[test]
    fn report_aggregates_all_ok_as_ok() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("state.sqlite");
        let store = crate::state::open(Some(&path)).expect("open state");

        let input1 = QuotaSnapshotInput {
            provider: "agy".to_string(),
            scope: "gemini-weekly".to_string(),
            remaining_pct: Some(50.0),
            used_pct: Some(50.0),
            status: Some("ok".to_string()),
            reset_at_unix: None,
            captured_at_unix: Some(1000),
            metadata_json: None,
        };
        store.insert_quota_snapshot(&input1).unwrap();

        let report = generate_report(&store, Some("agy")).expect("report");
        assert_eq!(report.providers[0].status, "ok");
    }
}
