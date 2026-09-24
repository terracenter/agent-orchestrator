use crate::adapters::AdapterStatus;
use color_eyre::eyre::{eyre, Result, WrapErr};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

const SUPPORTED_SCHEMA_VERSION: u8 = 1;
pub const POLICY_CONFIG_ENV: &str = "ORQ_POLICY_CONFIG";
pub const POLICY_CONFIG_FILE: &str = "policy.json";

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

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyApproval {
    #[serde(default)]
    pub allow_gated_adapter: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approve_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approve_reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PolicyDecision {
    pub allowed: bool,
    pub reason: String,
}

pub fn default_config() -> Result<PolicyConfig> {
    let path = crate::config::resolve_path(None, POLICY_CONFIG_ENV, POLICY_CONFIG_FILE)?;
    let content = std::fs::read_to_string(&path)
        .wrap_err_with(|| format!("reading external policy config {}", path.display()))?;
    parse_config(&content)
}

pub fn default_loaded_policy() -> Result<LoadedPolicy> {
    let path = crate::config::resolve_path(None, POLICY_CONFIG_ENV, POLICY_CONFIG_FILE)?;
    let content = std::fs::read_to_string(&path)
        .wrap_err_with(|| format!("reading external policy config {}", path.display()))?;
    let config = parse_config(&content)?;
    let sha256 = hex_sha256(content.as_bytes());
    Ok(LoadedPolicy {
        config,
        source: path.display().to_string(),
        path: path.display().to_string(),
        sha256,
    })
}

pub async fn load_config(path: Option<&Path>) -> Result<LoadedPolicy> {
    let (content, source) =
        crate::config::read_json(path, POLICY_CONFIG_ENV, POLICY_CONFIG_FILE, "policy config")
            .await?;
    let config = parse_config(&content)?;
    Ok(LoadedPolicy {
        config,
        source: source.clone(),
        path: source,
        sha256: hex_sha256(content.as_bytes()),
    })
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
    approval: &PolicyApproval,
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

    let is_gated_adapter = config
        .gated_adapter_statuses
        .iter()
        .any(|gated| gated == status_name);

    if is_gated_adapter {
        if !approval.allow_gated_adapter {
            return deny(format!(
                "agent {agent} is gated; pass --allow-gated-adapter after human approval"
            ));
        }
        if approval
            .approve_reason
            .as_deref()
            .unwrap_or("")
            .trim()
            .is_empty()
        {
            return deny(
                "gated adapter approval requires a non-empty reason via --approve-reason"
                    .to_string(),
            );
        }
    }

    if model_needs_approval(model, &config.approval_required_model_patterns) {
        let is_model_approved = approval
            .approve_model
            .as_deref()
            .map(|m| m.trim().eq_ignore_ascii_case(model.trim()))
            .unwrap_or(false);
        if !is_model_approved {
            return deny(format!(
                "model {model} requires explicit human approval via --approve-model"
            ));
        }
        if approval
            .approve_reason
            .as_deref()
            .unwrap_or("")
            .trim()
            .is_empty()
        {
            return deny(
                "model approval requires a non-empty reason via --approve-reason".to_string(),
            );
        }
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
    use super::{
        default_config, default_loaded_policy, evaluate, load_config, parse_config, PolicyApproval,
    };
    use crate::adapters::AdapterStatus;
    use tempfile::NamedTempFile;

    #[test]
    fn blocks_gated_without_approval() {
        let config = default_config().unwrap();
        let approval = PolicyApproval::default();
        let decision = evaluate(
            "claude-code",
            "haiku",
            AdapterStatus::Gated,
            &approval,
            &config,
        );
        assert!(!decision.allowed);
        assert!(decision.reason.contains("--allow-gated-adapter"));
    }

    #[test]
    fn blocks_gated_with_flag_but_missing_reason() {
        let config = default_config().unwrap();
        let approval = PolicyApproval {
            allow_gated_adapter: true,
            approve_model: None,
            approve_reason: None,
        };
        let decision = evaluate(
            "claude-code",
            "haiku",
            AdapterStatus::Gated,
            &approval,
            &config,
        );
        assert!(!decision.allowed);
        assert!(decision.reason.contains("--approve-reason"));
    }

    #[test]
    fn blocks_sonnet_without_approval() {
        let config = default_config().unwrap();
        let approval = PolicyApproval::default();
        let decision = evaluate(
            "pi",
            "claude-sonnet-4",
            AdapterStatus::Available,
            &approval,
            &config,
        );
        assert!(!decision.allowed);
        assert!(decision.reason.contains("--approve-model"));
    }

    #[test]
    fn blocks_sonnet_with_wrong_approved_model() {
        let config = default_config().unwrap();
        let approval = PolicyApproval {
            allow_gated_adapter: false,
            approve_model: Some("other-model".to_string()),
            approve_reason: Some("emergency".to_string()),
        };
        let decision = evaluate(
            "pi",
            "claude-sonnet-4",
            AdapterStatus::Available,
            &approval,
            &config,
        );
        assert!(!decision.allowed);
        assert!(decision.reason.contains("--approve-model"));
    }

    #[test]
    fn blocks_sonnet_with_model_approved_but_missing_reason() {
        let config = default_config().unwrap();
        let approval = PolicyApproval {
            allow_gated_adapter: false,
            approve_model: Some("claude-sonnet-4".to_string()),
            approve_reason: None,
        };
        let decision = evaluate(
            "pi",
            "claude-sonnet-4",
            AdapterStatus::Available,
            &approval,
            &config,
        );
        assert!(!decision.allowed);
        assert!(decision.reason.contains("--approve-reason"));
    }

    #[test]
    fn allow_gated_adapter_does_not_unlock_restricted_model() {
        let config = default_config().unwrap();
        let approval = PolicyApproval {
            allow_gated_adapter: true,
            approve_model: None,
            approve_reason: Some("adapter approved".to_string()),
        };
        let decision = evaluate(
            "claude-code",
            "claude-sonnet-4",
            AdapterStatus::Gated,
            &approval,
            &config,
        );
        assert!(!decision.allowed);
        assert!(decision.reason.contains("--approve-model"));
    }

    #[test]
    fn approve_model_does_not_unlock_gated_adapter() {
        let config = default_config().unwrap();
        let approval = PolicyApproval {
            allow_gated_adapter: false,
            approve_model: Some("claude-sonnet-4".to_string()),
            approve_reason: Some("model approved".to_string()),
        };
        let decision = evaluate(
            "claude-code",
            "claude-sonnet-4",
            AdapterStatus::Gated,
            &approval,
            &config,
        );
        assert!(!decision.allowed);
        assert!(decision.reason.contains("--allow-gated-adapter"));
    }

    #[test]
    fn allows_gated_and_restricted_model_with_full_approvals() {
        let config = default_config().unwrap();
        let approval = PolicyApproval {
            allow_gated_adapter: true,
            approve_model: Some("claude-sonnet-4".to_string()),
            approve_reason: Some("authorized incident analysis".to_string()),
        };
        let decision = evaluate(
            "claude-code",
            "claude-sonnet-4",
            AdapterStatus::Gated,
            &approval,
            &config,
        );
        assert!(decision.allowed);
        assert_eq!(decision.reason, "allowed");
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
        assert_ne!(loaded.source, "builtin");
        assert!(loaded.path.ends_with("policy.json"));
        assert_eq!(loaded.sha256.len(), 64);
        assert!(loaded
            .config
            .approval_required_model_patterns
            .contains(&"sonnet".to_string()));
    }

    #[tokio::test]
    async fn load_config_none_uses_external_config() {
        let loaded = load_config(None).await.unwrap();
        assert_ne!(loaded.source, "builtin");
        assert!(loaded.path.ends_with("policy.json"));
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
        assert_eq!(loaded.source, temp.path().display().to_string());
        assert_eq!(loaded.path, temp.path().display().to_string());
        assert_eq!(
            loaded.config.approval_required_model_patterns,
            vec!["custom-restricted".to_string()]
        );
    }
}
