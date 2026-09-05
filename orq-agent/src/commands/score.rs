use color_eyre::eyre::Result;
use std::path::Path;

use crate::{score, state};

pub(crate) struct ScoreListArgs {
    pub(crate) agent: Option<String>,
    pub(crate) model: Option<String>,
    pub(crate) repo: Option<String>,
    pub(crate) task_type: Option<String>,
    pub(crate) limit: Option<usize>,
    pub(crate) db_path: Option<String>,
}

pub(crate) struct ScoreIngestArgs {
    pub(crate) db_path: Option<String>,
}

pub(crate) struct ScoreAggregateArgs {
    pub(crate) agent: String,
    pub(crate) model: String,
    pub(crate) repo: Option<String>,
    pub(crate) task_type: Option<String>,
    pub(crate) db_path: Option<String>,
}

pub(crate) async fn run_weights() -> Result<score::ScoreWeightsReport> {
    Ok(score::get_score_weights_report())
}

pub(crate) async fn run_list(args: ScoreListArgs) -> Result<Vec<state::EmpiricalRecord>> {
    let db_path = args.db_path.as_deref().map(Path::new);
    let store = state::open(db_path)?;

    let filter = state::EmpiricalHistoryFilter {
        agent_id: args.agent,
        model_id: args.model,
        repo: args.repo,
        task_type: args.task_type,
        user_id: None,
        limit: args.limit,
    };

    Ok(store.list_empirical_history(&filter)?)
}

pub(crate) async fn run_ingest(args: ScoreIngestArgs) -> Result<score::IngestReceiptsReport> {
    let db_path = args.db_path.as_deref().map(Path::new);
    let store = state::open(db_path)?;
    score::ingest_from_delegate_receipts(&store)
}

pub(crate) async fn run_aggregate(args: ScoreAggregateArgs) -> Result<state::AggregatedScore> {
    let db_path = args.db_path.as_deref().map(Path::new);
    let store = state::open(db_path)?;
    Ok(store.aggregate_scores(
        &args.agent,
        &args.model,
        args.repo.as_deref(),
        args.task_type.as_deref(),
    )?)
}
