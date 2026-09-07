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
    let (mut models_catalog, models_source) = models::load_catalog(models_config_path).await?;

    let store = state::open(args.db_path.as_deref().map(std::path::Path::new))?;

    let now = models::now_iso8601();
    let mut agents = Vec::new();
    let mut models_persisted = 0;

    for adapter in &adapters_registry.adapters {
        let probe = runtime::probe_agent(&adapter.name, &adapter.binary, 5).await;
        let (_, settings_path, auth_type) = runtime::check_agent_credentials(&adapter.name);
        let snapshot_agent = runtime::build_agent_profile(
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

        let mut runtime_candidates = Vec::new();
        let existing_models = models_catalog.agents.get(&adapter.name);
        for p in &snapshot_agent.providers {
            for rm in &p.models {
                let existing = existing_models.and_then(|ems| ems.iter().find(|m| m.id == rm.id));
                let notes = existing.map(|e| e.notes.clone()).unwrap_or_default();
                let cost_hint = rm.cost_hint.as_ref().map(|_| 0.0).or_else(|| existing.and_then(|e| e.cost_hint));
                let promo = rm.cost_hint.as_ref().and_then(|c| c.promo.clone()).or_else(|| existing.and_then(|e| e.promo.clone()));
                runtime_candidates.push(models::ModelCandidate {
                    id: rm.id.clone(),
                    source: rm.source_type.clone(),
                    confidence: if rm.verified { "high".to_string() } else { "medium".to_string() },
                    notes,
                    fetched_at: Some(now.clone()),
                    cost_hint,
                    promo,
                    status: Some(rm.status.clone()),
                });
            }
        }

        models_catalog.agents.insert(adapter.name.clone(), runtime_candidates);

        let count = runtime::persist_runtime_agent(&store, &snapshot_agent)?;
        models_persisted += count;
        agents.push(snapshot_agent);
    }

    let _ = models::save_catalog(models_config_path, &models_catalog).await?;

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
    let (mut models_catalog, _) = models::load_catalog(models_config_path).await?;

    let store = state::open(args.db_path.as_deref().map(std::path::Path::new))?;

    let now = models::now_iso8601();
    let probe = runtime::probe_agent(adapter.name(), adapter.binary(), 5).await;
    let (_, settings_path, auth_type) = runtime::check_agent_credentials(adapter.name());
    let snapshot_agent = runtime::build_agent_profile(
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

    let mut runtime_candidates = Vec::new();
    let existing_models = models_catalog.agents.get(adapter.name());
    for p in &snapshot_agent.providers {
        for rm in &p.models {
            let existing = existing_models.and_then(|ems| ems.iter().find(|m| m.id == rm.id));
            let notes = existing.map(|e| e.notes.clone()).unwrap_or_default();
            let cost_hint = rm.cost_hint.as_ref().map(|_| 0.0).or_else(|| existing.and_then(|e| e.cost_hint));
            let promo = rm.cost_hint.as_ref().and_then(|c| c.promo.clone()).or_else(|| existing.and_then(|e| e.promo.clone()));
            runtime_candidates.push(models::ModelCandidate {
                id: rm.id.clone(),
                source: rm.source_type.clone(),
                confidence: if rm.verified { "high".to_string() } else { "medium".to_string() },
                notes,
                fetched_at: Some(now.clone()),
                cost_hint,
                promo,
                status: Some(rm.status.clone()),
            });
        }
    }

    models_catalog.agents.insert(adapter.name().to_string(), runtime_candidates);

    let _ = models::save_catalog(models_config_path, &models_catalog).await?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_two_consecutive_refreshes_update_fetched_at() {
        let dir = tempdir().unwrap();
        let adapters_path = dir.path().join("adapters.json");
        let models_path = dir.path().join("models-catalog.json");
        let db_path = dir.path().join("state.sqlite");

        let adapters_json = r#"{
            "schema_version": 1,
            "adapters": [{
                "name": "qwen-code",
                "binary": "qwen",
                "status": "available",
                "argv": ["qwen", "$MODEL", "$TASK"]
            }]
        }"#;
        fs::write(&adapters_path, adapters_json).unwrap();

        let initial_catalog_json = r#"{
            "schema_version": 2,
            "agents": {
                "qwen-code": [{
                    "id": "qwen3.8-max",
                    "source": "runtime",
                    "confidence": "high",
                    "notes": "initial",
                    "fetched_at": "2020-01-01T00:00:00Z",
                    "status": "active"
                }]
            }
        }"#;
        fs::write(&models_path, initial_catalog_json).unwrap();

        // First refresh
        let rep1 = run_refresh(AgentRefreshArgs {
            agent: "qwen-code".to_string(),
            adapters_config: Some(adapters_path.display().to_string()),
            models_config: Some(models_path.display().to_string()),
            db_path: Some(db_path.display().to_string()),
        })
        .await
        .unwrap();

        let (catalog1, _) = models::load_catalog(Some(&models_path)).await.unwrap();
        let qwen_models1 = catalog1.agents.get("qwen-code").unwrap();
        let m1 = qwen_models1.iter().find(|m| m.id == "qwen3.8-max").unwrap();
        assert!(m1.fetched_at.is_some());
        assert_ne!(m1.fetched_at.as_deref(), Some("2020-01-01T00:00:00Z"));
        assert_eq!(m1.fetched_at.as_deref(), Some(rep1.refreshed_at.as_str()));

        // Check SQLite
        let store = state::open(Some(&db_path)).unwrap();
        let records1 = store.list_agent_models("qwen-code").unwrap();
        let db_m1 = records1.iter().find(|r| r.model_id == "qwen3.8-max").unwrap();
        let meta1: serde_json::Value = serde_json::from_str(&db_m1.metadata_json).unwrap();
        assert_eq!(meta1.get("fetched_at").and_then(|v| v.as_str()), Some(rep1.refreshed_at.as_str()));

        // Short pause to ensure timestamp advances
        tokio::time::sleep(tokio::time::Duration::from_millis(1100)).await;

        // Second refresh
        let rep2 = run_refresh(AgentRefreshArgs {
            agent: "qwen-code".to_string(),
            adapters_config: Some(adapters_path.display().to_string()),
            models_config: Some(models_path.display().to_string()),
            db_path: Some(db_path.display().to_string()),
        })
        .await
        .unwrap();

        let (catalog2, _) = models::load_catalog(Some(&models_path)).await.unwrap();
        let qwen_models2 = catalog2.agents.get("qwen-code").unwrap();
        let m2 = qwen_models2.iter().find(|m| m.id == "qwen3.8-max").unwrap();
        assert_eq!(m2.fetched_at.as_deref(), Some(rep2.refreshed_at.as_str()));
        assert_ne!(m1.fetched_at, m2.fetched_at);

        let records2 = store.list_agent_models("qwen-code").unwrap();
        let db_m2 = records2.iter().find(|r| r.model_id == "qwen3.8-max").unwrap();
        let meta2: serde_json::Value = serde_json::from_str(&db_m2.metadata_json).unwrap();
        assert_eq!(meta2.get("fetched_at").and_then(|v| v.as_str()), Some(rep2.refreshed_at.as_str()));
        assert_ne!(meta1.get("fetched_at"), meta2.get("fetched_at"));
    }

    #[tokio::test]
    async fn test_refresh_removes_orphan_models() {
        let dir = tempdir().unwrap();
        let adapters_path = dir.path().join("adapters.json");
        let models_path = dir.path().join("models-catalog.json");
        let db_path = dir.path().join("state.sqlite");

        let adapters_json = r#"{
            "schema_version": 1,
            "adapters": [{
                "name": "qwen-code",
                "binary": "qwen",
                "status": "available",
                "argv": ["qwen", "$MODEL", "$TASK"]
            }]
        }"#;
        fs::write(&adapters_path, adapters_json).unwrap();

        // Catalog contains an orphan model that runtime doesn't report
        let initial_catalog_json = r#"{
            "schema_version": 2,
            "agents": {
                "qwen-code": [
                    {
                        "id": "orphan-discontinued-model-v1",
                        "source": "manual",
                        "confidence": "low",
                        "notes": "should be purged on refresh",
                        "fetched_at": "2020-01-01T00:00:00Z",
                        "status": "deprecated"
                    },
                    {
                        "id": "qwen3.8-max",
                        "source": "runtime",
                        "confidence": "high",
                        "notes": "active",
                        "fetched_at": "2020-01-01T00:00:00Z",
                        "status": "active"
                    }
                ]
            }
        }"#;
        fs::write(&models_path, initial_catalog_json).unwrap();

        run_refresh(AgentRefreshArgs {
            agent: "qwen-code".to_string(),
            adapters_config: Some(adapters_path.display().to_string()),
            models_config: Some(models_path.display().to_string()),
            db_path: Some(db_path.display().to_string()),
        })
        .await
        .unwrap();

        let (catalog, _) = models::load_catalog(Some(&models_path)).await.unwrap();
        let qwen_models = catalog.agents.get("qwen-code").unwrap();

        // Orphan model must NOT exist in the refreshed catalog
        assert!(qwen_models.iter().all(|m| m.id != "orphan-discontinued-model-v1"));
        // Runtime models must exist
        assert!(qwen_models.iter().any(|m| m.id == "qwen3.8-max"));
    }
}
