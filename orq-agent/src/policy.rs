use crate::adapters::AdapterStatus;
use color_eyre::eyre::{eyre, Result, WrapErr};
use serde::{Deserialize, Serialize};
use std::path::Path;

const SUPPORTED_SCHEMA_VERSION: u8 = 1;
const DEFAULT_POLICY_CONFIG_PATH: &str = "config/policy.json";
const POLICY_CONFIG_ENV: &str = "ORQ_POLICY_CONFIG";

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PolicyConfig {
    pub schema_version: u8,
    pub approval_required_model_patterns: Vec<String>,
    pub blocked_adapter_statuses: Vec<String>,
    pub gated_adapter_statuses: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct PolicyDecision {
    pub allowed: bool,
    pub reason: String,
}

#[allow(dead_code)]
pub fn default_config() -> Result<PolicyConfig> {
    let path = default_config_path(POLICY_CONFIG_ENV, DEFAULT_POLICY_CONFIG_PATH);
    let content = std::fs::read_to_string(&path)
        .wrap_err_with(|| format!("reading policy config {}", path.display()))?;
    parse_config(&content)
}

pub async fn load_config(path: Option<&Path>) -> Result<(PolicyConfig, String)> {
    let path_buf;
    let path = match path {
        Some(path) => path,
        None => {
            path_buf = default_config_path(POLICY_CONFIG_ENV, DEFAULT_POLICY_CONFIG_PATH);
            path_buf.as_path()
        }
    };
    let content = tokio::fs::read_to_string(path)
        .await
        .wrap_err_with(|| format!("reading policy config {}", path.display()))?;
    Ok((parse_config(&content)?, path.display().to_string()))
}

fn default_config_path(env_name: &str, relative_path: &str) -> std::path::PathBuf {
    std::env::var_os(env_name)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative_path))
}

pub fn parse_config(content: &str) -> Result<PolicyConfig> {
    let config: PolicyConfig =
        serde_json::from_str(content).wrap_err("parsing policy config json")?;
    validate_config(&config)?;
    Ok(config)
}

fn validate_config(config: &PolicyConfig) -> Result<()> {
    if config.schema_version != SUPPORTED_SCHEMA_VERSION {
        return Err(eyre!(
            "unsupported policy schema_version {}; expected {}",
            config.schema_version,
            SUPPORTED_SCHEMA_VERSION
        ));
    }
    if config.approval_required_model_patterns.is_empty() {
        return Err(eyre!(
            "policy config must define approval_required_model_patterns"
        ));
    }
    Ok(())
}

pub fn evaluate(
    agent: &str,
    model: &str,
    status: AdapterStatus,
    allow_gated: bool,
    config: &PolicyConfig,
) -> PolicyDecision {
    let status_name = adapter_status_name(status);
    if config
        .blocked_adapter_statuses
        .iter()
        .any(|blocked| blocked == status_name)
    {
        return deny(format!("agent {agent} is {status_name}"));
    }

    if config
        .gated_adapter_statuses
        .iter()
        .any(|gated| gated == status_name)
        && !allow_gated
    {
        return deny(format!(
            "agent {agent} is gated; pass --allow-gated after human approval"
        ));
    }

    if model_needs_approval(model, &config.approval_required_model_patterns) && !allow_gated {
        return deny(format!("model {model} requires explicit human approval"));
    }

    PolicyDecision {
        allowed: true,
        reason: "allowed".to_string(),
    }
}

pub fn model_needs_approval(model: &str, approval_patterns: &[String]) -> bool {
    let model = model.to_ascii_lowercase();
    approval_patterns
        .iter()
        .map(|pattern| pattern.to_ascii_lowercase())
        .any(|pattern| model.contains(&pattern))
}

fn adapter_status_name(status: AdapterStatus) -> &'static str {
    match status {
        AdapterStatus::Available => "available",
        AdapterStatus::Missing => "missing",
        AdapterStatus::Gated => "gated",
        AdapterStatus::DeprecatedOrQuarantine => "deprecated_or_quarantine",
    }
}

fn deny(reason: String) -> PolicyDecision {
    PolicyDecision {
        allowed: false,
        reason,
    }
}

#[cfg(test)]
mod tests {
    use super::{default_config, evaluate, parse_config};
    use crate::adapters::AdapterStatus;

    #[test]
    fn blocks_gated_without_approval() {
        let config = default_config().unwrap();
        let decision = evaluate("claude-code", "haiku", AdapterStatus::Gated, false, &config);
        assert!(!decision.allowed);
    }

    #[test]
    fn blocks_sonnet_without_approval() {
        let config = default_config().unwrap();
        let decision = evaluate(
            "pi",
            "claude-sonnet-4",
            AdapterStatus::Available,
            false,
            &config,
        );
        assert!(!decision.allowed);
    }

    #[test]
    fn rejects_empty_approval_patterns() {
        let err = parse_config(
            r#"{"schema_version":1,"approval_required_model_patterns":[],"blocked_adapter_statuses":[],"gated_adapter_statuses":[]}"#,
        )
        .unwrap_err();
        assert!(err.to_string().contains("approval_required_model_patterns"));
    }
}
