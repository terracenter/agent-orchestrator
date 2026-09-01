use crate::adapters::{find_adapter, AdapterStatus};
use color_eyre::eyre::{eyre, Result};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ModelsReport {
    pub schema_version: u8,
    pub agent: String,
    pub detected: bool,
    pub status: AdapterStatus,
    pub models: Vec<ModelCandidate>,
    pub discovery: DiscoveryStatus,
    pub secrets_read: bool,
}

#[derive(Debug, Serialize)]
pub struct ModelCandidate {
    pub id: &'static str,
    pub source: &'static str,
    pub confidence: &'static str,
    pub notes: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveryStatus {
    StaticCatalog,
    Unsupported,
}

pub fn list(agent: &str) -> Result<ModelsReport> {
    let adapter = find_adapter(agent).ok_or_else(|| eyre!("unknown agent adapter: {agent}"))?;
    let detected = adapter.binary_path().is_some();
    let models = static_models(adapter.name());
    let discovery = if models.is_empty() {
        DiscoveryStatus::Unsupported
    } else {
        DiscoveryStatus::StaticCatalog
    };

    Ok(ModelsReport {
        schema_version: 1,
        agent: adapter.name().to_string(),
        detected,
        status: adapter.status(),
        models,
        discovery,
        secrets_read: false,
    })
}

fn static_models(agent: &str) -> Vec<ModelCandidate> {
    match agent {
        "qwen-code" => vec![ModelCandidate {
            id: "qwen3.8-max",
            source: "observed_cli_usage",
            confidence: "candidate",
            notes: "usable by direct qwen CLI, pending Orq registry certification",
        }],
        "pi" => vec![ModelCandidate {
            id: "nvidia/openai/gpt-oss-20b",
            source: "workspace_policy",
            confidence: "candidate",
            notes: "NVIDIA stays under Pi until a separate local runner exists",
        }],
        "claude-code" => vec![ModelCandidate {
            id: "claude-haiku-4-5",
            source: "observed_cli_usage",
            confidence: "candidate_gated",
            notes: "cheap Claude route; Sonnet/Opus remain human-gated",
        }],
        "hermes" => vec![],
        _ => vec![],
    }
}
