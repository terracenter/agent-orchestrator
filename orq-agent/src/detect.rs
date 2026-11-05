use crate::adapters::{adapters_from_registry, known_adapters, AdaptersRegistry, AgentDetection};
use crate::models::ModelsCatalog;
use crate::models_detect;
use color_eyre::eyre::Result;
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Serialize)]
pub struct DetectReport {
    pub schema_version: u8,
    pub agents: Vec<AgentDetection>,
    pub secrets_read: bool,
}

#[derive(Debug, Serialize)]
pub struct AgentWithModels {
    #[serde(flatten)]
    pub detection: AgentDetection,
    pub models_source: String,
    pub models: Vec<models_detect::DetectedModel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub models_note: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DetectModelsReport {
    pub schema_version: u8,
    pub agents: Vec<AgentWithModels>,
    pub secrets_read: bool,
}

#[allow(dead_code)]
pub fn detect_agents() -> Result<DetectReport> {
    let agents = known_adapters()?
        .into_iter()
        .map(|adapter| adapter.detect())
        .collect();

    Ok(DetectReport {
        schema_version: 1,
        agents,
        secrets_read: false,
    })
}

pub fn detect_agents_from_registry(registry: &AdaptersRegistry) -> DetectReport {
    let agents = adapters_from_registry(registry)
        .into_iter()
        .map(|adapter| adapter.detect())
        .collect();

    DetectReport {
        schema_version: registry.schema_version,
        agents,
        secrets_read: false,
    }
}

pub fn detect_agents_with_models_from_registry(
    registry: &AdaptersRegistry,
    catalog: &ModelsCatalog,
    home: Option<&Path>,
) -> DetectModelsReport {
    let agents = adapters_from_registry(registry)
        .into_iter()
        .map(|adapter| {
            let detection = adapter.detect();
            let resolution =
                models_detect::resolve_models(adapter.name(), adapter.models_spec(), catalog, home);
            AgentWithModels {
                detection,
                models_source: resolution.source,
                models: resolution.models,
                models_note: resolution.note,
            }
        })
        .collect();

    DetectModelsReport {
        schema_version: registry.schema_version,
        agents,
        secrets_read: false,
    }
}

#[cfg(test)]
mod tests {
    use super::{detect_agents, detect_agents_with_models_from_registry};

    #[test]
    fn detect_report_never_reads_secrets() {
        let report = detect_agents().unwrap();
        assert!(!report.secrets_read);
    }

    #[test]
    fn detect_with_models_uses_adapter_spec() {
        let registry = crate::adapters::parse_registry(
            r#"{"schema_version":1,"adapters":[{"name":"fake-agent","binary":"definitely-not-orq-fake","status":"available","argv":["$MODEL","$TASK"],"detect":{"models":{"source":"aliases","aliases":["a","b"]}}}]}"#,
        )
        .expect("registry");
        let catalog =
            crate::models::parse_catalog(r#"{"schema_version":1,"agents":{"fake-agent":[]}}"#)
                .expect("catalog");

        let report = detect_agents_with_models_from_registry(&registry, &catalog, None);
        assert_eq!(report.agents.len(), 1);
        assert_eq!(report.agents[0].models_source, "aliases");
        assert_eq!(report.agents[0].models.len(), 2);
        assert!(!report.secrets_read);
    }
}
