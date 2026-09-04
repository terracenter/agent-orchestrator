use crate::adapters::{AdapterStatus, AgentDetection};
use crate::certstore::{is_certified, is_failed, CertificateStore};
use crate::detect;
use crate::policy;
use crate::state::StateStore;
use color_eyre::eyre::{eyre, Result, WrapErr};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

const SUPPORTED_SCHEMA_VERSION: u8 = 1;
const DEFAULT_ROUTING_CONFIG_PATH: &str = "config/routing-matrix.json";
const ROUTING_CONFIG_ENV: &str = "ORQ_ROUTING_CONFIG";

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RoutingConfig {
    pub schema_version: u8,
    pub approval_required_model_patterns: Vec<String>,
    pub routes: Vec<RouteRule>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RouteRule {
    pub task_kind: String,
    pub default_agent: String,
    pub default_model: String,
    pub cheap_sufficient: String,
    pub escalate_to: String,
    pub avoid: Vec<String>,
    pub rationale: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct RouteDecision {
    pub schema_version: u8,
    pub task_kind: String,
    pub selected_agent: String,
    pub selected_model: String,
    pub selected_policy_reason: String,
    pub default_agent: String,
    pub default_model: String,
    pub cheap_sufficient: String,
    pub escalate_to: String,
    pub avoid: Vec<String>,
    pub fallback_applied: bool,
    pub requires_confirmation: bool,
    pub secrets_read: bool,
    pub config_source: String,
    pub certificate_store_used: bool,
    pub certificate_store_ignored_files: usize,
    pub preferred_certificate: Option<String>,
    pub circuit_breaker_used: bool,
    pub circuit_breaker_filtered: usize,
    pub rationale: String,
}

#[allow(dead_code)]
pub fn load_default_config() -> Result<RoutingConfig> {
    let path = default_config_path(ROUTING_CONFIG_ENV, DEFAULT_ROUTING_CONFIG_PATH);
    let content = std::fs::read_to_string(&path)
        .wrap_err_with(|| format!("reading routing config {}", path.display()))?;
    parse_config(&content)
}

pub async fn load_config(path: Option<&Path>) -> Result<(RoutingConfig, String)> {
    let path_buf;
    let path = match path {
        Some(path) => path,
        None => {
            path_buf = default_config_path(ROUTING_CONFIG_ENV, DEFAULT_ROUTING_CONFIG_PATH);
            path_buf.as_path()
        }
    };
    let content = tokio::fs::read_to_string(path)
        .await
        .wrap_err_with(|| format!("reading routing config {}", path.display()))?;
    Ok((parse_config(&content)?, path.display().to_string()))
}

fn default_config_path(env_name: &str, relative_path: &str) -> std::path::PathBuf {
    std::env::var_os(env_name)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative_path))
}

pub fn parse_config(content: &str) -> Result<RoutingConfig> {
    let config: RoutingConfig =
        serde_json::from_str(content).wrap_err("parsing routing config json")?;
    validate_config(&config)?;
    Ok(config)
}

fn validate_config(config: &RoutingConfig) -> Result<()> {
    if config.schema_version != SUPPORTED_SCHEMA_VERSION {
        return Err(eyre!(
            "unsupported routing schema_version {}; expected {}",
            config.schema_version,
            SUPPORTED_SCHEMA_VERSION
        ));
    }
    if config.approval_required_model_patterns.is_empty() {
        return Err(eyre!(
            "routing config must define approval_required_model_patterns"
        ));
    }
    if config.routes.is_empty() {
        return Err(eyre!("routing config must define at least one route"));
    }

    let mut seen = HashSet::new();
    for route in &config.routes {
        validate_non_empty("task_kind", &route.task_kind)?;
        validate_non_empty("default_agent", &route.default_agent)?;
        validate_non_empty("default_model", &route.default_model)?;
        validate_non_empty("rationale", &route.rationale)?;
        if !seen.insert(route.task_kind.as_str()) {
            return Err(eyre!("duplicate route for task_kind {}", route.task_kind));
        }
        validate_route_expr("cheap_sufficient", &route.cheap_sufficient)?;
        validate_route_expr("escalate_to", &route.escalate_to)?;
    }
    Ok(())
}

