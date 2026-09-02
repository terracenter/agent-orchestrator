use crate::adapters::{self, AdapterStatus, AdaptersRegistry};
use crate::detect;
use crate::models::{self, DiscoveryStatus, ModelsCatalog};
use crate::state::{AgentRecord, ModelRecord, StateStore};
use color_eyre::eyre::{Result, WrapErr};
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Serialize)]
pub struct DiscoverReport {
    pub schema_version: u8,
    pub config_source: DiscoverConfigSource,
    pub state_source: String,
    pub agents: Vec<DiscoveredAgent>,
    pub agents_persisted: usize,
    pub models_persisted: usize,
    pub secrets_read: bool,
}

#[derive(Debug, Serialize)]
pub struct DiscoverConfigSource {
    pub adapters: String,
    pub models: String,
}

#[derive(Debug, Serialize)]
pub struct DiscoveredAgent {
    pub name: String,
    pub binary: String,
    pub detected: bool,
    pub binary_path: Option<String>,
    pub adapter: AdapterStatus,
    pub discovery: DiscoveryStatus,
    pub models: Vec<DiscoveredModel>,
    pub secrets_read: bool,
}

#[derive(Debug, Serialize)]
pub struct DiscoveredModel {
    pub id: String,
    pub source: String,
    pub confidence: String,
    pub task_kind: String,
    pub discovery: DiscoveryStatus,
}

pub struct DiscoverRequest<'a> {
    pub adapters_config: Option<&'a Path>,
    pub models_config: Option<&'a Path>,
    pub state_db_path: Option<&'a Path>,
}

pub async fn run(request: DiscoverRequest<'_>) -> Result<DiscoverReport> {
    let (adapters_registry, adapters_source) = adapters::load_registry(request.adapters_config)
        .await
        .wrap_err("loading adapters registry for discover")?;
    let (models_catalog, models_source) = models::load_catalog(request.models_config)
        .await
        .wrap_err("loading models catalog for discover")?;
    let store = crate::state::open(request.state_db_path).wrap_err("opening state store")?;
    discover_into_store(
        &adapters_registry,
        &models_catalog,
        &adapters_source,
        &models_source,
        &store,
    )
}

pub fn discover_into_store(
    adapters_registry: &AdaptersRegistry,
    models_catalog: &ModelsCatalog,
    adapters_source: &str,
    models_source: &str,
    store: &StateStore,
) -> Result<DiscoverReport> {
    let detect_report = detect::detect_agents_from_registry(adapters_registry);
    let mut agents = Vec::new();
    let mut models_persisted = 0usize;

    for detection in detect_report.agents {
        let model_candidates = models_catalog
            .agents
            .get(&detection.name)
            .cloned()
            .unwrap_or_default();
        let discovery = if model_candidates.is_empty() {
            DiscoveryStatus::Unsupported
        } else {
            DiscoveryStatus::ConfigCatalog
        };
        let metadata_json = serde_json::json!({
            "binary": detection.binary,
            "binary_path": detection.binary_path,
            "detected": detection.detected,
            "discovery": discovery,
            "secrets_read": false
        })
        .to_string();
        store
            .upsert_agent(&AgentRecord {
                agent_id: detection.name.clone(),
                display_name: detection.name.clone(),
                adapter_status: format!("{:?}", detection.adapter),
                metadata_json,
            })
            .wrap_err_with(|| format!("persisting discovered agent {}", detection.name))?;

        let mut discovered_models = Vec::new();
        for model in model_candidates {
            let task_kind = "general".to_string();
            store
                .upsert_model(&ModelRecord {
                    agent_id: detection.name.clone(),
                    model_id: model.id.clone(),
                    task_kind: task_kind.clone(),
                    gated: detection.adapter == AdapterStatus::Gated,
                    active: detection.detected,
                    metadata_json: serde_json::json!({
                        "source": model.source,
                        "confidence": model.confidence,
                        "notes": model.notes,
                        "discovery": discovery,
                        "secrets_read": false
                    })
                    .to_string(),
                })
                .wrap_err_with(|| {
                    format!(
                        "persisting discovered model {} for agent {}",
                        model.id, detection.name
                    )
                })?;
            models_persisted += 1;
            discovered_models.push(DiscoveredModel {
                id: model.id,
                source: model.source,
                confidence: model.confidence,
                task_kind,
                discovery,
            });
        }

        agents.push(DiscoveredAgent {
            name: detection.name,
            binary: detection.binary,
            detected: detection.detected,
            binary_path: detection.binary_path,
            adapter: detection.adapter,
            discovery,
            models: discovered_models,
            secrets_read: false,
        });
    }

    Ok(DiscoverReport {
        schema_version: detect_report.schema_version,
        config_source: DiscoverConfigSource {
            adapters: adapters_source.to_string(),
            models: models_source.to_string(),
        },
        state_source: store.path().display().to_string(),
        agents_persisted: agents.len(),
        models_persisted,
        agents,
        secrets_read: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::parse_registry;
    use crate::models::parse_catalog;

    #[test]
    fn discover_persists_agents_models_and_sources_without_secrets() {
        let registry = parse_registry(
            r#"{"schema_version":1,"adapters":[{"name":"fake-agent","binary":"definitely-not-orq-fake","status":"available","argv":["$MODEL","$TASK"]}]}"#,
        )
        .expect("registry");
        let catalog = parse_catalog(
            r#"{"schema_version":1,"agents":{"fake-agent":[{"id":"fake-model","source":"test","confidence":"test","notes":"not production data"}]}}"#,
        )
        .expect("catalog");
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("state.sqlite");
        let store = crate::state::open(Some(&db_path)).expect("state");

        let report =
            discover_into_store(&registry, &catalog, "adapters-test", "models-test", &store)
                .expect("discover");

        assert_eq!(report.config_source.adapters, "adapters-test");
        assert_eq!(report.config_source.models, "models-test");
        assert_eq!(report.state_source, db_path.display().to_string());
        assert!(!report.secrets_read);
        assert_eq!(report.agents_persisted, 1);
        assert_eq!(report.models_persisted, 1);
        let agent = store
            .find_agent("fake-agent")
            .expect("find agent")
            .expect("agent");
        assert_eq!(agent.agent_id, "fake-agent");
        let model = store
            .find_model("fake-agent", "fake-model", "general")
            .expect("find model")
            .expect("model");
        assert_eq!(model.model_id, "fake-model");
    }

    #[test]
    fn discover_marks_adapter_without_catalog_models_as_unsupported() {
        let registry = parse_registry(
            r#"{"schema_version":1,"adapters":[{"name":"fake-agent","binary":"definitely-not-orq-fake","status":"available","argv":["$MODEL","$TASK"]}]}"#,
        )
        .expect("registry");
        let catalog =
            parse_catalog(r#"{"schema_version":1,"agents":{"other-agent":[]}}"#).expect("catalog");
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("state.sqlite");
        let store = crate::state::open(Some(&db_path)).expect("state");

        let report =
            discover_into_store(&registry, &catalog, "adapters-test", "models-test", &store)
                .expect("discover");

        assert_eq!(report.agents[0].discovery, DiscoveryStatus::Unsupported);
        assert_eq!(report.models_persisted, 0);
        assert!(!report.agents[0].secrets_read);
    }
}
