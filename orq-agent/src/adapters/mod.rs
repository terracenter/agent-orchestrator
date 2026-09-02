use color_eyre::eyre::{eyre, Result, WrapErr};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

const SUPPORTED_SCHEMA_VERSION: u8 = 1;
const DEFAULT_ADAPTERS_REGISTRY_PATH: &str = "config/adapters-registry.json";
const ADAPTERS_REGISTRY_ENV: &str = "ORQ_ADAPTERS_REGISTRY";

pub trait AgentAdapter: Send + Sync {
    fn name(&self) -> &str;
    fn binary(&self) -> &str;
    fn status(&self) -> AdapterStatus {
        AdapterStatus::Available
    }

    fn build_argv(&self, model: &str, task: &str) -> Vec<String>;

    fn binary_path(&self) -> Option<String> {
        let env_name = format!(
            "ORQ_AGENT_BIN_{}",
            self.name().replace('-', "_").to_ascii_uppercase()
        );
        std::env::var(env_name).ok().or_else(|| {
            which::which(self.binary())
                .ok()
                .map(|p| p.display().to_string())
        })
    }

    fn detect(&self) -> AgentDetection {
        let binary_path = self.binary_path();
        AgentDetection {
            name: self.name().to_string(),
            binary: self.binary().to_string(),
            detected: binary_path.is_some(),
            binary_path,
            adapter: self.status(),
            secrets_read: false,
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterStatus {
    Available,
    Missing,
    DeprecatedOrQuarantine,
    Gated,
}

#[derive(Debug, Serialize)]
pub struct AgentDetection {
    pub name: String,
    pub binary: String,
    pub detected: bool,
    pub binary_path: Option<String>,
    pub adapter: AdapterStatus,
    pub secrets_read: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AdaptersRegistry {
    pub schema_version: u8,
    pub adapters: Vec<AdapterDefinition>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AdapterDefinition {
    pub name: String,
    pub binary: String,
    pub status: AdapterStatus,
    pub argv: Vec<String>,
}

#[derive(Clone, Debug)]
struct ConfiguredAdapter {
    definition: AdapterDefinition,
}

impl AgentAdapter for ConfiguredAdapter {
    fn name(&self) -> &str {
        &self.definition.name
    }

    fn binary(&self) -> &str {
        &self.definition.binary
    }

    fn status(&self) -> AdapterStatus {
        self.definition.status
    }

    fn build_argv(&self, model: &str, task: &str) -> Vec<String> {
        self.definition
            .argv
            .iter()
            .map(|arg| match arg.as_str() {
                "$MODEL" => model.to_string(),
                "$TASK" => task.to_string(),
                _ => arg.to_string(),
            })
            .collect()
    }
}

pub fn default_registry() -> Result<AdaptersRegistry> {
    let path = default_config_path(ADAPTERS_REGISTRY_ENV, DEFAULT_ADAPTERS_REGISTRY_PATH);
    let content = std::fs::read_to_string(&path)
        .wrap_err_with(|| format!("reading adapters registry {}", path.display()))?;
    parse_registry(&content)
}

pub async fn load_registry(path: Option<&Path>) -> Result<(AdaptersRegistry, String)> {
    let path_buf;
    let path = match path {
        Some(path) => path,
        None => {
            path_buf = default_config_path(ADAPTERS_REGISTRY_ENV, DEFAULT_ADAPTERS_REGISTRY_PATH);
            path_buf.as_path()
        }
    };
    let content = tokio::fs::read_to_string(path)
        .await
        .wrap_err_with(|| format!("reading adapters registry {}", path.display()))?;
    Ok((parse_registry(&content)?, path.display().to_string()))
}

fn default_config_path(env_name: &str, relative_path: &str) -> std::path::PathBuf {
    std::env::var_os(env_name)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative_path))
}

pub fn parse_registry(content: &str) -> Result<AdaptersRegistry> {
    let registry: AdaptersRegistry =
        serde_json::from_str(content).wrap_err("parsing adapters registry json")?;
    validate_registry(&registry)?;
    Ok(registry)
}

fn validate_registry(registry: &AdaptersRegistry) -> Result<()> {
    if registry.schema_version != SUPPORTED_SCHEMA_VERSION {
        return Err(eyre!(
            "unsupported adapters registry schema_version {}; expected {}",
            registry.schema_version,
            SUPPORTED_SCHEMA_VERSION
        ));
    }
    if registry.adapters.is_empty() {
        return Err(eyre!("adapters registry must define at least one adapter"));
    }
    let mut names = HashSet::new();
    for adapter in &registry.adapters {
        if adapter.name.trim().is_empty() {
            return Err(eyre!("adapter name cannot be empty"));
        }
        let binary_lc = adapter.binary.to_ascii_lowercase();
        if adapter.binary.trim().is_empty()
            || adapter.binary.contains('/')
            || binary_lc.contains("placeholder")
            || binary_lc.contains("changeme")
            || binary_lc.contains("todo")
            || binary_lc.starts_with("xxx")
        {
            return Err(eyre!(
                "adapter {} binary must be a real command name, not an empty, path, or placeholder value",
                adapter.name
            ));
        }
        if adapter.argv.is_empty() {
            return Err(eyre!("adapter {} argv cannot be empty", adapter.name));
        }
        if !adapter.argv.iter().any(|arg| arg == "$MODEL") {
            return Err(eyre!("adapter {} argv must include $MODEL", adapter.name));
        }
        if !adapter.argv.iter().any(|arg| arg == "$TASK") {
            return Err(eyre!("adapter {} argv must include $TASK", adapter.name));
        }
        if !names.insert(adapter.name.clone()) {
            return Err(eyre!("duplicate adapter name {}", adapter.name));
        }
    }
    Ok(())
}

#[allow(dead_code)]
pub fn known_adapters() -> Result<Vec<Box<dyn AgentAdapter>>> {
    let registry = default_registry()?;
    Ok(adapters_from_registry(&registry))
}

pub fn adapters_from_registry(registry: &AdaptersRegistry) -> Vec<Box<dyn AgentAdapter>> {
    registry
        .adapters
        .iter()
        .cloned()
        .map(|definition| Box::new(ConfiguredAdapter { definition }) as Box<dyn AgentAdapter>)
        .collect()
}

#[allow(dead_code)]
pub fn find_adapter(name: &str) -> Option<Box<dyn AgentAdapter>> {
    let registry = default_registry().ok()?;
    find_adapter_in_registry(name, &registry)
}

pub fn find_adapter_in_registry(
    name: &str,
    registry: &AdaptersRegistry,
) -> Option<Box<dyn AgentAdapter>> {
    adapters_from_registry(registry)
        .into_iter()
        .find(|adapter| adapter.name() == name)
}

#[cfg(test)]
mod tests {
    use super::{default_registry, parse_registry, AdapterStatus, AgentAdapter, ConfiguredAdapter};

    #[test]
    fn default_registry_loads_qwen() {
        let registry = default_registry().unwrap();
        assert!(registry
            .adapters
            .iter()
            .any(|adapter| adapter.name == "qwen-code"));
    }

    #[test]
    fn maps_status_from_config() {
        let registry = default_registry().unwrap();
        let claude = registry
            .adapters
            .iter()
            .find(|adapter| adapter.name == "claude-code")
            .unwrap();
        assert_eq!(claude.status, AdapterStatus::Gated);
    }

    #[test]
    fn interpolates_argv_literals_only() {
        let registry = default_registry().unwrap();
        let qwen = registry
            .adapters
            .into_iter()
            .find(|adapter| adapter.name == "qwen-code")
            .unwrap();
        let adapter = ConfiguredAdapter { definition: qwen };
        assert_eq!(
            adapter.build_argv("m", "task"),
            vec![
                "--safe-mode",
                "-m",
                "m",
                "-p",
                "task",
                "--output-format",
                "text"
            ]
        );
    }

    #[test]
    fn rejects_path_binary() {
        let err = parse_registry(
            r#"{"schema_version":1,"adapters":[{"name":"bad","binary":"/bin/echo","status":"available","argv":["$MODEL","$TASK"]}]}"#,
        )
        .unwrap_err();
        assert!(err.to_string().contains("path, or placeholder value"));
    }

    #[test]
    fn rejects_placeholder_binary() {
        let err = parse_registry(
            r#"{"schema_version":1,"adapters":[{"name":"bad","binary":"XXX_PLACEHOLDER_XXX","status":"available","argv":["$MODEL","$TASK"]}]}"#,
        )
        .unwrap_err();
        assert!(err.to_string().contains("placeholder value"));
    }

    #[test]
    fn rejects_missing_placeholders() {
        let err = parse_registry(
            r#"{"schema_version":1,"adapters":[{"name":"bad","binary":"echo","status":"available","argv":["--model"]}]}"#,
        )
        .unwrap_err();
        assert!(err.to_string().contains("$MODEL"));
    }
}