fn validate_non_empty(field: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        return Err(eyre!("routing field {field} cannot be empty"));
    }
    Ok(())
}

fn validate_route_expr(field: &str, value: &str) -> Result<()> {
    if value == "none" || value.contains('/') {
        Ok(())
    } else {
        Err(eyre!(
            "routing field {field} must be 'agent/model' or 'none'"
        ))
    }
}

#[allow(dead_code)]
pub fn decide(
    config: &RoutingConfig,
    task_kind: &str,
    allow_gated: bool,
    config_source: &str,
) -> Result<RouteDecision> {
    let detected = detect::detect_agents()?;
    decide_with_detected(
        config,
        task_kind,
        allow_gated,
        config_source,
        &detected,
        None,
        None,
    )
}

pub fn decide_with_detected(
    config: &RoutingConfig,
    task_kind: &str,
    allow_gated: bool,
    config_source: &str,
    detected: &detect::DetectReport,
    certificate_store: Option<&CertificateStore>,
    state_store: Option<&StateStore>,
) -> Result<RouteDecision> {
    let rule = config
        .routes
        .iter()
        .find(|route| route.task_kind == task_kind)
        .ok_or_else(|| eyre!("task_kind {task_kind} is not present in routing config"))?;
    let selected = select_route(
        rule,
        &config.approval_required_model_patterns,
        &detected.agents,
        allow_gated,
        certificate_store,
        state_store,
    );

    Ok(RouteDecision {
        schema_version: config.schema_version,
        task_kind: rule.task_kind.clone(),
        selected_agent: selected.agent,
        selected_model: selected.model,
        selected_policy_reason: selected.policy_reason,
        default_agent: rule.default_agent.clone(),
        default_model: rule.default_model.clone(),
        cheap_sufficient: rule.cheap_sufficient.clone(),
        escalate_to: rule.escalate_to.clone(),
        avoid: rule.avoid.clone(),
        fallback_applied: selected.fallback_applied,
        requires_confirmation: selected.requires_confirmation
            || route_requires_confirmation(rule, &config.approval_required_model_patterns),
        secrets_read: detected.secrets_read,
        config_source: config_source.to_string(),
        certificate_store_used: certificate_store.is_some(),
        certificate_store_ignored_files: certificate_store
            .map(CertificateStore::ignored_files)
            .unwrap_or(0),
        preferred_certificate: selected.preferred_certificate,
        circuit_breaker_used: state_store.is_some(),
        circuit_breaker_filtered: selected.circuit_breaker_filtered,
        rationale: rule.rationale.clone(),
    })
}

struct SelectedRoute {
    agent: String,
    model: String,
    fallback_applied: bool,
    requires_confirmation: bool,
    policy_reason: String,
    preferred_certificate: Option<String>,
    circuit_breaker_filtered: usize,
}

