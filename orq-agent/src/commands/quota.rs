use crate::quota::{
    generate_report, normalize_snapshot_input, parse_raw_snapshots_from_json, QuotaRecordResponse,
    QuotaReport, RawSnapshotInput,
};
use crate::state;
use color_eyre::eyre::{bail, Result};
use std::path::Path;

pub struct QuotaRecordArgs {
    pub provider: Option<String>,
    pub scope: Option<String>,
    pub remaining_pct: Option<f64>,
    pub used_pct: Option<f64>,
    pub status: Option<String>,
    pub reset_at_unix: Option<u64>,
    pub reset_in_seconds: Option<u64>,
    pub captured_at_unix: Option<u64>,
    pub metadata: Option<String>,
    pub json: Option<String>,
    pub db_path: Option<String>,
}

pub struct QuotaReportArgs {
    pub provider: Option<String>,
    pub db_path: Option<String>,
}

pub async fn run_record(args: QuotaRecordArgs) -> Result<QuotaRecordResponse> {
    let db_path = args.db_path.as_deref().map(Path::new);
    let store = state::open(db_path)?;

    let raw_inputs = if let Some(json_str) = args.json {
        parse_raw_snapshots_from_json(&json_str)?
    } else {
        let provider = match args.provider {
            Some(p) if !p.trim().is_empty() => p.trim().to_lowercase(),
            _ => bail!("missing required --provider or --json"),
        };
        let scope = match args.scope {
            Some(s) if !s.trim().is_empty() => s.trim().to_string(),
            _ => bail!("missing required --scope or --json"),
        };

        vec![RawSnapshotInput {
            provider,
            scope,
            remaining_pct: args.remaining_pct,
            used_pct: args.used_pct,
            status: args.status,
            reset_at_unix: args.reset_at_unix,
            reset_in_seconds: args.reset_in_seconds,
            captured_at_unix: args.captured_at_unix,
            metadata: None,
            metadata_json: args.metadata,
        }]
    };

    let mut recorded = Vec::new();
    for raw in raw_inputs {
        let input = normalize_snapshot_input(raw)?;
        let record = store.insert_quota_snapshot(&input)?;
        recorded.push(record);
    }

    Ok(QuotaRecordResponse {
        schema_version: 1,
        recorded_snapshots: recorded,
        secrets_read: false,
    })
}

pub async fn run_report(args: QuotaReportArgs) -> Result<QuotaReport> {
    let db_path = args.db_path.as_deref().map(Path::new);
    let store = state::open(db_path)?;
    generate_report(&store, args.provider.as_deref())
}
