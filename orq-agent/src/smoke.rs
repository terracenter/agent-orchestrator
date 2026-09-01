use crate::exec::{self, ExecRequest};
use crate::policy::PolicyConfig;
use crate::receipt::ExecReceipt;
use color_eyre::eyre::{Result, WrapErr};
use std::time::{SystemTime, UNIX_EPOCH};

pub async fn run(
    agent: String,
    model: String,
    timeout_seconds: u64,
    allow_gated: bool,
    correlation_id: Option<String>,
    policy_config: PolicyConfig,
) -> Result<ExecReceipt> {
    let marker = format!("ORQ_SMOKE_OK agent={agent} model={model}");
    let task_file = write_smoke_task(&agent, &model, &marker).await?;
    let receipt = exec::run(ExecRequest {
        agent,
        model,
        task_file: task_file.clone(),
        timeout_seconds,
        allow_gated,
        correlation_id,
        policy_config,
    })
    .await;
    let _ = tokio::fs::remove_file(&task_file).await;
    let mut receipt = receipt?;
    if matches!(receipt.status, crate::receipt::ExecStatus::Succeeded)
        && !receipt.stdout_tail.contains(&marker)
    {
        receipt.status = crate::receipt::ExecStatus::Failed;
        receipt.policy_reason = format!("smoke marker not found: {marker}");
    }
    Ok(receipt)
}

async fn write_smoke_task(agent: &str, _model: &str, marker: &str) -> Result<String> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .wrap_err("system clock before UNIX_EPOCH")?
        .as_nanos();
    let path = std::env::temp_dir().join(format!("orq-agent-smoke-{agent}-{nonce}.md"));
    let content =
        format!("Orq smoke test. Respond with one short line containing exactly: {marker}\n");
    tokio::fs::write(&path, content)
        .await
        .wrap_err_with(|| format!("writing smoke task {}", path.display()))?;
    Ok(path.display().to_string())
}