fn select_route(
    rule: &RouteRule,
    approval_patterns: &[String],
    detected: &[AgentDetection],
    allow_gated: bool,
    certificate_store: Option<&CertificateStore>,
    state_store: Option<&StateStore>,
) -> SelectedRoute {
    let candidates = [
        candidate_from_parts(&rule.default_agent, &rule.default_model, false),
        candidate_from_expr(&rule.cheap_sufficient, true),
        candidate_from_expr(&rule.escalate_to, true),
    ];

    let mut circuit_breaker_filtered = 0usize;
    for candidate in candidates.into_iter().flatten() {
        if let Some(store) = state_store {
            match store.breaker_allows_model(&candidate.agent, &candidate.model) {
                Ok(true) => {}
                Ok(false) => {
                    circuit_breaker_filtered += 1;
                    continue;
                }
                Err(_) => {
                    circuit_breaker_filtered += 1;
                    continue;
                }
            }
        }
        if let Some(certificate) = certificate_store
            .and_then(|store| store.lookup(&candidate.agent, &candidate.model, &rule.task_kind))
        {
            if is_failed(certificate) {
                continue;
            }
            if is_certified(certificate) {
                if let Some(selected) = select_allowed_candidate(
                    candidate,
                    approval_patterns,
                    detected,
                    allow_gated,
                    Some(certificate.certificate_id.clone()),
                ) {
                    return SelectedRoute {
                        circuit_breaker_filtered,
                        ..selected
                    };
                }
                continue;
            }
        }

        if let Some(selected) =
            select_allowed_candidate(candidate, approval_patterns, detected, allow_gated, None)
        {
            return SelectedRoute {
                circuit_breaker_filtered,
                ..selected
            };
        }
    }

    SelectedRoute {
        agent: rule.default_agent.clone(),
        model: rule.default_model.clone(),
        fallback_applied: false,
        requires_confirmation: true,
        policy_reason: "no detected allowed route; returning default for explicit human review"
            .to_string(),
        preferred_certificate: None,
        circuit_breaker_filtered,
    }
}

fn select_allowed_candidate(
    candidate: Candidate,
    approval_patterns: &[String],
    detected: &[AgentDetection],
    allow_gated: bool,
    preferred_certificate: Option<String>,
) -> Option<SelectedRoute> {
    let status = detected_status(detected, &candidate.agent)?;
    let policy_config = policy::PolicyConfig {
        schema_version: 1,
        approval_required_model_patterns: approval_patterns.to_vec(),
        blocked_adapter_statuses: vec!["deprecated_or_quarantine".to_string()],
        gated_adapter_statuses: vec!["gated".to_string()],
    };
    let policy = policy::evaluate(
        &candidate.agent,
        &candidate.model,
        status,
        allow_gated,
        &policy_config,
    );
    if !policy.allowed {
        return None;
    }
    let requires_confirmation = requires_confirmation(status, &candidate.model, approval_patterns);
    let policy_reason = match &preferred_certificate {
        Some(certificate_id) => format!("certified:{certificate_id}; {}", policy.reason),
        None => policy.reason,
    };
    Some(SelectedRoute {
        agent: candidate.agent,
        model: candidate.model,
        fallback_applied: candidate.fallback_applied,
        requires_confirmation,
        policy_reason,
        preferred_certificate,
        circuit_breaker_filtered: 0,
    })
}

struct Candidate {
    agent: String,
    model: String,
    fallback_applied: bool,
}

fn candidate_from_parts(agent: &str, model: &str, fallback_applied: bool) -> Option<Candidate> {
    if agent == "none" {
        return None;
    }
    Some(Candidate {
        agent: agent.to_string(),
        model: model.to_string(),
        fallback_applied,
    })
}

fn candidate_from_expr(expr: &str, fallback_applied: bool) -> Option<Candidate> {
    if expr == "none" {
        return None;
    }
    let (agent, model) = expr.split_once('/')?;
    candidate_from_parts(agent, model, fallback_applied)
}

fn detected_status(detected: &[AgentDetection], agent: &str) -> Option<AdapterStatus> {
    detected
        .iter()
        .find(|item| item.name == agent && item.detected)
        .map(|item| item.adapter)
}

fn requires_confirmation(status: AdapterStatus, model: &str, approval_patterns: &[String]) -> bool {
    matches!(status, AdapterStatus::Gated) || model_needs_approval(model, approval_patterns)
}

fn route_requires_confirmation(rule: &RouteRule, approval_patterns: &[String]) -> bool {
    model_needs_approval(&rule.default_model, approval_patterns)
}

fn model_needs_approval(model: &str, approval_patterns: &[String]) -> bool {
    policy::model_needs_approval(model, approval_patterns)
}

