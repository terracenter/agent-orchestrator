use crate::adapters::{find_adapter_in_registry, AdapterStatus, AdaptersRegistry};
use color_eyre::eyre::{eyre, Result, WrapErr};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

const SUPPORTED_SCHEMA_VERSION: u8 = 1;
const DEFAULT_MODELS_CATALOG_PATH: &str = "config/models-catalog.json";
const MODELS_CATALOG_ENV: &str = "ORQ_MODELS_CATALOG";

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ModelsCatalog {
    pub schema_version: u8,
    pub agents: BTreeMap<String, Vec<ModelCandidate>>,
}

#[derive(Debug, Serialize)]
pub struct ModelsReport {
    pub schema_version: u8,
    pub agent: String,
    pub detected: bool,
    pub status: AdapterStatus,
    pub models: Vec<ModelCandidate>,
    pub discovery: DiscoveryStatus,
    pub config_source: String,
    pub secrets_read: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ModelCandidate {
    pub id: String,
    pub source: String,
    pub confidence: String,
    pub notes: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveryStatus {
    ConfigCatalog,
    Unsupported,
}

#[allow(dead_code)]
pub fn default_catalog() -> Result<ModelsCatalog> {
    let path = default_config_path(MODELS_CATALOG_ENV, DEFAULT_MODELS_CATALOG_PATH);
    let content = std::fs::read_to_string(&path)
        .wrap_err_with(|| format!("reading models catalog {}", path.display()))?;
    parse_catalog(&content)
}

pub async fn load_catalog(path: Option<&Path>) -> Result<(ModelsCatalog, String)> {
    let path_buf;
    let path = match path {
        Some(path) => path,
        None => {
            path_buf = default_config_path(MODELS_CATALOG_ENV, DEFAULT_MODELS_CATALOG_PATH);
            path_buf.as_path()
        }
    };
    let content = tokio::fs::read_to_string(path)
        .await
        .wrap_err_with(|| format!("reading models catalog {}", path.display()))?;
    Ok((parse_catalog(&content)?, path.display().to_string()))
}

fn default_config_path(env_name: &str, relative_path: &str) -> std::path::PathBuf {
    std::env::var_os(env_name)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative_path))
}

pub fn parse_catalog(content: &str) -> Result<ModelsCatalog> {
    let catalog: ModelsCatalog =
        serde_json::from_str(content).wrap_err("parsing models catalog json")?;
    validate_catalog(&catalog)?;
    Ok(catalog)
}

fn validate_catalog(catalog: &ModelsCatalog) -> Result<()> {
    if catalog.schema_version != SUPPORTED_SCHEMA_VERSION {
        return Err(eyre!(
            "unsupported models catalog schema_version {}; expected {}",
            catalog.schema_version,
            SUPPORTED_SCHEMA_VERSION
        ));
    }
    if catalog.agents.is_empty() {
        return Err(eyre!("models catalog must define at least one agent"));
    }
    for (agent, models) in &catalog.agents {
        if agent.trim().is_empty() {
            return Err(eyre!("models catalog agent name cannot be empty"));
        }
        for model in models {
            if model.id.trim().is_empty() {
                return Err(eyre!("models catalog entry for {agent} has empty id"));
            }
        }
    }
    Ok(())
}

pub fn list(
    agent: &str,
    catalog: &ModelsCatalog,
    adapters_registry: &AdaptersRegistry,
    config_source: &str,
) -> Result<ModelsReport> {
    let adapter = find_adapter_in_registry(agent, adapters_registry)
        .ok_or_else(|| eyre!("unknown agent adapter: {agent}"))?;
    let detected = adapter.binary_path().is_some();
    let models = catalog
        .agents
        .get(adapter.name())
        .cloned()
        .unwrap_or_default();
    let discovery = if models.is_empty() {
        DiscoveryStatus::Unsupported
    } else {
        DiscoveryStatus::ConfigCatalog
    };

    Ok(ModelsReport {
        schema_version: catalog.schema_version,
        agent: adapter.name().to_string(),
        detected,
        status: adapter.status(),
        models,
        discovery,
        config_source: config_source.to_string(),
        secrets_read: false,
    })
}

#[cfg(test)]
mod tests {
    use super::{default_catalog, list, parse_catalog};

    #[test]
    fn default_catalog_reports_qwen_flash() {
        let catalog = default_catalog().unwrap();
        let adapters_registry = crate::adapters::default_registry().unwrap();
        let report = list("qwen-code", &catalog, &adapters_registry, "test").unwrap();
        assert!(report
            .models
            .iter()
            .any(|model| model.id == "qwen3.6-flash"));
    }

    #[test]
    fn rejects_invalid_schema() {
        let err = parse_catalog(r#"{"schema_version":2,"agents":{}}"#).unwrap_err();
        assert!(err
            .to_string()
            .contains("unsupported models catalog schema_version"));
    }

    #[test]
    fn rejects_empty_model_id() {
        let err = parse_catalog(
            r#"{"schema_version":1,"agents":{"qwen-code":[{"id":"","source":"s","confidence":"c","notes":"n"}]}}"#,
        )
        .unwrap_err();
        assert!(err.to_string().contains("empty id"));
    }
}
