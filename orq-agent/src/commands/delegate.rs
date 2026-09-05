use color_eyre::eyre::Result;

use crate::{adapters, delegate, policy};

#[allow(dead_code)]
pub(crate) struct DelegateArgs {
    pub(crate) task: Option<String>,
    pub(crate) agent: Option<String>,
    pub(crate) model: Option<String>,
    pub(crate) handoff: Option<String>,
    pub(crate) repo_path: Option<String>,
    pub(crate) agents_dir: Option<String>,
    pub(crate) workspace: Option<String>,
    pub(crate) write_handoff: Option<String>,
    pub(crate) write_receipt: Option<String>,
    pub(crate) force: bool,
    pub(crate) execute: bool,
    pub(crate) timeout_seconds: u64,
    pub(crate) correlation_id: Option<String>,
    pub(crate) policy_config: Option<String>,
    pub(crate) adapters_config: Option<String>,
}

#[allow(dead_code)]
pub(crate) async fn run(args: DelegateArgs) -> Result<delegate::DelegateOutput> {
    let policy_config_path = args.policy_config.as_deref().map(std::path::Path::new);
    let (policy_config, _) = policy::load_config(policy_config_path).await?;
    let adapters_config_path = args.adapters_config.as_deref().map(std::path::Path::new);
    let (adapters_registry, _) = adapters::load_registry(adapters_config_path).await?;

    delegate::run(delegate::DelegateRequest {
        task: args.task,
        agent: args.agent,
        model: args.model,
        handoff: args.handoff,
        repo_path: args.repo_path,
        agents_dir: args.agents_dir,
        workspace: args.workspace,
        write_handoff: args.write_handoff,
        write_receipt: args.write_receipt,
        force: args.force,
        execute: args.execute,
        timeout_seconds: args.timeout_seconds,
        correlation_id: args.correlation_id,
        policy_config,
        adapters_registry,
    })
    .await
}