#[cfg(test)]
mod tests {
    use super::{decide, decide_with_detected, load_default_config, parse_config};
    use crate::adapters::{AdapterStatus, AgentDetection};
    use crate::detect::DetectReport;
    use crate::state::BreakerOutcome;

    #[test]
    fn default_config_loads_documentation_route() {
        let config = load_default_config().unwrap();
        let route = config
            .routes
            .iter()
            .find(|route| route.task_kind == "documentation")
            .unwrap();
        assert_eq!(route.default_agent, "qwen-code");
        assert_eq!(route.default_model, "qwen3.6-flash");
    }

    #[test]
    fn invalid_schema_is_rejected() {
        let err = parse_config(
            r#"{"schema_version":2,"approval_required_model_patterns":["opus"],"routes":[]}"#,
        )
        .unwrap_err();
        assert!(err
            .to_string()
            .contains("unsupported routing schema_version"));
    }

    #[test]
    fn route_expression_must_be_agent_model_or_none() {
        let err = parse_config(
            r#"{"schema_version":1,"approval_required_model_patterns":["opus"],"routes":[{"task_kind":"x","default_agent":"a","default_model":"m","cheap_sufficient":"bad","escalate_to":"none","avoid":[],"rationale":"r"}]}"#,
        )
        .unwrap_err();
        assert!(err.to_string().contains("agent/model"));
    }

    #[test]
    fn configured_architecture_default_does_not_require_gated_approval() {
        let config = load_default_config().unwrap();
        let route = config
            .routes
            .iter()
            .find(|route| route.task_kind == "architecture")
            .unwrap();
        let detected = DetectReport {
            schema_version: 1,
            agents: vec![AgentDetection {
                name: route.default_agent.clone(),
                binary: "test-runner".to_string(),
                detected: true,
                binary_path: Some("test-runner".to_string()),
                adapter: AdapterStatus::Available,
                secrets_read: false,
            }],
            secrets_read: false,
        };
        let decision = decide_with_detected(
            &config,
            &route.task_kind,
            false,
            "test",
            &detected,
            None,
            None,
        )
        .unwrap();
        assert_eq!(decision.selected_agent, route.default_agent);
        assert_eq!(decision.selected_model, route.default_model);
        assert!(!decision.requires_confirmation);
        assert!(!decision.secrets_read);
    }

    #[test]
    fn missing_task_kind_is_an_error() {
        let config = load_default_config().unwrap();
        let err = decide(&config, "unknown_kind", false, "test").unwrap_err();
        assert!(err.to_string().contains("not present in routing config"));
    }

    #[test]
    fn test_circuit_breaker_timeout_skips_default_model_and_uses_fallback() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::state::open(Some(&dir.path().join("state.sqlite"))).unwrap();

