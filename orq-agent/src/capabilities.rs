use color_eyre::eyre::{eyre, Result, WrapErr};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

const SUPPORTED_SCHEMA_VERSION: u8 = 1;
const DEFAULT_TASK_CAPABILITIES_PATH: &str = "config/task-capabilities.json";
const TASK_CAPABILITIES_ENV: &str = "ORQ_TASK_CAPABILITIES";

/// Environment variable names the sandboxed child process always receives on top of any
/// granted capability. Declaring them again in a task-kind capability would let a task_kind
/// silently override the baseline confinement, so config validation rejects it.
const RESERVED_ENV_NAMES: [&str; 2] = ["PATH", "HOME"];

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TaskCapabilitiesConfig {
    pub schema_version: u8,
    pub task_kinds: HashMap<String, TaskCapabilityDefinition>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct TaskCapabilityDefinition {
    #[serde(default)]
    pub env_passthrough: Vec<String>,
}

#[allow(dead_code)]
pub fn default_config() -> Result<TaskCapabilitiesConfig> {
    let path = default_config_path(TASK_CAPABILITIES_ENV, DEFAULT_TASK_CAPABILITIES_PATH);
    let content = std::fs::read_to_string(&path)
        .wrap_err_with(|| format!("reading task capabilities config {}", path.display()))?;
    parse_config(&content)
}

pub async fn load_config(path: Option<&Path>) -> Result<(TaskCapabilitiesConfig, String)> {
    let path_buf;
    let path = match path {
        Some(path) => path,
        None => {
            path_buf = default_config_path(TASK_CAPABILITIES_ENV, DEFAULT_TASK_CAPABILITIES_PATH);
            path_buf.as_path()
        }
    };
    let content = tokio::fs::read_to_string(path)
        .await
        .wrap_err_with(|| format!("reading task capabilities config {}", path.display()))?;
    Ok((parse_config(&content)?, path.display().to_string()))
}

fn default_config_path(env_name: &str, relative_path: &str) -> std::path::PathBuf {
    std::env::var_os(env_name)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative_path))
}

pub fn parse_config(content: &str) -> Result<TaskCapabilitiesConfig> {
    let config: TaskCapabilitiesConfig =
        serde_json::from_str(content).wrap_err("parsing task capabilities config json")?;
    validate_config(&config)?;
    Ok(config)
}

fn validate_config(config: &TaskCapabilitiesConfig) -> Result<()> {
    if config.schema_version != SUPPORTED_SCHEMA_VERSION {
        return Err(eyre!(
            "unsupported task capabilities schema_version {}; expected {}",
            config.schema_version,
            SUPPORTED_SCHEMA_VERSION
        ));
    }
    for (task_kind, definition) in &config.task_kinds {
        if task_kind.trim().is_empty() {
            return Err(eyre!("task_kind key cannot be empty"));
        }
        for env_name in &definition.env_passthrough {
            if env_name.trim().is_empty() {
                return Err(eyre!(
                    "task_kind {task_kind} declares an empty env_passthrough entry"
                ));
            }
            if RESERVED_ENV_NAMES.contains(&env_name.as_str()) {
                return Err(eyre!(
                    "task_kind {task_kind} cannot re-declare reserved env var {env_name}"
                ));
            }
        }
    }
    Ok(())
}

/// Resolves the capabilities granted to a task_kind. Any task_kind absent from the config
/// (including an empty/unspecified one) resolves to the zero-value definition: no extra
/// capabilities. This is the deny-by-default rule required for #175.
pub fn resolve(config: &TaskCapabilitiesConfig, task_kind: &str) -> TaskCapabilityDefinition {
    config
        .task_kinds
        .get(task_kind)
        .cloned()
        .unwrap_or_default()
}

/// Reads the granted env vars from the current process environment. Values are never logged;
/// callers must only forward them into a child `Command`, never into a receipt or log line.
pub fn granted_env(definition: &TaskCapabilityDefinition) -> Vec<(String, String)> {
    definition
        .env_passthrough
        .iter()
        .filter_map(|name| std::env::var(name).ok().map(|value| (name.clone(), value)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{default_config, granted_env, parse_config, resolve, TaskCapabilitiesConfig};

    #[test]
    fn default_config_loads_real_tool_execution_with_gh_token() {
        let config = default_config().unwrap();
        let definition = resolve(&config, "real_tool_execution");
        assert_eq!(definition.env_passthrough, vec!["GH_TOKEN".to_string()]);
    }

    #[test]
    fn unknown_task_kind_grants_nothing() {
        let config = default_config().unwrap();
        let definition = resolve(&config, "totally-unknown-task-kind");
        assert!(definition.env_passthrough.is_empty());
    }

    #[test]
    fn declared_task_kind_without_extras_grants_nothing() {
        let config = default_config().unwrap();
        let definition = resolve(&config, "documentation");
        assert!(definition.env_passthrough.is_empty());
    }

    #[test]
    fn rejects_reserved_env_names() {
        let err =
            parse_config(r#"{"schema_version":1,"task_kinds":{"x":{"env_passthrough":["HOME"]}}}"#)
                .unwrap_err();
        assert!(err.to_string().contains("reserved env var"));
    }

    #[test]
    fn rejects_unsupported_schema_version() {
        let err = parse_config(r#"{"schema_version":2,"task_kinds":{}}"#).unwrap_err();
        assert!(err.to_string().contains("schema_version"));
    }

    #[test]
    fn granted_env_only_reads_declared_names() {
        std::env::set_var("ORQ_TEST_CAPABILITY_VAR_ALLOWED", "value-a");
        std::env::set_var("ORQ_TEST_CAPABILITY_VAR_DENIED", "value-b");
        let config: TaskCapabilitiesConfig = serde_json::from_str(
            r#"{"schema_version":1,"task_kinds":{"x":{"env_passthrough":["ORQ_TEST_CAPABILITY_VAR_ALLOWED"]}}}"#,
        )
        .unwrap();
        let definition = resolve(&config, "x");
        let env = granted_env(&definition);
        assert_eq!(
            env,
            vec![(
                "ORQ_TEST_CAPABILITY_VAR_ALLOWED".to_string(),
                "value-a".to_string()
            )]
        );
        std::env::remove_var("ORQ_TEST_CAPABILITY_VAR_ALLOWED");
        std::env::remove_var("ORQ_TEST_CAPABILITY_VAR_DENIED");
    }
}
