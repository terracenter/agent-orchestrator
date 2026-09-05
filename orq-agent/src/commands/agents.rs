use crate::{adapters, models, runtime, state};
use color_eyre::eyre::{eyre, Result};

pub(crate) struct AgentsDiscoverArgs {
    pub(crate) adapters_config: Option<String>,
    pub(crate) models_config: Option<String>,
    pub(crate) db_path: Option<String>,
}

pub(crate) struct AgentRefreshArgs {
    pub(crate) agent: String,
    pub(crate) adapters_config: Option<String>,
    pub(crate) models_config: Option<String>,
    pub(crate) db_path: Option<String>,
}

pub(crate) struct DoctorArgs {
    pub(crate) adapters_config: Option<String>,
}

pub(crate) async fn run_discover(
    args: AgentsDiscoverArgs,
) -> Result<runtime::AgentsDiscoverReport> {
    let adapters_config_path = args.adapters_config.as_deref().map(std::path::Path::new);
    let (adapters_registry, adapters_source) =
        adapters::load_registry(adapters_config_path).await?;

    let models_config_path = args.models_config.as_deref().map(std::path::Path::new);
    let (models_catalog, models_source) = models::load_catalog(models_config_path).await?;

    let store = state::open(args.db_path.as_deref().map(std::path::Path::new))?;

    let now = models::now_iso8601();
    let mut agents = Vec::new();
    let mut models_persisted = 0;

    for adapter in &adapters_registry.adapters {
        let probe = runtime::probe_agent(&adapter.name, &adapter.binary, 5).await;
        let (_, settings_path, auth_type) = runtime::check_agent_credentials(&adapter.name);
        let mut snapshot_agent = runtime::build_agent_profile(
            &adapter.name,
            &adapter.binary,
            probe.binary_path.as_deref(),
            probe.version.as_deref(),
            probe.detected,
            probe.has_credentials,
            settings_path.as_deref(),
            auth_type.as_deref(),
            &now,
        );

        if let Some(cat_models) = models_catalog.agents.get(&adapter.name) {
            for cat_m in cat_models {
                let mut found = false;
                for p in &mut snapshot_agent.providers {
                    if p.models.iter().any(|m| m.id == cat_m.id) {
                        found = true;
                        break;
                    }
                }
                if !found {
                    if let Some(p) = snapshot_agent.providers.first_mut() {
                        p.models.push(runtime::RuntimeModel {
                            id: cat_m.id.clone(),
                            status: cat_m
                                .status
                                .clone()
                                .unwrap_or_else(|| "available".to_string()),
                            source_type: cat_m.source.clone(),
                            verified: false,
                            last_verified_at: cat_m.fetched_at.clone(),
                            cost_hint: cat_m.cost_hint.map(|_c| runtime::RuntimeCostHint {
                                unit: "usd_per_token".to_string(),
                                promo: cat_m.promo.clone(),
                            }),
                            capabilities: None,
                        });
                    }
                }
            }
        }

        let count = runtime::persist_runtime_agent(&store, &snapshot_agent)?;
        models_persisted += count;
        agents.push(snapshot_agent);
    }

    Ok(runtime::AgentsDiscoverReport {
        schema_version: 1,
        snapshot_at: now,
        source: "runtime_doctor".to_string(),
        config_source: runtime::RuntimeDiscoverConfigSource {
            adapters: adapters_source,
            models: models_source,
        },
        state_source: store.path().display().to_string(),
        agents_persisted: agents.len(),
        models_persisted,
        agents,
        secrets_read: false,
    })
}

pub(crate) async fn run_refresh(args: AgentRefreshArgs) -> Result<runtime::AgentRefreshReport> {
    let adapters_config_path = args.adapters_config.as_deref().map(std::path::Path::new);
    let (adapters_registry, _) = adapters::load_registry(adapters_config_path).await?;

    let adapter = adapters::find_adapter_in_registry(&args.agent, &adapters_registry)
        .ok_or_else(|| eyre!("unknown agent adapter: {}", args.agent))?;

    let models_config_path = args.models_config.as_deref().map(std::path::Path::new);
    let (models_catalog, _) = models::load_catalog(models_config_path).await?;

    let store = state::open(args.db_path.as_deref().map(std::path::Path::new))?;

    let now = models::now_iso8601();
    let probe = runtime::probe_agent(adapter.name(), adapter.binary(), 5).await;
    let (_, settings_path, auth_type) = runtime::check_agent_credentials(adapter.name());
    let mut snapshot_agent = runtime::build_agent_profile(
        adapter.name(),
        adapter.binary(),
        probe.binary_path.as_deref(),
        probe.version.as_deref(),
        probe.detected,
        probe.has_credentials,
        settings_path.as_deref(),
        auth_type.as_deref(),
        &now,
    );

    if let Some(cat_models) = models_catalog.agents.get(adapter.name()) {
        for cat_m in cat_models {
            let mut found = false;
            for p in &mut snapshot_agent.providers {
                if p.models.iter().any(|m| m.id == cat_m.id) {
                    found = true;
                    break;
                }
            }
            if !found {
                if let Some(p) = snapshot_agent.providers.first_mut() {
                    p.models.push(runtime::RuntimeModel {
                        id: cat_m.id.clone(),
                        status: cat_m
                            .status
                            .clone()
                            .unwrap_or_else(|| "available".to_string()),
                        source_type: cat_m.source.clone(),
                        verified: false,
                        last_verified_at: cat_m.fetched_at.clone(),
                        cost_hint: cat_m.cost_hint.map(|_c| runtime::RuntimeCostHint {
                            unit: "usd_per_token".to_string(),
                            promo: cat_m.promo.clone(),
                        }),
                        capabilities: None,
                    });
                }
            }
        }
    }

    let models_persisted = runtime::persist_runtime_agent(&store, &snapshot_agent)?;

    Ok(runtime::AgentRefreshReport {
        schema_version: 1,
        refreshed_at: now,
        source: "runtime_doctor".to_string(),
        agent: snapshot_agent,
        state_source: store.path().display().to_string(),
        models_persisted,
        secrets_read: false,
    })
}

pub(crate) async fn run_doctor(args: DoctorArgs) -> Result<runtime::DoctorReport> {
    let adapters_config_path = args.adapters_config.as_deref().map(std::path::Path::new);
    let (adapters_registry, _) = adapters::load_registry(adapters_config_path).await?;
    runtime::doctor_health_check(&adapters_registry).await
}