        let config = parse_config(
            r#"{
                "schema_version": 1,
                "approval_required_model_patterns": ["opus"],
                "routes": [{
                    "task_kind": "code",
                    "default_agent": "primary-agent",
                    "default_model": "primary-model",
                    "cheap_sufficient": "fallback-agent/fallback-model",
                    "escalate_to": "none",
                    "avoid": [],
                    "rationale": "testing fallback under breaker cooldown"
                }]
            }"#,
        )
        .unwrap();

        let detected = DetectReport {
            schema_version: 1,
            agents: vec![
                AgentDetection {
                    name: "primary-agent".to_string(),
                    binary: "primary-runner".to_string(),
                    detected: true,
                    binary_path: Some("primary-runner".to_string()),
                    adapter: AdapterStatus::Available,
                    secrets_read: false,
                },
                AgentDetection {
                    name: "fallback-agent".to_string(),
                    binary: "fallback-runner".to_string(),
                    detected: true,
                    binary_path: Some("fallback-runner".to_string()),
                    adapter: AdapterStatus::Available,
                    secrets_read: false,
                },
            ],
            secrets_read: false,
        };

        store
            .record_breaker_outcome("primary-agent", "primary-model", BreakerOutcome::TimedOut)
            .unwrap();

        let decision = decide_with_detected(
            &config,
            "code",
            false,
            "test",
            &detected,
            None,
            Some(&store),
        )
        .unwrap();

        assert_eq!(decision.selected_agent, "fallback-agent");
        assert_eq!(decision.selected_model, "fallback-model");
        assert!(decision.fallback_applied);
        assert!(decision.circuit_breaker_used);
        assert_eq!(decision.circuit_breaker_filtered, 1);
    }

    #[test]
    fn test_circuit_breaker_allow_gated_true_still_skips_gated_model_in_cooldown() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::state::open(Some(&dir.path().join("state.sqlite"))).unwrap();

        let config = parse_config(
            r#"{
                "schema_version": 1,
                "approval_required_model_patterns": ["gated-model"],
                "routes": [{
                    "task_kind": "security",
                    "default_agent": "gated-agent",
                    "default_model": "gated-model",
                    "cheap_sufficient": "healthy-agent/healthy-model",
                    "escalate_to": "none",
                    "avoid": [],
                    "rationale": "testing gated agent with breaker cooldown"
                }]
            }"#,
        )
        .unwrap();

        let detected = DetectReport {
            schema_version: 1,
            agents: vec![
                AgentDetection {
                    name: "gated-agent".to_string(),
                    binary: "gated-runner".to_string(),
                    detected: true,
                    binary_path: Some("gated-runner".to_string()),
                    adapter: AdapterStatus::Gated,
                    secrets_read: false,
                },
                AgentDetection {
                    name: "healthy-agent".to_string(),
                    binary: "healthy-runner".to_string(),
                    detected: true,
                    binary_path: Some("healthy-runner".to_string()),
                    adapter: AdapterStatus::Available,
                    secrets_read: false,
                },
            ],
            secrets_read: false,
        };

        store
            .record_breaker_outcome("gated-agent", "gated-model", BreakerOutcome::TimedOut)
            .unwrap();

        let decision = decide_with_detected(
            &config,
            "security",
            true,
            "test",
            &detected,
            None,
            Some(&store),
        )
        .unwrap();

        assert_eq!(decision.selected_agent, "healthy-agent");
        assert_eq!(decision.selected_model, "healthy-model");
        assert!(decision.fallback_applied);
        assert_eq!(decision.circuit_breaker_filtered, 1);
    }

    #[test]
    fn test_circuit_breaker_cooldown_expired_allows_default_model_again() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::state::open(Some(&dir.path().join("state.sqlite"))).unwrap();

        let config = parse_config(
            r#"{
                "schema_version": 1,
                "approval_required_model_patterns": ["opus"],
                "routes": [{
                    "task_kind": "code",
                    "default_agent": "primary-agent",
                    "default_model": "primary-model",
                    "cheap_sufficient": "fallback-agent/fallback-model",
                    "escalate_to": "none",
                    "avoid": [],
                    "rationale": "testing expired cooldown"
                }]
            }"#,
        )
        .unwrap();

        let detected = DetectReport {
            schema_version: 1,
            agents: vec![AgentDetection {
                name: "primary-agent".to_string(),
                binary: "primary-runner".to_string(),
                detected: true,
                binary_path: Some("primary-runner".to_string()),
                adapter: AdapterStatus::Available,
                secrets_read: false,
            }],
            secrets_read: false,
        };

        store
            .force_open_breaker_until("primary-agent", "primary-model", 100)
            .unwrap();

        let decision = decide_with_detected(
            &config,
            "code",
            false,
            "test",
            &detected,
            None,
            Some(&store),
        )
        .unwrap();

        assert_eq!(decision.selected_agent, "primary-agent");
        assert_eq!(decision.selected_model, "primary-model");
        assert!(!decision.fallback_applied);
        assert_eq!(decision.circuit_breaker_filtered, 0);
    }
}
