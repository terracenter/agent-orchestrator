use std::path::{Path, PathBuf};

use color_eyre::eyre::{eyre, Result, WrapErr};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::models;
use crate::runtime::{self, ModelsSnapshot};

#[derive(Clone, Debug, Default)]
pub struct ObserverEmitArgs {
    pub endpoint: Option<String>,
    pub token_file: Option<String>,
    pub dry_run: bool,
    pub output: Option<String>,
    pub host_ip: Option<String>,
    pub adapters_config: Option<String>,
    pub models_config: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ObserverAgentSummary {
    pub agent_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_expires_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ObserverPayload {
    pub snapshot_id: String,
    pub discovered_agents_count: usize,
    pub active_models_count: usize,
    pub agents_summary: Vec<ObserverAgentSummary>,
    pub verification_signature: String,
}

#[derive(Serialize)]
struct ObserverPayloadCore<'a> {
    snapshot_id: &'a str,
    discovered_agents_count: usize,
    active_models_count: usize,
    agents_summary: &'a [ObserverAgentSummary],
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ObserverEvent {
    pub event_type: String,
    pub timestamp: String,
    pub host: String,
    pub host_ip: String,
    pub payload: ObserverPayload,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ObserverEmitReport {
    pub status: String,
    pub event_type: String,
    pub snapshot_id: String,
    pub endpoint: String,
    pub discovered_agents_count: usize,
    pub active_models_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_status: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_path: Option<String>,
    pub secrets_read: bool,
}

fn hex_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn hostname_label() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("HOST"))
        .unwrap_or_else(|_| "host01".to_string())
}

pub fn build_observer_event(
    snapshot: &ModelsSnapshot,
    host_ip: Option<&str>,
) -> Result<ObserverEvent> {
    let mut active_models_count = 0;
    let mut agents_summary = Vec::new();

    for agent in &snapshot.agents {
        for provider in &agent.providers {
            for model in &provider.models {
                if model.status == "active" {
                    active_models_count += 1;
                }
            }
        }

        let first_provider = agent.providers.first();
        let provider_id = first_provider.map(|p| p.id.clone());

        let active_model = agent
            .providers
            .iter()
            .flat_map(|p| p.models.iter())
            .find(|m| m.status == "active")
            .map(|m| m.id.clone())
            .or_else(|| {
                first_provider
                    .and_then(|p| p.models.first())
                    .map(|m| m.id.clone())
            });

        let plan_status = first_provider
            .and_then(|p| p.plan.as_ref())
            .map(|pl| pl.name.clone());

        let plan_expires_at = first_provider
            .and_then(|p| p.plan.as_ref())
            .and_then(|pl| pl.period_end.clone());

        agents_summary.push(ObserverAgentSummary {
            agent_id: agent.id.clone(),
            version: agent.binary.version.clone(),
            provider: provider_id,
            active_model,
            plan_status,
            plan_expires_at,
        });
    }

    let core = ObserverPayloadCore {
        snapshot_id: &snapshot.snapshot_id,
        discovered_agents_count: snapshot.agents.len(),
        active_models_count,
        agents_summary: &agents_summary,
    };
    let core_bytes =
        serde_json::to_vec(&core).wrap_err("serializing observer payload core for signature")?;
    let verification_signature = format!("sha256:{}", hex_sha256(&core_bytes));

    let payload = ObserverPayload {
        snapshot_id: snapshot.snapshot_id.clone(),
        discovered_agents_count: snapshot.agents.len(),
        active_models_count,
        agents_summary,
        verification_signature,
    };

    Ok(ObserverEvent {
        event_type: "agent.discovery.snapshot".to_string(),
        timestamp: models::now_iso8601(),
        host: hostname_label(),
        host_ip: host_ip.unwrap_or("127.0.0.1").to_string(),
        payload,
    })
}

pub fn load_host_token(path: &Path) -> Result<String> {
    if !path.exists() {
        return Err(eyre!("host token file not found at {}", path.display()));
    }
    let content = std::fs::read_to_string(path)
        .wrap_err_with(|| format!("reading host token file from {}", path.display()))?;
    let token = content.trim().to_string();
    if token.is_empty() {
        return Err(eyre!("host token file is empty at {}", path.display()));
    }
    Ok(token)
}

pub fn resolve_token_file(arg: Option<&str>) -> PathBuf {
    if let Some(path) = arg {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    if let Ok(env_path) = std::env::var("ORQ_OBSERVER_TOKEN_FILE") {
        let trimmed = env_path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".config")
        .join("sge-observer")
        .join("agent-orchestrator.host-token")
}

pub fn resolve_endpoint(arg: Option<&str>) -> String {
    if let Some(ep) = arg {
        let trimmed = ep.trim();
        if !trimmed.is_empty() {
            return trimmed.trim_end_matches('/').to_string();
        }
    }
    if let Ok(ep) = std::env::var("ORQ_OBSERVER_ENDPOINT") {
        let trimmed = ep.trim();
        if !trimmed.is_empty() {
            return trimmed.trim_end_matches('/').to_string();
        }
    }
    "https://sge-panel.humanbyte.net".to_string()
}

pub async fn emit(args: ObserverEmitArgs) -> Result<ObserverEmitReport> {
    let adapters_config_path = args.adapters_config.as_deref().map(Path::new);
    let models_config_path = args.models_config.as_deref().map(Path::new);

    let snapshot =
        runtime::run_models_snapshot(None, adapters_config_path, models_config_path).await?;
    let event = build_observer_event(&snapshot, args.host_ip.as_deref())?;
    let endpoint = resolve_endpoint(args.endpoint.as_deref());

    if args.dry_run {
        if let Some(ref out_path) = args.output {
            let json_str = serde_json::to_string_pretty(&event)
                .wrap_err("serializing observer event for output file")?;
            if let Some(parent) = Path::new(out_path).parent() {
                if !parent.as_os_str().is_empty() {
                    tokio::fs::create_dir_all(parent)
                        .await
                        .wrap_err_with(|| format!("creating directory for {}", out_path))?;
                }
            }
            tokio::fs::write(out_path, json_str)
                .await
                .wrap_err_with(|| format!("writing observer event to {}", out_path))?;
        }

        return Ok(ObserverEmitReport {
            status: "dry_run".to_string(),
            event_type: event.event_type,
            snapshot_id: event.payload.snapshot_id,
            endpoint,
            discovered_agents_count: event.payload.discovered_agents_count,
            active_models_count: event.payload.active_models_count,
            http_status: None,
            output_path: args.output,
            secrets_read: false,
        });
    }

    let token_path = resolve_token_file(args.token_file.as_deref());
    let token = load_host_token(&token_path)?;

    let url = format!("{endpoint}/api/events/ingest");
    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .header("X-Host-Token", token)
        .header("Content-Type", "application/json")
        .json(&event)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .wrap_err_with(|| format!("sending observer event to {url}"))?;

    let http_status = response.status();
    if !http_status.is_success() {
        let body = response.text().await.unwrap_or_default();
        let body_tail = crate::receipt::tail_sanitized(body.as_bytes(), 512);
        return Err(eyre!(
            "observer event ingestion failed with HTTP status {}: {}",
            http_status,
            body_tail
        ));
    }

    if let Some(ref out_path) = args.output {
        let json_str = serde_json::to_string_pretty(&event)
            .wrap_err("serializing observer event for output file")?;
        if let Some(parent) = Path::new(out_path).parent() {
            if !parent.as_os_str().is_empty() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .wrap_err_with(|| format!("creating directory for {}", out_path))?;
            }
        }
        tokio::fs::write(out_path, json_str)
            .await
            .wrap_err_with(|| format!("writing observer event to {}", out_path))?;
    }

    Ok(ObserverEmitReport {
        status: "sent".to_string(),
        event_type: event.event_type,
        snapshot_id: event.payload.snapshot_id,
        endpoint,
        discovered_agents_count: event.payload.discovered_agents_count,
        active_models_count: event.payload.active_models_count,
        http_status: Some(http_status.as_u16()),
        output_path: args.output,
        secrets_read: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::{
        RuntimeAgentSnapshot, RuntimeBinaryInfo, RuntimeConfigMetadata, RuntimeModel, RuntimePlan,
        RuntimeProvider,
    };
    use tempfile::NamedTempFile;
    use tokio::sync::Mutex;

    static TEST_ENV_MUTEX: Mutex<()> = Mutex::const_new(());

    fn sample_snapshot() -> ModelsSnapshot {
        ModelsSnapshot {
            schema_url: None,
            snapshot_id: "snap-20260904-213500-host01".to_string(),
            fetched_at: "2026-09-04T21:35:00Z".to_string(),
            user_id: "freddy".to_string(),
            source: "runtime_doctor".to_string(),
            agents: vec![
                RuntimeAgentSnapshot {
                    id: "qwen-code".to_string(),
                    name: "Qwen Code".to_string(),
                    vendor: "bailian".to_string(),
                    binary: RuntimeBinaryInfo {
                        command: "qwen".to_string(),
                        path: Some("/usr/local/bin/qwen".to_string()),
                        version: Some("1.4.2".to_string()),
                        detected: true,
                    },
                    config_metadata: RuntimeConfigMetadata {
                        settings_path: None,
                        has_credentials: true,
                        auth_type: Some("api_key".to_string()),
                        secrets_read: false,
                    },
                    providers: vec![RuntimeProvider {
                        id: "bailian".to_string(),
                        name: "Alibaba Cloud Bailian".to_string(),
                        endpoint: None,
                        plan: Some(RuntimePlan {
                            name: "Qwen Standard Plan".to_string(),
                            active: true,
                            period_start: None,
                            period_end: Some("2026-09-24T12:00:00Z".to_string()),
                            auto_renewal: None,
                        }),
                        models: vec![
                            RuntimeModel {
                                id: "qwen3.8-max".to_string(),
                                status: "active".to_string(),
                                source_type: "catalog".to_string(),
                                verified: true,
                                last_verified_at: None,
                                cost_hint: None,
                                capabilities: None,
                            },
                            RuntimeModel {
                                id: "qwen3.8-plus".to_string(),
                                status: "active".to_string(),
                                source_type: "catalog".to_string(),
                                verified: false,
                                last_verified_at: None,
                                cost_hint: None,
                                capabilities: None,
                            },
                            RuntimeModel {
                                id: "qwen-turbo-old".to_string(),
                                status: "deprecated".to_string(),
                                source_type: "catalog".to_string(),
                                verified: false,
                                last_verified_at: None,
                                cost_hint: None,
                                capabilities: None,
                            },
                        ],
                    }],
                    modes: vec![],
                    tools: vec![],
                    probed_at: "2026-09-04T21:35:00Z".to_string(),
                    source: "runtime_doctor".to_string(),
                    secrets_read: false,
                },
                RuntimeAgentSnapshot {
                    id: "claude-code".to_string(),
                    name: "Claude Code".to_string(),
                    vendor: "anthropic".to_string(),
                    binary: RuntimeBinaryInfo {
                        command: "claude".to_string(),
                        path: Some("/usr/local/bin/claude".to_string()),
                        version: Some("1.0.18".to_string()),
                        detected: true,
                    },
                    config_metadata: RuntimeConfigMetadata {
                        settings_path: None,
                        has_credentials: true,
                        auth_type: Some("api_key".to_string()),
                        secrets_read: false,
                    },
                    providers: vec![RuntimeProvider {
                        id: "anthropic".to_string(),
                        name: "Anthropic API".to_string(),
                        endpoint: None,
                        plan: None,
                        models: vec![RuntimeModel {
                            id: "claude-3-7-sonnet".to_string(),
                            status: "active".to_string(),
                            source_type: "catalog".to_string(),
                            verified: true,
                            last_verified_at: None,
                            cost_hint: None,
                            capabilities: None,
                        }],
                    }],
                    modes: vec![],
                    tools: vec![],
                    probed_at: "2026-09-04T21:35:00Z".to_string(),
                    source: "runtime_doctor".to_string(),
                    secrets_read: false,
                },
            ],
            secrets_read: false,
        }
    }

    #[test]
    fn build_observer_event_maps_counts_and_summary() {
        let snapshot = sample_snapshot();
        let event = build_observer_event(&snapshot, Some("192.168.1.50")).unwrap();

        assert_eq!(event.event_type, "agent.discovery.snapshot");
        assert_eq!(event.host_ip, "192.168.1.50");
        assert_eq!(event.payload.snapshot_id, "snap-20260904-213500-host01");
        assert_eq!(event.payload.discovered_agents_count, 2);
        // qwen has 2 active models (1 deprecated), claude has 1 active model -> total 3 active models
        assert_eq!(event.payload.active_models_count, 3);
        assert_eq!(event.payload.agents_summary.len(), 2);

        let qwen_sum = &event.payload.agents_summary[0];
        assert_eq!(qwen_sum.agent_id, "qwen-code");
        assert_eq!(qwen_sum.version.as_deref(), Some("1.4.2"));
        assert_eq!(qwen_sum.provider.as_deref(), Some("bailian"));
        assert_eq!(qwen_sum.active_model.as_deref(), Some("qwen3.8-max"));
        assert_eq!(qwen_sum.plan_status.as_deref(), Some("Qwen Standard Plan"));
        assert_eq!(
            qwen_sum.plan_expires_at.as_deref(),
            Some("2026-09-24T12:00:00Z")
        );

        let claude_sum = &event.payload.agents_summary[1];
        assert_eq!(claude_sum.agent_id, "claude-code");
        assert_eq!(claude_sum.version.as_deref(), Some("1.0.18"));
        assert_eq!(claude_sum.provider.as_deref(), Some("anthropic"));
        assert_eq!(
            claude_sum.active_model.as_deref(),
            Some("claude-3-7-sonnet")
        );
        assert_eq!(claude_sum.plan_status, None);
        assert_eq!(claude_sum.plan_expires_at, None);

        assert!(event.payload.verification_signature.starts_with("sha256:"));
        assert_eq!(event.payload.verification_signature.len(), 7 + 64);
    }

    #[test]
    fn verification_signature_is_sha256_and_stable() {
        let snapshot = sample_snapshot();
        let event1 = build_observer_event(&snapshot, None).unwrap();
        let event2 = build_observer_event(&snapshot, None).unwrap();

        assert_eq!(
            event1.payload.verification_signature,
            event2.payload.verification_signature
        );
        let sig = &event1.payload.verification_signature;
        assert!(sig.starts_with("sha256:"));
        let hex_part = &sig[7..];
        assert_eq!(hex_part.len(), 64);
        assert!(hex_part.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn load_host_token_trims_and_handles_errors_safely() {
        let mut temp = NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut temp, b"  secret-host-token-xyz \n\r\n").unwrap();
        let token = load_host_token(temp.path()).unwrap();
        assert_eq!(token, "secret-host-token-xyz");

        // Empty file error
        let mut empty_temp = NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut empty_temp, b"   \n\t ").unwrap();
        let empty_err = load_host_token(empty_temp.path()).unwrap_err();
        let empty_err_msg = format!("{empty_err}");
        assert!(empty_err_msg.contains("host token file is empty"));
        assert!(!empty_err_msg.contains("secret"));

        // Non existent file error
        let missing_path = Path::new("/tmp/definitely-non-existent-host-token-file-xyz123");
        let err = load_host_token(missing_path).unwrap_err();
        let err_msg = format!("{err}");
        assert!(err_msg.contains("host token file not found"));
        assert!(!err_msg.contains("secret"));
    }

    #[tokio::test]
    async fn resolve_token_file_priority() {
        let _guard = TEST_ENV_MUTEX.lock().await;

        // 1. Explicit arg
        let res_arg = resolve_token_file(Some("/custom/token/file"));
        assert_eq!(res_arg, PathBuf::from("/custom/token/file"));

        // 2. Env var
        std::env::set_var("ORQ_OBSERVER_TOKEN_FILE", "/env/token/path");
        let res_env = resolve_token_file(None);
        assert_eq!(res_env, PathBuf::from("/env/token/path"));
        std::env::remove_var("ORQ_OBSERVER_TOKEN_FILE");

        // 3. Default fallback to ~/.config/sge-observer/agent-orchestrator.host-token
        let res_default = resolve_token_file(None);
        assert!(res_default.ends_with(".config/sge-observer/agent-orchestrator.host-token"));
    }

    #[tokio::test]
    async fn resolve_endpoint_priority() {
        let _guard = TEST_ENV_MUTEX.lock().await;

        // 1. Explicit arg
        let res_arg = resolve_endpoint(Some("https://custom.endpoint.local/"));
        assert_eq!(res_arg, "https://custom.endpoint.local");

        // 2. Env var
        std::env::set_var("ORQ_OBSERVER_ENDPOINT", "https://env.endpoint.local/");
        let res_env = resolve_endpoint(None);
        assert_eq!(res_env, "https://env.endpoint.local");
        std::env::remove_var("ORQ_OBSERVER_ENDPOINT");

        // 3. Default fallback
        let res_default = resolve_endpoint(None);
        assert_eq!(res_default, "https://sge-panel.humanbyte.net");
    }
}
