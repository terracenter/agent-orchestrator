use crate::adapters::AdapterStatus;
use color_eyre::eyre::{eyre, Result, WrapErr};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

const SUPPORTED_SCHEMA_VERSION: u8 = 1;
pub const BUILTIN_POLICY_JSON: &str = include_str!("../config/policy.json");

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct PolicyConfig {
    pub schema_version: u8,
    pub approval_required_model_patterns: Vec<String>,
    pub blocked_adapter_statuses: Vec<String>,
    pub gated_adapter_statuses: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct LoadedPolicy {
    pub config: PolicyConfig,
    pub source: String,
    pub path: String,
    pub sha256: String,
}

#[derive(Debug, Serialize)]
pub struct PolicyDecision {
    pub allowed: bool,
    pub reason: String,
}

pub fn default_config() -> Result<PolicyConfig> {
    parse_config(BUILTIN_POLICY_JSON)
}

pub fn default_loaded_policy() -> Result<LoadedPolicy> {
    let config = default_config()?;
    let sha256 = hex_sha256(BUILTIN_POLICY_JSON.as_bytes());
    Ok(LoadedPolicy {
        config,
        source: "builtin".to_string(),
        path: "builtin".to_string(),
        sha256,
    })
}

pub async fn load_config(path: Option<&Path>) -> Result<LoadedPolicy> {
    match path {
        None => default_loaded_policy(),
        Some(path) => {
            let content = tokio::fs::read_to_string(path)
                .await
                .wrap_err_with(|| format!("reading policy config {}", path.display()))?;
            let config = parse_config(&content)?;
            let sha256 = hex_sha256(content.as_bytes());
            Ok(LoadedPolicy {
                config,
                source: "override".to_string(),
                path: path.display().to_string(),
                sha256,
            })
        }
    }
}

pub fn hex_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
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
    use super::{default_config, default_loaded_policy, evaluate, load_config, parse_config};
    use crate::adapters::AdapterStatus;
    use tempfile::NamedTempFile;

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

    #[tokio::test]
    async fn default_loaded_policy_is_builtin() {
        let loaded = default_loaded_policy().unwrap();
        assert_eq!(loaded.source, "builtin");
        assert_eq!(loaded.path, "builtin");
        assert_eq!(loaded.sha256.len(), 64);
        assert!(loaded
            .config
            .approval_required_model_patterns
            .contains(&"sonnet".to_string()));
    }

    #[tokio::test]
    async fn load_config_none_ignores_env_var() {
        std::env::set_var("ORQ_POLICY_CONFIG", "/nonexistent/insecure_policy.json");
        let loaded = load_config(None).await.unwrap();
        assert_eq!(loaded.source, "builtin");
        assert_eq!(loaded.path, "builtin");
        std::env::remove_var("ORQ_POLICY_CONFIG");
    }

    #[tokio::test]
    async fn load_config_with_valid_override() {
        use std::io::Write;
        let mut temp = NamedTempFile::new().unwrap();
        writeln!(
            temp,
            r#"{{"schema_version":1,"approval_required_model_patterns":["custom-restricted"],"blocked_adapter_statuses":["deprecated_or_quarantine"],"gated_adapter_statuses":["gated"]}}"#
        )
        .unwrap();

        let loaded = load_config(Some(temp.path())).await.unwrap();
        assert_eq!(loaded.source, "override");
        assert_eq!(loaded.path, temp.path().display().to_string());
        assert_eq!(
            loaded.config.approval_required_model_patterns,
            vec!["custom-restricted".to_string()]
        );
    }
}
