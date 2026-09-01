use crate::adapters::{AdapterStatus, AgentDetection};
use crate::detect;
use crate::policy;
use color_eyre::eyre::{eyre, Result, WrapErr};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

const DEFAULT_ROUTING_CONFIG: &str = include_str!("../config/routing-matrix.json");
const SUPPORTED_SCHEMA_VERSION: u8 = 1;

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
    pub rationale: String,
}

pub fn load_default_config() -> Result<RoutingConfig> {
    parse_config(DEFAULT_ROUTING_CONFIG)
}

pub async fn load_config(path: Option<&Path>) -> Result<(RoutingConfig, String)> {
    match path {
        Some(path) => {
            let content = tokio::fs::read_to_string(path)
                .await
                .wrap_err_with(|| format!("reading routing config {}", path.display()))?;
            Ok((parse_config(&content)?, path.display().to_string()))
        }
        None => Ok((
            load_default_config()?,
            "embedded:orq-agent/config/routing-matrix.json".to_string(),
        )),
    }
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

pub fn decide(
    config: &RoutingConfig,
    task_kind: &str,
    allow_gated: bool,
    config_source: &str,
) -> Result<RouteDecision> {
    let rule = config
        .routes
        .iter()
        .find(|route| route.task_kind == task_kind)
        .ok_or_else(|| eyre!("task_kind {task_kind} is not present in routing config"))?;
    let detected = detect::detect_agents();
    let selected = select_route(
        rule,
        &config.approval_required_model_patterns,
        &detected.agents,
        allow_gated,
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
        rationale: rule.rationale.clone(),
    })
}

struct SelectedRoute {
    agent: String,
    model: String,
    fallback_applied: bool,
    requires_confirmation: bool,
    policy_reason: String,
}

fn select_route(
    rule: &RouteRule,
    approval_patterns: &[String],
    detected: &[AgentDetection],
    allow_gated: bool,
) -> SelectedRoute {
    let candidates = [
        candidate_from_parts(&rule.default_agent, &rule.default_model, false),
        candidate_from_expr(&rule.cheap_sufficient, true),
        candidate_from_expr(&rule.escalate_to, true),
    ];

    for candidate in candidates.into_iter().flatten() {
        if let Some(status) = detected_status(detected, &candidate.agent) {
            let policy = policy::evaluate(&candidate.agent, &candidate.model, status, allow_gated);
            if policy.allowed {
                let requires_confirmation =
                    requires_confirmation(status, &candidate.model, approval_patterns);
                return SelectedRoute {
                    agent: candidate.agent,
                    model: candidate.model,
                    fallback_applied: candidate.fallback_applied,
                    requires_confirmation,
                    policy_reason: policy.reason,
                };
            }
        }
    }

    SelectedRoute {
        agent: rule.default_agent.clone(),
        model: rule.default_model.clone(),
        fallback_applied: false,
        requires_confirmation: true,
        policy_reason: "no detected allowed route; returning default for explicit human review"
            .to_string(),
    }
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
    let model = model.to_ascii_lowercase();
    approval_patterns
        .iter()
        .map(|pattern| pattern.to_ascii_lowercase())
        .any(|pattern| model.contains(&pattern))
}

#[cfg(test)]
mod tests {
    use super::{decide, load_default_config, parse_config};

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
    fn architecture_requires_confirmation_from_config() {
        let config = load_default_config().unwrap();
        let decision = decide(&config, "architecture", false, "test").unwrap();
        assert!(decision.requires_confirmation);
        assert_eq!(decision.secrets_read, false);
    }

    #[test]
    fn missing_task_kind_is_an_error() {
        let config = load_default_config().unwrap();
        let err = decide(&config, "unknown_kind", false, "test").unwrap_err();
        assert!(err.to_string().contains("not present in routing config"));
    }
}
