use color_eyre::eyre::{eyre, Result, WrapErr};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

const SUPPORTED_SCHEMA_VERSION: u8 = 1;
pub const BUILTIN_ADAPTERS_REGISTRY_JSON: &str =
    include_str!("../../config/adapters-registry.json");

pub trait AgentAdapter: Send + Sync {
    fn name(&self) -> &str;
    fn binary(&self) -> &str;
    fn status(&self) -> AdapterStatus {
        AdapterStatus::Available
    }

    fn build_argv(&self, model: &str, task: &str) -> Vec<String>;

    fn binary_path(&self) -> Option<String> {
        which::which(self.binary())
            .ok()
            .map(|p| p.display().to_string())
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
    parse_registry(BUILTIN_ADAPTERS_REGISTRY_JSON)
}

pub async fn load_registry(path: Option<&Path>) -> Result<(AdaptersRegistry, String)> {
    match path {
        None => Ok((default_registry()?, "builtin".to_string())),
        Some(path) => {
            let content = tokio::fs::read_to_string(path)
                .await
                .wrap_err_with(|| format!("reading adapters registry {}", path.display()))?;
            Ok((parse_registry(&content)?, path.display().to_string()))
        }
    }
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
    use super::{
        default_registry, load_registry, parse_registry, AdapterStatus, AgentAdapter,
        ConfiguredAdapter,
    };
    use tempfile::NamedTempFile;

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

    #[tokio::test]
    async fn load_registry_none_uses_builtin_and_ignores_env() {
        std::env::set_var("ORQ_ADAPTERS_REGISTRY", "/nonexistent/insecure_reg.json");
        let (registry, path) = load_registry(None).await.unwrap();
        assert_eq!(path, "builtin");
        assert!(registry
            .adapters
            .iter()
            .any(|adapter| adapter.name == "qwen-code"));
        std::env::remove_var("ORQ_ADAPTERS_REGISTRY");
    }

    #[tokio::test]
    async fn load_registry_with_valid_override() {
        use std::io::Write;
        let mut temp = NamedTempFile::new().unwrap();
        writeln!(
            temp,
            r#"{{"schema_version":1,"adapters":[{{"name":"test-agent","binary":"sh","status":"available","argv":["$MODEL","$TASK"]}}]}}"#
        )
        .unwrap();

        let (registry, path) = load_registry(Some(temp.path())).await.unwrap();
        assert_eq!(path, temp.path().display().to_string());
        assert_eq!(registry.adapters.len(), 1);
        assert_eq!(registry.adapters[0].name, "test-agent");
    }

    #[test]
    fn binary_path_ignores_env_var_override() {
        let qwen_def = super::AdapterDefinition {
            name: "qwen-code".to_string(),
            binary: "nonexistent-qwen-bin-xyz".to_string(),
            status: AdapterStatus::Available,
            argv: vec!["$MODEL".to_string(), "$TASK".to_string()],
        };
        let adapter = ConfiguredAdapter {
            definition: qwen_def,
        };
        std::env::set_var("ORQ_AGENT_BIN_QWEN_CODE", "/bin/sh");
        assert_eq!(adapter.binary_path(), None);
        std::env::remove_var("ORQ_AGENT_BIN_QWEN_CODE");
    }
}
