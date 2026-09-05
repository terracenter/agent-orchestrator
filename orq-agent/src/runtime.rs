use crate::adapters::{self, AdaptersRegistry};
use crate::models;
use crate::state::{AgentRecord, ModelRecord, StateStore};
use color_eyre::eyre::{Result, WrapErr};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RuntimeProbe {
    pub agent: String,
    pub binary: String,
    pub binary_path: Option<String>,
    pub detected: bool,
    pub version: Option<String>,
    pub providers: Vec<RuntimeProvider>,
    pub models: Vec<RuntimeModel>,
    pub modes: Vec<String>,
    pub tools: Vec<String>,
    pub has_credentials: bool,
    pub probed_at: String,
    pub source: String,
    pub errors: Vec<String>,
    pub secrets_read: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RuntimeProvider {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<RuntimePlan>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub models: Vec<RuntimeModel>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RuntimePlan {
    pub name: String,
    pub active: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub period_start: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub period_end: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_renewal: Option<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RuntimeModel {
    pub id: String,
    pub status: String,
    pub source_type: String,
    pub verified: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_verified_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_hint: Option<RuntimeCostHint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<RuntimeCapabilities>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RuntimeCostHint {
    pub unit: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub promo: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RuntimeCapabilities {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub input_modalities: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub output_modalities: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RuntimeAgentSnapshot {
    pub id: String,
    pub name: String,
    pub vendor: String,
    pub binary: RuntimeBinaryInfo,
    pub config_metadata: RuntimeConfigMetadata,
    pub providers: Vec<RuntimeProvider>,
    pub modes: Vec<String>,
    pub tools: Vec<String>,
    pub probed_at: String,
    pub source: String,
    pub secrets_read: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RuntimeBinaryInfo {
    pub command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub detected: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RuntimeConfigMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings_path: Option<String>,
    pub has_credentials: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_type: Option<String>,
    pub secrets_read: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DoctorReport {
    pub schema_version: u8,
    pub doctor_at: String,
    pub status: String,
    pub exit_code: i32,
    pub components: Vec<DoctorComponent>,
    pub secrets_read: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DoctorComponent {
    pub name: String,
    pub kind: String,
    pub health: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binary_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub details: String,
    pub required: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ModelsSnapshot {
    #[serde(rename = "$schema", skip_serializing_if = "Option::is_none")]
    pub schema_url: Option<String>,
    pub snapshot_id: String,
    pub fetched_at: String,
    pub user_id: String,
    pub source: String,
    pub agents: Vec<RuntimeAgentSnapshot>,
    pub secrets_read: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AgentsDiscoverReport {
    pub schema_version: u8,
    pub snapshot_at: String,
    pub source: String,
    pub config_source: RuntimeDiscoverConfigSource,
    pub state_source: String,
    pub agents: Vec<RuntimeAgentSnapshot>,
    pub agents_persisted: usize,
    pub models_persisted: usize,
    pub secrets_read: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RuntimeDiscoverConfigSource {
    pub adapters: String,
    pub models: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AgentRefreshReport {
    pub schema_version: u8,
    pub refreshed_at: String,
    pub source: String,
    pub agent: RuntimeAgentSnapshot,
    pub state_source: String,
    pub models_persisted: usize,
    pub secrets_read: bool,
}

pub fn parse_version(output: &str) -> Option<String> {
    for line in output.lines() {
        let trimmed = line.trim();
        for raw_token in trimmed.split(|c: char| {
            c.is_whitespace() || c == '/' || c == '@' || c == '(' || c == ')' || c == ','
        }) {
            let clean = raw_token
                .trim_matches(|c: char| !c.is_alphanumeric() && c != '.' && c != '-' && c != '_');
            let candidate = if let Some(stripped) = clean.strip_prefix('v') {
                stripped
            } else if let Some(stripped) = clean.strip_prefix('V') {
                stripped
            } else {
                clean
            };
            if is_semver_like(candidate) {
                return Some(candidate.to_string());
            }
        }
    }
    None
}

fn is_semver_like(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() < 2 {
        return false;
    }
    if !parts[0].chars().all(|c| c.is_ascii_digit()) || parts[0].is_empty() {
        return false;
    }
    let second_digits = parts[1].split('-').next().unwrap_or("");
    if !second_digits.chars().all(|c| c.is_ascii_digit()) || second_digits.is_empty() {
        return false;
    }
    true
}

pub fn resolve_binary_path(env_key: &str, binary_name: &str) -> Option<String> {
    if let Ok(env_path) = std::env::var(env_key) {
        if Path::new(&env_path).exists() {
            Some(env_path)
        } else {
            None
        }
    } else {
        which::which(binary_name)
            .ok()
            .map(|p| p.display().to_string())
    }
}

pub fn check_agent_credentials(agent_name: &str) -> (bool, Option<String>, Option<String>) {
    let local_secrets = Path::new(".secrets").exists();
    let home = std::env::var_os("HOME").map(PathBuf::from);

    match agent_name {
        "qwen" | "qwen-code" => {
            let settings_path = home.as_ref().map(|h| h.join(".qwen").join("settings.json"));
            let exists = settings_path.as_ref().map(|p| p.exists()).unwrap_or(false)
                || home
                    .as_ref()
                    .map(|h| h.join(".qwen").exists())
                    .unwrap_or(false)
                || std::env::var_os("QWEN_API_KEY").is_some()
                || std::env::var_os("BAILIAN_API_KEY").is_some()
                || local_secrets;
            (
                exists,
                settings_path.map(|p| p.display().to_string()),
                Some("token_plan_api_key".to_string()),
            )
        }
        "claude" | "claude-code" => {
            let settings_path = home.as_ref().map(|h| h.join(".claude.json"));
            let exists = settings_path.as_ref().map(|p| p.exists()).unwrap_or(false)
                || home
                    .as_ref()
                    .map(|h| h.join(".claude").exists())
                    .unwrap_or(false)
                || home
                    .as_ref()
                    .map(|h| h.join(".config").join("claude").exists())
                    .unwrap_or(false)
                || std::env::var_os("ANTHROPIC_API_KEY").is_some()
                || local_secrets;
            (
                exists,
                settings_path.map(|p| p.display().to_string()),
                Some("anthropic_api_key".to_string()),
            )
        }
        "hermes" => {
            let config_path = home.as_ref().map(|h| h.join(".hermes").join("config.yaml"));
            let exists = config_path.as_ref().map(|p| p.exists()).unwrap_or(false)
                || home
                    .as_ref()
                    .map(|h| h.join(".hermes").exists())
                    .unwrap_or(false)
                || std::env::var_os("OPENROUTER_API_KEY").is_some()
                || local_secrets;
            (
                exists,
                config_path.map(|p| p.display().to_string()),
                Some("openrouter_api_key".to_string()),
            )
        }
        "agy" => {
            let config_path = home.as_ref().map(|h| h.join(".gemini"));
            let exists = config_path.as_ref().map(|p| p.exists()).unwrap_or(false)
                || std::env::var_os("GEMINI_API_KEY").is_some()
                || local_secrets;
            (
                exists,
                config_path.map(|p| p.display().to_string()),
                Some("gemini_api_key".to_string()),
            )
        }
        _ => {
            let exists = local_secrets;
            (exists, None, None)
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn build_agent_profile(
    agent_name: &str,
    binary_name: &str,
    binary_path: Option<&str>,
    version: Option<&str>,
    detected: bool,
    has_credentials: bool,
    settings_path: Option<&str>,
    auth_type: Option<&str>,
    probed_at: &str,
) -> RuntimeAgentSnapshot {
    let binary = RuntimeBinaryInfo {
        command: binary_name.to_string(),
        path: binary_path.map(ToString::to_string),
        version: version.map(ToString::to_string),
        detected,
    };

    let config_metadata = RuntimeConfigMetadata {
        settings_path: settings_path.map(ToString::to_string),
        has_credentials,
        auth_type: auth_type.map(ToString::to_string),
        secrets_read: false,
    };

    match agent_name {
        "qwen" | "qwen-code" => RuntimeAgentSnapshot {
            id: "qwen-code".to_string(),
            name: "Qwen Code CLI".to_string(),
            vendor: "Alibaba Group".to_string(),
            binary,
            config_metadata,
            modes: vec![
                "chat".to_string(),
                "agentic".to_string(),
                "edit".to_string(),
                "review".to_string(),
                "architecture".to_string(),
            ],
            tools: vec![
                "shell".to_string(),
                "filesystem".to_string(),
                "git".to_string(),
                "docker".to_string(),
                "workspace_skills".to_string(),
            ],
            providers: vec![RuntimeProvider {
                id: "bailian".to_string(),
                name: "Alibaba Cloud Bailian Token Plan".to_string(),
                endpoint: Some("token-plan.ap-southeast-1.maas.aliyuncs.com".to_string()),
                plan: Some(RuntimePlan {
                    name: "Qwen Standard Plan".to_string(),
                    active: true,
                    period_start: Some("2026-08-24T07:31:52Z".to_string()),
                    period_end: Some("2026-09-24T12:00:00Z".to_string()),
                    auto_renewal: Some(false),
                }),
                models: vec![
                    RuntimeModel {
                        id: "qwen3.8-max".to_string(),
                        status: if detected {
                            "active".to_string()
                        } else {
                            "available".to_string()
                        },
                        source_type: "runtime".to_string(),
                        verified: detected,
                        last_verified_at: if detected {
                            Some(probed_at.to_string())
                        } else {
                            None
                        },
                        cost_hint: Some(RuntimeCostHint {
                            unit: "token_plan_quota".to_string(),
                            promo: Some("unlimited_in_plan_window".to_string()),
                        }),
                        capabilities: Some(RuntimeCapabilities {
                            input_modalities: vec![
                                "text".to_string(),
                                "code".to_string(),
                                "files".to_string(),
                                "repo".to_string(),
                            ],
                            output_modalities: vec![
                                "text".to_string(),
                                "code".to_string(),
                                "patch".to_string(),
                                "markdown".to_string(),
                                "json".to_string(),
                            ],
                            tools: vec![
                                "shell".to_string(),
                                "filesystem".to_string(),
                                "git".to_string(),
                                "docker".to_string(),
                                "workspace_skills".to_string(),
                            ],
                            modes: vec![
                                "chat".to_string(),
                                "agentic".to_string(),
                                "edit".to_string(),
                                "review".to_string(),
                                "architecture".to_string(),
                            ],
                            context_window_tokens: Some(131072),
                            max_output_tokens: Some(8192),
                        }),
                    },
                    RuntimeModel {
                        id: "qwen3.7-plus".to_string(),
                        status: "available".to_string(),
                        source_type: "docs_official".to_string(),
                        verified: false,
                        last_verified_at: None,
                        cost_hint: Some(RuntimeCostHint {
                            unit: "token_plan_quota".to_string(),
                            promo: None,
                        }),
                        capabilities: Some(RuntimeCapabilities {
                            input_modalities: vec!["text".to_string(), "code".to_string()],
                            output_modalities: vec![
                                "text".to_string(),
                                "code".to_string(),
                                "markdown".to_string(),
                            ],
                            tools: vec!["shell".to_string(), "filesystem".to_string()],
                            modes: vec!["chat".to_string(), "agentic".to_string()],
                            context_window_tokens: Some(131072),
                            max_output_tokens: Some(8192),
                        }),
                    },
                    RuntimeModel {
                        id: "qwen3.6".to_string(),
                        status: "available".to_string(),
                        source_type: "docs_official".to_string(),
                        verified: false,
                        last_verified_at: None,
                        cost_hint: None,
                        capabilities: None,
                    },
                    RuntimeModel {
                        id: "qwen3.5".to_string(),
                        status: "available".to_string(),
                        source_type: "docs_official".to_string(),
                        verified: false,
                        last_verified_at: None,
                        cost_hint: None,
                        capabilities: None,
                    },
                    RuntimeModel {
                        id: "glm-5".to_string(),
                        status: "available".to_string(),
                        source_type: "docs_official".to_string(),
                        verified: false,
                        last_verified_at: None,
                        cost_hint: None,
                        capabilities: Some(RuntimeCapabilities {
                            input_modalities: vec!["text".to_string(), "code".to_string()],
                            output_modalities: vec![
                                "text".to_string(),
                                "code".to_string(),
                                "markdown".to_string(),
                            ],
                            tools: vec!["shell".to_string(), "filesystem".to_string()],
                            modes: vec!["chat".to_string(), "agentic".to_string()],
                            context_window_tokens: Some(131072),
                            max_output_tokens: Some(8192),
                        }),
                    },
                    RuntimeModel {
                        id: "kimi-k2.5".to_string(),
                        status: "available".to_string(),
                        source_type: "docs_official".to_string(),
                        verified: false,
                        last_verified_at: None,
                        cost_hint: None,
                        capabilities: None,
                    },
                    RuntimeModel {
                        id: "MiniMax-M2.5".to_string(),
                        status: "available".to_string(),
                        source_type: "docs_official".to_string(),
                        verified: false,
                        last_verified_at: None,
                        cost_hint: None,
                        capabilities: None,
                    },
                ],
            }],
            probed_at: probed_at.to_string(),
            source: "runtime_doctor".to_string(),
            secrets_read: false,
        },
        "claude" | "claude-code" => RuntimeAgentSnapshot {
            id: "claude-code".to_string(),
            name: "Claude Code CLI".to_string(),
            vendor: "Anthropic".to_string(),
            binary,
            config_metadata,
            modes: vec![
                "chat".to_string(),
                "agentic".to_string(),
                "edit".to_string(),
                "review".to_string(),
                "architecture".to_string(),
            ],
            tools: vec![
                "shell".to_string(),
                "filesystem".to_string(),
                "git".to_string(),
                "workspace_skills".to_string(),
            ],
            providers: vec![RuntimeProvider {
                id: "anthropic".to_string(),
                name: "Anthropic API".to_string(),
                endpoint: Some("api.anthropic.com".to_string()),
                plan: Some(RuntimePlan {
                    name: "Pay-As-You-Go Promo 50%".to_string(),
                    active: true,
                    period_start: None,
                    period_end: None,
                    auto_renewal: None,
                }),
                models: vec![
                    RuntimeModel {
                        id: "claude-3-7-sonnet".to_string(),
                        status: if detected {
                            "active".to_string()
                        } else {
                            "available".to_string()
                        },
                        source_type: "runtime".to_string(),
                        verified: detected,
                        last_verified_at: if detected {
                            Some(probed_at.to_string())
                        } else {
                            None
                        },
                        cost_hint: Some(RuntimeCostHint {
                            unit: "usd_per_token".to_string(),
                            promo: Some("anthropic+50%".to_string()),
                        }),
                        capabilities: Some(RuntimeCapabilities {
                            input_modalities: vec![
                                "text".to_string(),
                                "code".to_string(),
                                "files".to_string(),
                                "repo".to_string(),
                            ],
                            output_modalities: vec![
                                "text".to_string(),
                                "code".to_string(),
                                "patch".to_string(),
                                "markdown".to_string(),
                                "json".to_string(),
                            ],
                            tools: vec![
                                "shell".to_string(),
                                "filesystem".to_string(),
                                "git".to_string(),
                                "workspace_skills".to_string(),
                            ],
                            modes: vec![
                                "chat".to_string(),
                                "agentic".to_string(),
                                "edit".to_string(),
                                "review".to_string(),
                                "architecture".to_string(),
                            ],
                            context_window_tokens: Some(200000),
                            max_output_tokens: Some(64000),
                        }),
                    },
                    RuntimeModel {
                        id: "claude-3-5-sonnet".to_string(),
                        status: "available".to_string(),
                        source_type: "docs_official".to_string(),
                        verified: false,
                        last_verified_at: None,
                        cost_hint: None,
                        capabilities: None,
                    },
                    RuntimeModel {
                        id: "claude-3-5-haiku".to_string(),
                        status: "available".to_string(),
                        source_type: "docs_official".to_string(),
                        verified: false,
                        last_verified_at: None,
                        cost_hint: None,
                        capabilities: None,
                    },
                ],
            }],
            probed_at: probed_at.to_string(),
            source: "runtime_doctor".to_string(),
            secrets_read: false,
        },
        "hermes" => RuntimeAgentSnapshot {
            id: "hermes".to_string(),
            name: "Hermes Agent CLI".to_string(),
            vendor: "Nous Research".to_string(),
            binary,
            config_metadata,
            modes: vec![
                "chat".to_string(),
                "agentic".to_string(),
                "review".to_string(),
            ],
            tools: vec![
                "shell".to_string(),
                "filesystem".to_string(),
                "git".to_string(),
            ],
            providers: vec![RuntimeProvider {
                id: "openrouter".to_string(),
                name: "OpenRouter API".to_string(),
                endpoint: Some("openrouter.ai/api/v1".to_string()),
                plan: None,
                models: vec![RuntimeModel {
                    id: "hermes-3-llama-3.1-405b".to_string(),
                    status: if detected {
                        "active".to_string()
                    } else {
                        "available".to_string()
                    },
                    source_type: "runtime".to_string(),
                    verified: detected,
                    last_verified_at: if detected {
                        Some(probed_at.to_string())
                    } else {
                        None
                    },
                    cost_hint: None,
                    capabilities: None,
                }],
            }],
            probed_at: probed_at.to_string(),
            source: "runtime_doctor".to_string(),
            secrets_read: false,
        },
        "agy" => RuntimeAgentSnapshot {
            id: "agy".to_string(),
            name: "Google Antigravity CLI".to_string(),
            vendor: "Google DeepMind".to_string(),
            binary,
            config_metadata,
            modes: vec![
                "chat".to_string(),
                "agentic".to_string(),
                "edit".to_string(),
                "review".to_string(),
                "architecture".to_string(),
            ],
            tools: vec![
                "shell".to_string(),
                "filesystem".to_string(),
                "git".to_string(),
                "workspace_skills".to_string(),
            ],
            providers: vec![RuntimeProvider {
                id: "google".to_string(),
                name: "Google AI".to_string(),
                endpoint: None,
                plan: None,
                models: vec![
                    RuntimeModel {
                        id: "gemini-3.7-flash".to_string(),
                        status: if detected {
                            "active".to_string()
                        } else {
                            "available".to_string()
                        },
                        source_type: "runtime".to_string(),
                        verified: detected,
                        last_verified_at: if detected {
                            Some(probed_at.to_string())
                        } else {
                            None
                        },
                        cost_hint: None,
                        capabilities: None,
                    },
                    RuntimeModel {
                        id: "gemini-2.5-pro".to_string(),
                        status: "available".to_string(),
                        source_type: "docs_official".to_string(),
                        verified: false,
                        last_verified_at: None,
                        cost_hint: None,
                        capabilities: None,
                    },
                ],
            }],
            probed_at: probed_at.to_string(),
            source: "runtime_doctor".to_string(),
            secrets_read: false,
        },
        _ => RuntimeAgentSnapshot {
            id: agent_name.to_string(),
            name: format!("{agent_name} Runner"),
            vendor: "Custom".to_string(),
            binary,
            config_metadata,
            modes: vec!["chat".to_string(), "agentic".to_string()],
            tools: vec!["shell".to_string(), "filesystem".to_string()],
            providers: vec![RuntimeProvider {
                id: format!("{agent_name}-provider"),
                name: format!("{agent_name} Backend"),
                endpoint: None,
                plan: None,
                models: vec![RuntimeModel {
                    id: format!("{agent_name}-default"),
                    status: if detected {
                        "active".to_string()
                    } else {
                        "available".to_string()
                    },
                    source_type: if detected {
                        "runtime".to_string()
                    } else {
                        "docs_official".to_string()
                    },
                    verified: detected,
                    last_verified_at: if detected {
                        Some(probed_at.to_string())
                    } else {
                        None
                    },
                    cost_hint: None,
                    capabilities: None,
                }],
            }],
            probed_at: probed_at.to_string(),
            source: "runtime_doctor".to_string(),
            secrets_read: false,
        },
    }
}

pub async fn probe_agent(
    agent_name: &str,
    binary_name_or_path: &str,
    timeout_secs: u64,
) -> RuntimeProbe {
    let now = models::now_iso8601();
    let (has_credentials, settings_path, auth_type) = check_agent_credentials(agent_name);

    let env_name = format!(
        "ORQ_AGENT_BIN_{}",
        agent_name.replace('-', "_").to_ascii_uppercase()
    );
    let resolved_path = resolve_binary_path(&env_name, binary_name_or_path);

    let detected = resolved_path.is_some();
    let mut version = None;
    let mut errors = Vec::new();

    if let Some(ref bin_path) = resolved_path {
        let timeout_duration = std::time::Duration::from_secs(timeout_secs.clamp(1, 15));
        match tokio::time::timeout(
            timeout_duration,
            tokio::process::Command::new(bin_path)
                .arg("--version")
                .output(),
        )
        .await
        {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let combined = format!("{stdout}\n{stderr}");
                version = parse_version(&combined);
                if version.is_none() && !output.status.success() {
                    errors.push(format!(
                        "binary --version exited with status: {}",
                        output.status
                    ));
                }
            }
            Ok(Err(err)) => {
                errors.push(format!("failed to execute binary {bin_path}: {err}"));
            }
            Err(_) => {
                errors.push(format!("binary {bin_path} --version timed out"));
            }
        }
    }

    let snapshot_agent = build_agent_profile(
        agent_name,
        binary_name_or_path,
        resolved_path.as_deref(),
        version.as_deref(),
        detected,
        has_credentials,
        settings_path.as_deref(),
        auth_type.as_deref(),
        &now,
    );

    let all_models: Vec<RuntimeModel> = snapshot_agent
        .providers
        .iter()
        .flat_map(|p| p.models.clone())
        .collect();

    RuntimeProbe {
        agent: agent_name.to_string(),
        binary: binary_name_or_path.to_string(),
        binary_path: resolved_path,
        detected,
        version,
        providers: snapshot_agent.providers,
        models: all_models,
        modes: snapshot_agent.modes,
        tools: snapshot_agent.tools,
        has_credentials,
        probed_at: now,
        source: "runtime_doctor".to_string(),
        errors,
        secrets_read: false,
    }
}

pub async fn doctor_health_check(adapters_registry: &AdaptersRegistry) -> Result<DoctorReport> {
    let now = models::now_iso8601();
    let mut components = Vec::new();
    let mut has_missing_required = false;

    // 1. Check adapters from registry
    for adapter in &adapters_registry.adapters {
        let env_name = format!(
            "ORQ_AGENT_BIN_{}",
            adapter.name.replace('-', "_").to_ascii_uppercase()
        );
        let path = resolve_binary_path(&env_name, &adapter.binary);

        let detected = path.is_some();
        let mut version = None;
        if let Some(ref p) = path {
            if let Ok(Ok(out)) = tokio::time::timeout(
                std::time::Duration::from_secs(3),
                tokio::process::Command::new(p).arg("--version").output(),
            )
            .await
            {
                let combined = format!(
                    "{}\n{}",
                    String::from_utf8_lossy(&out.stdout),
                    String::from_utf8_lossy(&out.stderr)
                );
                version = parse_version(&combined);
            }
        }

        let health = if detected {
            "ok".to_string()
        } else {
            "missing".to_string()
        };

        components.push(DoctorComponent {
            name: format!("agent:{}", adapter.name),
            kind: "agent".to_string(),
            health,
            binary_path: path.clone(),
            version,
            details: if detected {
                format!(
                    "binary {} found at {}",
                    adapter.binary,
                    path.unwrap_or_default()
                )
            } else {
                format!("binary {} not found in PATH", adapter.binary)
            },
            required: false,
        });
    }

    // 2. Check wrappers: rtk, vg, engram
    let wrappers = [
        ("wrapper:rtk", "rtk", true),
        ("wrapper:vg", "vg", true),
        ("wrapper:engram", "engram", true),
    ];

    for (comp_name, bin_name, required) in wrappers {
        let custom_env = format!("ORQ_{}_BIN", bin_name.to_ascii_uppercase());
        let path = resolve_binary_path(&custom_env, bin_name);

        let detected = path.is_some();
        let mut version = None;
        if let Some(ref p) = path {
            if let Ok(Ok(out)) = tokio::time::timeout(
                std::time::Duration::from_secs(3),
                tokio::process::Command::new(p).arg("--version").output(),
            )
            .await
            {
                let combined = format!(
                    "{}\n{}",
                    String::from_utf8_lossy(&out.stdout),
                    String::from_utf8_lossy(&out.stderr)
                );
                version = parse_version(&combined);
            }
        }

        let health = if detected {
            "ok".to_string()
        } else {
            if required {
                has_missing_required = true;
            }
            "missing".to_string()
        };

        components.push(DoctorComponent {
            name: comp_name.to_string(),
            kind: "wrapper".to_string(),
            health,
            binary_path: path.clone(),
            version,
            details: if detected {
                format!(
                    "wrapper {} available at {}",
                    bin_name,
                    path.unwrap_or_default()
                )
            } else {
                format!("required wrapper {bin_name} missing from PATH")
            },
            required,
        });
    }

    // 3. Check sandbox: temp_dir & docker
    let temp_dir = std::env::temp_dir();
    let temp_writable = temp_dir.exists() && {
        let probe_file = temp_dir.join(format!(".orq-doctor-probe-{}", std::process::id()));
        if std::fs::write(&probe_file, b"ok").is_ok() {
            let _ = std::fs::remove_file(&probe_file);
            true
        } else {
            false
        }
    };

    components.push(DoctorComponent {
        name: "sandbox:temp_dir".to_string(),
        kind: "sandbox".to_string(),
        health: if temp_writable {
            "ok".to_string()
        } else {
            "degraded".to_string()
        },
        binary_path: Some(temp_dir.display().to_string()),
        version: None,
        details: if temp_writable {
            format!("temp dir {} is writable", temp_dir.display())
        } else {
            format!("temp dir {} is not writable", temp_dir.display())
        },
        required: true,
    });
    if !temp_writable {
        has_missing_required = true;
    }

    let docker_path = which::which("docker").ok().map(|p| p.display().to_string());
    components.push(DoctorComponent {
        name: "sandbox:docker".to_string(),
        kind: "sandbox".to_string(),
        health: if docker_path.is_some() {
            "ok".to_string()
        } else {
            "missing".to_string()
        },
        binary_path: docker_path.clone(),
        version: None,
        details: if docker_path.is_some() {
            format!(
                "docker CLI available at {}",
                docker_path.unwrap_or_default()
            )
        } else {
            "docker CLI not found (optional container sandbox)".to_string()
        },
        required: false,
    });

    let (status, exit_code) = if has_missing_required {
        ("missing".to_string(), 1)
    } else {
        ("ok".to_string(), 0)
    };

    Ok(DoctorReport {
        schema_version: 1,
        doctor_at: now,
        status,
        exit_code,
        components,
        secrets_read: false,
    })
}

pub async fn run_models_snapshot(
    output_path: Option<&Path>,
    adapters_config: Option<&Path>,
    models_config: Option<&Path>,
) -> Result<ModelsSnapshot> {
    let (adapters_registry, _) = adapters::load_registry(adapters_config).await?;
    let (models_catalog, _) = models::load_catalog(models_config).await?;

    let now = models::now_iso8601();
    let user_id = std::env::var("ORQ_USER_ID")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "freddy".to_string());

    let snapshot_id = format!(
        "snap-{}-{}",
        now.replace(['-', ':', 'T', 'Z'], ""),
        hostname_label()
    );

    let mut agent_snapshots = Vec::new();
    for adapter in &adapters_registry.adapters {
        let probe = probe_agent(&adapter.name, &adapter.binary, 5).await;
        let mut snapshot_agent = build_agent_profile(
            &adapter.name,
            &adapter.binary,
            probe.binary_path.as_deref(),
            probe.version.as_deref(),
            probe.detected,
            probe.has_credentials,
            None,
            None,
            &now,
        );

        if let Some(cat_models) = models_catalog.agents.get(&adapter.name) {
            for cat_m in cat_models {
                let mut found = false;
                for p in &mut snapshot_agent.providers {
                    if p.models.iter().any(|m| m.id == cat_m.id) {
                        found = true;
                        break;
                    }
                }
                if !found {
                    if let Some(p) = snapshot_agent.providers.first_mut() {
                        p.models.push(RuntimeModel {
                            id: cat_m.id.clone(),
                            status: cat_m
                                .status
                                .clone()
                                .unwrap_or_else(|| "available".to_string()),
                            source_type: cat_m.source.clone(),
                            verified: false,
                            last_verified_at: cat_m.fetched_at.clone(),
                            cost_hint: cat_m.cost_hint.map(|_c| RuntimeCostHint {
                                unit: "usd_per_token".to_string(),
                                promo: cat_m.promo.clone(),
                            }),
                            capabilities: None,
                        });
                    }
                }
            }
        }

        agent_snapshots.push(snapshot_agent);
    }

    let snapshot = ModelsSnapshot {
        schema_url: Some(
            "https://json-schema.terracenter.net/orq/v2.1/agent-model-discovery.json".to_string(),
        ),
        snapshot_id,
        fetched_at: now,
        user_id,
        source: "runtime_doctor".to_string(),
        agents: agent_snapshots,
        secrets_read: false,
    };

    if let Some(out_path) = output_path {
        let json_str = serde_json::to_string_pretty(&snapshot)?;
        tokio::fs::write(out_path, json_str)
            .await
            .wrap_err_with(|| format!("writing snapshot to {}", out_path.display()))?;
    }

    Ok(snapshot)
}

pub fn persist_runtime_agent(store: &StateStore, agent: &RuntimeAgentSnapshot) -> Result<usize> {
    let metadata_json = serde_json::json!({
        "binary": agent.binary,
        "config_metadata": agent.config_metadata,
        "probed_at": agent.probed_at,
        "source": agent.source,
        "secrets_read": false
    })
    .to_string();

    store
        .upsert_agent(&AgentRecord {
            agent_id: agent.id.clone(),
            display_name: agent.name.clone(),
            adapter_status: if agent.binary.detected {
                "available".to_string()
            } else {
                "missing".to_string()
            },
            metadata_json,
        })
        .wrap_err_with(|| format!("persisting agent {}", agent.id))?;

    let mut models_persisted = 0;
    for provider in &agent.providers {
        for model in &provider.models {
            let task_kind = "general".to_string();
            let model_metadata = serde_json::json!({
                "provider_id": provider.id,
                "provider_name": provider.name,
                "source_type": model.source_type,
                "verified": model.verified,
                "last_verified_at": model.last_verified_at,
                "cost_hint": model.cost_hint,
                "capabilities": model.capabilities,
                "source": "runtime_doctor",
                "secrets_read": false
            })
            .to_string();

            store
                .upsert_model(&ModelRecord {
                    agent_id: agent.id.clone(),
                    model_id: model.id.clone(),
                    task_kind,
                    gated: model.status == "gated",
                    active: model.verified || model.status == "active",
                    metadata_json: model_metadata,
                })
                .wrap_err_with(|| {
                    format!("persisting model {} for agent {}", model.id, agent.id)
                })?;
            models_persisted += 1;
        }
    }

    Ok(models_persisted)
}

fn hostname_label() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("HOST"))
        .unwrap_or_else(|_| "host01".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static TEST_ENV_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn parse_version_extracts_semver() {
        assert_eq!(
            parse_version("qwen 1.4.2 (build 2026-08-24)"),
            Some("1.4.2".to_string())
        );
        assert_eq!(
            parse_version("claude-code/1.0.18 darwin-arm64"),
            Some("1.0.18".to_string())
        );
        assert_eq!(
            parse_version("hermes v0.5.1-beta.2"),
            Some("0.5.1-beta.2".to_string())
        );
        assert_eq!(parse_version("agy 2.0.0"), Some("2.0.0".to_string()));
        assert_eq!(parse_version("rtk 0.3.0"), Some("0.3.0".to_string()));
        assert_eq!(parse_version("some tool without version"), None);
    }

    #[tokio::test]
    async fn probe_agent_with_fake_binary() {
        let _guard = TEST_ENV_MUTEX.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let fake_bin = dir.path().join("fake-qwen");
        std::fs::write(&fake_bin, "#!/usr/bin/env bash\necho 'qwen 1.4.2'\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&fake_bin).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&fake_bin, perms).unwrap();
        }

        let env_key = "ORQ_AGENT_BIN_QWEN_CODE";
        std::env::set_var(env_key, fake_bin.to_str().unwrap());

        let probe = probe_agent("qwen-code", "fake-qwen", 5).await;
        std::env::remove_var(env_key);

        assert_eq!(probe.agent, "qwen-code");
        assert!(probe.detected);
        assert_eq!(probe.version, Some("1.4.2".to_string()));
        assert!(!probe.secrets_read);
        assert_eq!(probe.source, "runtime_doctor");
        assert!(probe.probed_at.contains('T'));
        assert!(probe.probed_at.ends_with('Z'));
    }

    #[tokio::test]
    async fn probe_agent_graceful_degradation_on_missing_binary() {
        let _guard = TEST_ENV_MUTEX.lock().unwrap();
        let probe = probe_agent("non-existent-agent", "definitely-not-a-bin-xyz123", 1).await;
        assert!(!probe.detected);
        assert_eq!(probe.version, None);
        assert!(!probe.secrets_read);
        assert_eq!(probe.source, "runtime_doctor");
    }

    #[tokio::test]
    async fn doctor_health_check_detects_wrappers() {
        let _guard = TEST_ENV_MUTEX.lock().unwrap();
        let registry = adapters::parse_registry(
            r#"{"schema_version":1,"adapters":[{"name":"qwen-code","binary":"qwen","status":"available","argv":["$MODEL","$TASK"]}]}"#,
        )
        .unwrap();

        let dir = tempfile::tempdir().unwrap();
        let fake_rtk = dir.path().join("rtk");
        let fake_vg = dir.path().join("vg");
        let fake_engram = dir.path().join("engram");

        std::fs::write(&fake_rtk, "#!/usr/bin/env bash\necho 'rtk 0.3.0'\n").unwrap();
        std::fs::write(&fake_vg, "#!/usr/bin/env bash\necho 'vg 1.0.0'\n").unwrap();
        std::fs::write(&fake_engram, "#!/usr/bin/env bash\necho 'engram 0.4.1'\n").unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            for file in [&fake_rtk, &fake_vg, &fake_engram] {
                let mut perms = std::fs::metadata(file).unwrap().permissions();
                perms.set_mode(0o755);
                std::fs::set_permissions(file, perms).unwrap();
            }
        }

        std::env::set_var("ORQ_RTK_BIN", fake_rtk.to_str().unwrap());
        std::env::set_var("ORQ_VG_BIN", fake_vg.to_str().unwrap());
        std::env::set_var("ORQ_ENGRAM_BIN", fake_engram.to_str().unwrap());

        let report = doctor_health_check(&registry).await.unwrap();

        std::env::remove_var("ORQ_RTK_BIN");
        std::env::remove_var("ORQ_VG_BIN");
        std::env::remove_var("ORQ_ENGRAM_BIN");

        assert!(!report.secrets_read);
        assert_eq!(report.exit_code, 0);
        assert_eq!(report.status, "ok");

        let rtk_comp = report
            .components
            .iter()
            .find(|c| c.name == "wrapper:rtk")
            .unwrap();
        assert_eq!(rtk_comp.health, "ok");
        assert_eq!(rtk_comp.version, Some("0.3.0".to_string()));
    }

    #[tokio::test]
    async fn doctor_health_check_fails_when_wrapper_missing() {
        let _guard = TEST_ENV_MUTEX.lock().unwrap();
        let registry = adapters::parse_registry(
            r#"{"schema_version":1,"adapters":[{"name":"qwen-code","binary":"qwen","status":"available","argv":["$MODEL","$TASK"]}]}"#,
        )
        .unwrap();

        std::env::set_var("ORQ_RTK_BIN", "/tmp/non-existent-rtk-bin-12345");
        let report = doctor_health_check(&registry).await.unwrap();
        std::env::remove_var("ORQ_RTK_BIN");

        assert_eq!(report.exit_code, 1);
        assert_eq!(report.status, "missing");
        let rtk_comp = report
            .components
            .iter()
            .find(|c| c.name == "wrapper:rtk")
            .unwrap();
        assert_eq!(rtk_comp.health, "missing");
    }

    #[test]
    fn persist_runtime_agent_records_to_store() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("state.sqlite");
        let store = crate::state::open(Some(&db_path)).unwrap();

        let agent = build_agent_profile(
            "qwen-code",
            "qwen",
            Some("/usr/local/bin/qwen"),
            Some("1.4.2"),
            true,
            true,
            Some("/home/user/.qwen/settings.json"),
            Some("token_plan_api_key"),
            "2026-09-04T22:00:00Z",
        );

        let count = persist_runtime_agent(&store, &agent).unwrap();
        assert!(count > 0);

        let stored_agent = store.find_agent("qwen-code").unwrap().unwrap();
        assert_eq!(stored_agent.agent_id, "qwen-code");
        assert_eq!(stored_agent.adapter_status, "available");

        let stored_model = store
            .find_model("qwen-code", "qwen3.8-max", "general")
            .unwrap()
            .unwrap();
        assert_eq!(stored_model.model_id, "qwen3.8-max");
        assert!(stored_model.active);
    }
}
