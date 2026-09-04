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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quota_aware: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub quota_penalized_candidates: Vec<String>,
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
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn decide_with_detected(
    config: &RoutingConfig,
    task_kind: &str,
    allow_gated: bool,
    config_source: &str,
    detected: &detect::DetectReport,
    certificate_store: Option<&CertificateStore>,
    state_store: Option<&StateStore>,
    models_catalog: Option<&crate::models::ModelsCatalog>,
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
        models_catalog,
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
        quota_aware: if selected.quota_aware {
            Some(true)
        } else {
            None
        },
        quota_penalized_candidates: selected.quota_penalized_candidates,
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
    quota_aware: bool,
    quota_penalized_candidates: Vec<String>,
}

/// Default time-to-live for quota snapshots (24 hours).
/// Snapshots older than this TTL are considered stale and ignored during routing decisions.
pub const DEFAULT_SNAPSHOT_TTL_SECS: u64 = 86_400;

/// Threshold below which remaining immediate quota is considered critical/warning (15.0%).
/// Candidates at or below this percentage receive a soft penalty so healthy candidates are preferred.
pub const CRITICAL_QUOTA_THRESHOLD_PCT: f64 = 15.0;

/// Threshold for weekly remaining quota required to boost/allow a gated model when non-gated models are exhausted (50.0%).
pub const GATED_WEEKLY_THRESHOLD_PCT: f64 = 50.0;

#[derive(Debug, Clone, Default)]
struct CandidateQuota {
    has_quota_data: bool,
    is_exhausted: bool,
    is_warning: bool,
    short_term_remaining_pct: Option<f64>,
    weekly_remaining_pct: Option<f64>,
}

fn provider_matches(snapshot_provider: &str, candidate_agent: &str) -> bool {
    let p = snapshot_provider.trim().to_lowercase();
    let a = candidate_agent.trim().to_lowercase();
    if p == a {
        return true;
    }
    if let Some(stripped_a) = a.strip_suffix("-code") {
        if p == stripped_a {
            return true;
        }
    }
    if let Some(stripped_p) = p.strip_suffix("-code") {
        if a == stripped_p {
            return true;
        }
    }
    false
}

fn is_short_term_scope(scope_lower: &str) -> bool {
    scope_lower.contains("five_hour")
        || scope_lower.contains("five-hour")
        || scope_lower.contains("5h")
        || scope_lower.contains("short-term")
        || scope_lower.contains("short_term")
        || scope_lower.contains("hourly")
        || scope_lower.contains("hour")
        || scope_lower.contains("session")
}

fn is_weekly_scope(scope_lower: &str) -> bool {
    scope_lower.contains("weekly")
        || scope_lower.contains("week")
        || scope_lower.contains("month")
        || scope_lower.contains("long-term")
        || scope_lower.contains("long_term")
}

fn assess_candidate_quota(
    agent: &str,
    snapshots: &[crate::state::QuotaSnapshotRecord],
) -> CandidateQuota {
    let now = crate::quota::now_unix();
    let matching: Vec<&crate::state::QuotaSnapshotRecord> = snapshots
        .iter()
        .filter(|s| {
            provider_matches(&s.provider, agent)
                && now.saturating_sub(s.captured_at_unix) <= DEFAULT_SNAPSHOT_TTL_SECS
        })
        .collect();

    if matching.is_empty() {
        return CandidateQuota::default();
    }

    let any_known = matching
        .iter()
        .any(|s| s.status != "quota_unknown" || s.remaining_pct.is_some() || s.used_pct.is_some());
    if !any_known {
        return CandidateQuota::default();
    }

    let mut is_exhausted = false;
    let mut is_warning = false;
    let mut short_term_remaining: Option<f64> = None;
    let mut weekly_remaining: Option<f64> = None;

    for s in &matching {
        // If the reset timestamp is defined and already elapsed (reset_at_unix <= now),
        // the quota limit has refreshed, so this scope is treated as recovered.
        let is_reset_passed = s.reset_at_unix.is_some_and(|reset| reset <= now);
        if is_reset_passed {
            continue;
        }

        let status_lower = s.status.to_lowercase();
        let rem = s
            .remaining_pct
            .or_else(|| s.used_pct.map(|u| (100.0 - u).max(0.0)));

        if status_lower == "exhausted" || status_lower == "exceeded" || rem == Some(0.0) {
            is_exhausted = true;
        } else if status_lower == "warning"
            || rem.is_some_and(|r| r <= CRITICAL_QUOTA_THRESHOLD_PCT)
        {
            is_warning = true;
        }

        if let Some(r) = rem {
            let scope_lower = s.scope.to_lowercase();
            if is_short_term_scope(&scope_lower) {
                short_term_remaining = Some(short_term_remaining.map_or(r, |curr| curr.min(r)));
                if r <= CRITICAL_QUOTA_THRESHOLD_PCT {
                    is_warning = true;
                }
            } else if is_weekly_scope(&scope_lower) {
                weekly_remaining = Some(weekly_remaining.map_or(r, |curr| curr.min(r)));
            }
        }
    }

    CandidateQuota {
        has_quota_data: true,
        is_exhausted,
        is_warning,
        short_term_remaining_pct: short_term_remaining,
        weekly_remaining_pct: weekly_remaining,
    }
}

struct EvaluatedCandidate {
    original_index: usize,
    selected: SelectedRoute,
    is_gated: bool,
    quota: CandidateQuota,
    is_down_or_deprecated: bool,
    cost_hint: Option<f64>,
    has_promo: bool,
}

impl EvaluatedCandidate {
    fn tier(&self, any_healthy: bool) -> i32 {
        let is_short_term_critical = self
            .quota
            .short_term_remaining_pct
            .is_some_and(|p| p <= CRITICAL_QUOTA_THRESHOLD_PCT);

        let is_gated_under_weekly_threshold = self.is_gated
            && self.quota.has_quota_data
            && self
                .quota
                .weekly_remaining_pct
                .is_none_or(|w| w < GATED_WEEKLY_THRESHOLD_PCT);

        // Penalty tiers:
        // -100: Exhausted or model status down/deprecated (when at least one candidate is healthy)
        // -50: Degraded / Warning (status == warning, immediate quota <= 15%, or gated under weekly threshold)
        // 0: Healthy / Baseline
        // Note: Certificates annotate but NEVER reorder candidates (no tier boost).
        // Note: Gated models NEVER displace healthy defaults (no tier boost above 0).
        if any_healthy && (self.quota.is_exhausted || self.is_down_or_deprecated) {
            -100
        } else if any_healthy
            && (self.quota.is_warning || is_short_term_critical || is_gated_under_weekly_threshold)
        {
            -50
        } else {
            0
        }
    }
}

fn select_route(
    rule: &RouteRule,
    approval_patterns: &[String],
    detected: &[AgentDetection],
    allow_gated: bool,
    certificate_store: Option<&CertificateStore>,
    state_store: Option<&StateStore>,
    models_catalog: Option<&crate::models::ModelsCatalog>,
) -> SelectedRoute {
    let raw_candidates = [
        candidate_from_parts(&rule.default_agent, &rule.default_model, false),
        candidate_from_expr(&rule.cheap_sufficient, true),
        candidate_from_expr(&rule.escalate_to, true),
    ];

    let mut circuit_breaker_filtered = 0usize;
    let mut allowed_candidates: Vec<EvaluatedCandidate> = Vec::new();

    let quota_snapshots = state_store
        .and_then(|store| store.latest_quota_snapshots(None).ok())
        .unwrap_or_default();

    for (index, candidate) in raw_candidates.into_iter().flatten().enumerate() {
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

        let mut preferred_cert = None;
        if let Some(certificate) = certificate_store
            .and_then(|store| store.lookup(&candidate.agent, &candidate.model, &rule.task_kind))
        {
            if is_failed(certificate) {
                continue;
            }
            if is_certified(certificate) {
                preferred_cert = Some(certificate.certificate_id.clone());
            }
        }

        let status = match detected_status(detected, &candidate.agent) {
            Some(status) => status,
            None => continue,
        };

        let policy_config = policy::PolicyConfig {
            schema_version: 1,
            approval_required_model_patterns: approval_patterns.to_vec(),
            blocked_adapter_statuses: vec!["deprecated_or_quarantine".to_string()],
            gated_adapter_statuses: vec!["gated".to_string()],
        };
        let policy_eval = policy::evaluate(
            &candidate.agent,
            &candidate.model,
            status,
            allow_gated,
            &policy_config,
        );
        if !policy_eval.allowed {
            continue;
        }

        let (cost_hint, promo, model_status) = if let Some(catalog) = models_catalog {
            if let Some(agent_models) = catalog.agents.get(&candidate.agent) {
                if let Some(m) = agent_models.iter().find(|m| m.id == candidate.model) {
                    (m.cost_hint, m.promo.clone(), m.status.clone())
                } else {
                    (None, None, None)
                }
            } else {
                (None, None, None)
            }
        } else {
            (None, None, None)
        };

        let is_model_down_or_deprecated = matches!(
            model_status.as_deref().map(str::to_lowercase).as_deref(),
            Some("deprecated") | Some("down") | Some("disabled") | Some("offline")
        );

        let is_gated = matches!(status, AdapterStatus::Gated);
        let requires_conf = requires_confirmation(status, &candidate.model, approval_patterns);
        let policy_reason = match &preferred_cert {
            Some(certificate_id) => format!("certified:{certificate_id}; {}", policy_eval.reason),
            None => policy_eval.reason,
        };

        let quota = assess_candidate_quota(&candidate.agent, &quota_snapshots);

        allowed_candidates.push(EvaluatedCandidate {
            original_index: index,
            selected: SelectedRoute {
                agent: candidate.agent,
                model: candidate.model,
                fallback_applied: candidate.fallback_applied,
                requires_confirmation: requires_conf,
                policy_reason,
                preferred_certificate: preferred_cert,
                circuit_breaker_filtered: 0,
                quota_aware: state_store.is_some(),
                quota_penalized_candidates: Vec::new(),
            },
            is_gated,
            quota,
            is_down_or_deprecated: is_model_down_or_deprecated,
            cost_hint,
            has_promo: promo.as_deref().map(|p| !p.trim().is_empty()).unwrap_or(false),
        });
    }

    if allowed_candidates.is_empty() {
        return SelectedRoute {
            agent: rule.default_agent.clone(),
            model: rule.default_model.clone(),
            fallback_applied: false,
            requires_confirmation: true,
            policy_reason: "no detected allowed route; returning default for explicit human review"
                .to_string(),
            preferred_certificate: None,
            circuit_breaker_filtered,
            quota_aware: state_store.is_some(),
            quota_penalized_candidates: Vec::new(),
        };
    }

    let any_healthy = allowed_candidates
        .iter()
        .any(|c| !c.quota.is_exhausted && !c.is_down_or_deprecated);
    let mut quota_penalized_candidates = Vec::new();
    for c in &allowed_candidates {
        if c.quota.is_exhausted || c.quota.is_warning || c.is_down_or_deprecated {
            quota_penalized_candidates.push(c.selected.agent.clone());
        }
    }

    allowed_candidates.sort_by(|a, b| {
        let tier_b = b.tier(any_healthy);
        let tier_a = a.tier(any_healthy);
        if tier_b != tier_a {
            return tier_b.cmp(&tier_a);
        }

        // Same tier! Compare cost and promo:
        match (b.cost_hint, a.cost_hint) {
            (Some(cost_b), Some(cost_a)) => {
                let diff = (cost_b - cost_a).abs();
                if diff > 1e-7 {
                    // Lower cost is better: cost_a < cost_b means a is better than b
                    return cost_a.partial_cmp(&cost_b).unwrap_or(std::cmp::Ordering::Equal);
                }
                // Equivalent cost: promo preference
                if b.has_promo != a.has_promo {
                    return b.has_promo.cmp(&a.has_promo);
                }
            }
            (None, None) => {
                if b.has_promo != a.has_promo {
                    return b.has_promo.cmp(&a.has_promo);
                }
            }
            (Some(_), None) => {
                if b.has_promo != a.has_promo {
                    return b.has_promo.cmp(&a.has_promo);
                }
            }
            (None, Some(_)) => {
                if b.has_promo != a.has_promo {
                    return b.has_promo.cmp(&a.has_promo);
                }
            }
        }

        let prio_b = usize::MAX - b.original_index;
        let prio_a = usize::MAX - a.original_index;
        prio_b.cmp(&prio_a)
    });

    let mut chosen = allowed_candidates.remove(0).selected;
    chosen.circuit_breaker_filtered = circuit_breaker_filtered;
    chosen.quota_aware = state_store.is_some();
    chosen.quota_penalized_candidates = quota_penalized_candidates;

    if chosen.agent != rule.default_agent || chosen.model != rule.default_model {
        chosen.fallback_applied = true;
    }

    if chosen.fallback_applied
        && chosen
            .quota_penalized_candidates
            .contains(&rule.default_agent)
    {
        chosen.policy_reason = format!(
            "quota_penalized:{}; {}",
            rule.default_agent, chosen.policy_reason
        );
    }

    chosen
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
            None,
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
            None,
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
            None,
        )
        .unwrap();

        assert_eq!(decision.selected_agent, "primary-agent");
        assert_eq!(decision.selected_model, "primary-model");
        assert!(!decision.fallback_applied);
        assert_eq!(decision.circuit_breaker_filtered, 0);
    }

    #[test]
    fn test_route_avoids_five_hour_exhausted_scope_and_picks_alternative() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::state::open(Some(&dir.path().join("state.sqlite"))).unwrap();

        let config = parse_config(
            r#"{
                "schema_version": 1,
                "approval_required_model_patterns": ["opus"],
                "routes": [{
                    "task_kind": "documentation",
                    "default_agent": "agy",
                    "default_model": "gemini-3.7-flash-high",
                    "cheap_sufficient": "qwen-code/qwen3.6-flash",
                    "escalate_to": "none",
                    "avoid": [],
                    "rationale": "testing five_hour quota exhaustion"
                }]
            }"#,
        )
        .unwrap();

        let detected = DetectReport {
            schema_version: 1,
            agents: vec![
                AgentDetection {
                    name: "agy".to_string(),
                    binary: "agy-runner".to_string(),
                    detected: true,
                    binary_path: Some("agy-runner".to_string()),
                    adapter: AdapterStatus::Available,
                    secrets_read: false,
                },
                AgentDetection {
                    name: "qwen-code".to_string(),
                    binary: "qwen-runner".to_string(),
                    detected: true,
                    binary_path: Some("qwen-runner".to_string()),
                    adapter: AdapterStatus::Available,
                    secrets_read: false,
                },
            ],
            secrets_read: false,
        };

        // AGY has five-hour quota exhausted (0.0% remaining)
        let exhausted_snapshot = crate::state::QuotaSnapshotInput {
            provider: "agy".to_string(),
            scope: "gemini-five-hour".to_string(),
            remaining_pct: Some(0.0),
            used_pct: Some(100.0),
            status: Some("exhausted".to_string()),
            reset_at_unix: None,
            captured_at_unix: Some(crate::quota::now_unix()),
            metadata_json: None,
        };
        store.insert_quota_snapshot(&exhausted_snapshot).unwrap();

        // Qwen has healthy quota
        let healthy_snapshot = crate::state::QuotaSnapshotInput {
            provider: "qwen".to_string(),
            scope: "general".to_string(),
            remaining_pct: Some(85.0),
            used_pct: Some(15.0),
            status: Some("ok".to_string()),
            reset_at_unix: None,
            captured_at_unix: Some(crate::quota::now_unix()),
            metadata_json: None,
        };
        store.insert_quota_snapshot(&healthy_snapshot).unwrap();

        let decision = decide_with_detected(
            &config,
            "documentation",
            false,
            "test",
            &detected,
            None,
            Some(&store),
            None,
        )
        .unwrap();

        assert_eq!(decision.selected_agent, "qwen-code");
        assert_eq!(decision.selected_model, "qwen3.6-flash");
        assert!(decision.fallback_applied);
    }

    #[test]
    fn test_route_prefers_gated_when_weekly_quota_high_and_allow_gated() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::state::open(Some(&dir.path().join("state.sqlite"))).unwrap();

        let config = parse_config(
            r#"{
                "schema_version": 1,
                "approval_required_model_patterns": ["sonnet"],
                "routes": [{
                    "task_kind": "refactor",
                    "default_agent": "agy",
                    "default_model": "gemini-3.7-flash-high",
                    "cheap_sufficient": "none",
                    "escalate_to": "claude-code/claude-sonnet-5",
                    "avoid": [],
                    "rationale": "testing gated preference with high weekly quota when non-gated default is exhausted"
                }]
            }"#,
        )
        .unwrap();

        let detected = DetectReport {
            schema_version: 1,
            agents: vec![
                AgentDetection {
                    name: "agy".to_string(),
                    binary: "agy-runner".to_string(),
                    detected: true,
                    binary_path: Some("agy-runner".to_string()),
                    adapter: AdapterStatus::Available,
                    secrets_read: false,
                },
                AgentDetection {
                    name: "claude-code".to_string(),
                    binary: "claude-runner".to_string(),
                    detected: true,
                    binary_path: Some("claude-runner".to_string()),
                    adapter: AdapterStatus::Gated,
                    secrets_read: false,
                },
            ],
            secrets_read: false,
        };

        // AGY (non-gated default) is exhausted
        let agy_snapshot = crate::state::QuotaSnapshotInput {
            provider: "agy".to_string(),
            scope: "gemini-five-hour".to_string(),
            remaining_pct: Some(0.0),
            used_pct: Some(100.0),
            status: Some("exhausted".to_string()),
            reset_at_unix: None,
            captured_at_unix: Some(crate::quota::now_unix()),
            metadata_json: None,
        };
        store.insert_quota_snapshot(&agy_snapshot).unwrap();

        // Claude has high weekly quota (80% remaining)
        let claude_snapshot = crate::state::QuotaSnapshotInput {
            provider: "claude-code".to_string(),
            scope: "weekly".to_string(),
            remaining_pct: Some(80.0),
            used_pct: Some(20.0),
            status: Some("ok".to_string()),
            reset_at_unix: None,
            captured_at_unix: Some(crate::quota::now_unix()),
            metadata_json: None,
        };
        store.insert_quota_snapshot(&claude_snapshot).unwrap();

        // 1. With allow_gated = true, claude-code is selected as healthy fallback
        let decision_gated = decide_with_detected(
            &config,
            "refactor",
            true,
            "test",
            &detected,
            None,
            Some(&store),
            None,
        )
        .unwrap();

        assert_eq!(decision_gated.selected_agent, "claude-code");
        assert_eq!(decision_gated.selected_model, "claude-sonnet-5");
        assert!(decision_gated.fallback_applied);
        assert_eq!(decision_gated.quota_aware, Some(true));
        assert!(decision_gated
            .quota_penalized_candidates
            .contains(&"agy".to_string()));

        // 2. With allow_gated = false, policy blocks claude-code and returns agy with confirmation
        let decision_ungated = decide_with_detected(
            &config,
            "refactor",
            false,
            "test",
            &detected,
            None,
            Some(&store),
            None,
        )
        .unwrap();

        assert_eq!(decision_ungated.selected_agent, "agy");
        assert_eq!(decision_ungated.selected_model, "gemini-3.7-flash-high");
        assert!(!decision_ungated.fallback_applied);
    }

    #[test]
    fn test_route_healthy_default_is_not_displaced_by_gated_even_with_allow_gated() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::state::open(Some(&dir.path().join("state.sqlite"))).unwrap();

        let config = parse_config(
            r#"{
                "schema_version": 1,
                "approval_required_model_patterns": ["sonnet"],
                "routes": [{
                    "task_kind": "refactor",
                    "default_agent": "agy",
                    "default_model": "gemini-3.7-flash-high",
                    "cheap_sufficient": "none",
                    "escalate_to": "claude-code/claude-sonnet-5",
                    "avoid": [],
                    "rationale": "avoid selecting gated Claude by default when non-gated default is healthy"
                }]
            }"#,
        )
        .unwrap();

        let detected = DetectReport {
            schema_version: 1,
            agents: vec![
                AgentDetection {
                    name: "agy".to_string(),
                    binary: "agy-runner".to_string(),
                    detected: true,
                    binary_path: Some("agy-runner".to_string()),
                    adapter: AdapterStatus::Available,
                    secrets_read: false,
                },
                AgentDetection {
                    name: "claude-code".to_string(),
                    binary: "claude-runner".to_string(),
                    detected: true,
                    binary_path: Some("claude-runner".to_string()),
                    adapter: AdapterStatus::Gated,
                    secrets_read: false,
                },
            ],
            secrets_read: false,
        };

        // AGY is 100% healthy
        let agy_snapshot = crate::state::QuotaSnapshotInput {
            provider: "agy".to_string(),
            scope: "gemini-five-hour".to_string(),
            remaining_pct: Some(90.0),
            used_pct: Some(10.0),
            status: Some("ok".to_string()),
            reset_at_unix: None,
            captured_at_unix: Some(crate::quota::now_unix()),
            metadata_json: None,
        };
        store.insert_quota_snapshot(&agy_snapshot).unwrap();

        // Claude has 100% weekly quota
        let claude_snapshot = crate::state::QuotaSnapshotInput {
            provider: "claude-code".to_string(),
            scope: "weekly".to_string(),
            remaining_pct: Some(100.0),
            used_pct: Some(0.0),
            status: Some("ok".to_string()),
            reset_at_unix: None,
            captured_at_unix: Some(crate::quota::now_unix()),
            metadata_json: None,
        };
        store.insert_quota_snapshot(&claude_snapshot).unwrap();

        // Even with allow_gated = true, healthy default AGY is selected!
        let decision = decide_with_detected(
            &config,
            "refactor",
            true,
            "test",
            &detected,
            None,
            Some(&store),
            None,
        )
        .unwrap();

        assert_eq!(decision.selected_agent, "agy");
        assert_eq!(decision.selected_model, "gemini-3.7-flash-high");
        assert!(!decision.fallback_applied);
    }

    #[test]
    fn test_route_without_quota_snapshots_behaves_identically_to_baseline() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::state::open(Some(&dir.path().join("state.sqlite"))).unwrap();

        let config = parse_config(
            r#"{
                "schema_version": 1,
                "approval_required_model_patterns": ["opus"],
                "routes": [{
                    "task_kind": "documentation",
                    "default_agent": "qwen-code",
                    "default_model": "qwen3.6-flash",
                    "cheap_sufficient": "agy/gemini-3.6-flash-low",
                    "escalate_to": "none",
                    "avoid": [],
                    "rationale": "baseline comparison without snapshots"
                }]
            }"#,
        )
        .unwrap();

        let detected = DetectReport {
            schema_version: 1,
            agents: vec![
                AgentDetection {
                    name: "qwen-code".to_string(),
                    binary: "qwen-runner".to_string(),
                    detected: true,
                    binary_path: Some("qwen-runner".to_string()),
                    adapter: AdapterStatus::Available,
                    secrets_read: false,
                },
                AgentDetection {
                    name: "agy".to_string(),
                    binary: "agy-runner".to_string(),
                    detected: true,
                    binary_path: Some("agy-runner".to_string()),
                    adapter: AdapterStatus::Available,
                    secrets_read: false,
                },
            ],
            secrets_read: false,
        };

        // Decision with empty store vs None store are identical
        let decision_with_store = decide_with_detected(
            &config,
            "documentation",
            false,
            "test",
            &detected,
            None,
            Some(&store),
            None,
        )
        .unwrap();

        let decision_without_store = decide_with_detected(
            &config,
            "documentation",
            false,
            "test",
            &detected,
            None,
            None,
            None,
        )
        .unwrap();

        assert_eq!(decision_with_store.selected_agent, "qwen-code");
        assert_eq!(decision_with_store.selected_model, "qwen3.6-flash");
        assert!(!decision_with_store.fallback_applied);

        assert_eq!(
            decision_without_store.selected_agent,
            decision_with_store.selected_agent
        );
        assert_eq!(
            decision_without_store.selected_model,
            decision_with_store.selected_model
        );
    }

    #[test]
    fn test_route_with_cert_dir_without_snapshots_matches_baseline() {
        let cert_dir = tempfile::tempdir().unwrap();
        // Create a positive certificate for the cheap_sufficient candidate (agy/gemini-3.6-flash-low)
        let cert_json = r#"{
            "schema_version": 1,
            "certificate_id": "cert-agy-001",
            "agent": "agy",
            "model": "gemini-3.6-flash-low",
            "task_kind": "documentation",
            "issued_at_unix": 1000,
            "expires_at_unix": 9999999999,
            "receipt": {
                "schema_version": 1,
                "correlation_id": "test",
                "agent": "agy",
                "model": "gemini-3.6-flash-low",
                "task_file": "test",
                "task_sha256": "test",
                "started_at_unix": 1000,
                "finished_at_unix": 1001,
                "duration_ms": 1000,
                "status": "succeeded",
                "exit_code": 0,
                "stdout_tail": "ok",
                "stderr_tail": "",
                "timed_out": false,
                "timeout_seconds": 30,
                "secrets_read": false,
                "cleanup_attempted": true,
                "cleanup_succeeded": true
            }
        }"#;
        std::fs::write(cert_dir.path().join("cert-agy.json"), cert_json).unwrap();
        let cert_store = crate::certstore::CertificateStore::load_dir(cert_dir.path()).unwrap();

        let config = parse_config(
            r#"{
                "schema_version": 1,
                "approval_required_model_patterns": ["opus"],
                "routes": [{
                    "task_kind": "documentation",
                    "default_agent": "qwen-code",
                    "default_model": "qwen3.6-flash",
                    "cheap_sufficient": "agy/gemini-3.6-flash-low",
                    "escalate_to": "none",
                    "avoid": [],
                    "rationale": "testing that certs do not reorder candidates without snapshots"
                }]
            }"#,
        )
        .unwrap();

        let detected = DetectReport {
            schema_version: 1,
            agents: vec![
                AgentDetection {
                    name: "qwen-code".to_string(),
                    binary: "qwen-runner".to_string(),
                    detected: true,
                    binary_path: Some("qwen-runner".to_string()),
                    adapter: AdapterStatus::Available,
                    secrets_read: false,
                },
                AgentDetection {
                    name: "agy".to_string(),
                    binary: "agy-runner".to_string(),
                    detected: true,
                    binary_path: Some("agy-runner".to_string()),
                    adapter: AdapterStatus::Available,
                    secrets_read: false,
                },
            ],
            secrets_read: false,
        };

        // Decision with cert_store vs without cert_store both select qwen-code (default)
        let decision_with_certs = decide_with_detected(
            &config,
            "documentation",
            false,
            "test",
            &detected,
            Some(&cert_store),
            None,
            None,
        )
        .unwrap();

        let decision_baseline = decide_with_detected(
            &config,
            "documentation",
            false,
            "test",
            &detected,
            None,
            None,
            None,
        )
        .unwrap();

        assert_eq!(decision_with_certs.selected_agent, "qwen-code");
        assert_eq!(decision_with_certs.selected_model, "qwen3.6-flash");
        assert!(!decision_with_certs.fallback_applied);
        assert_eq!(
            decision_with_certs.selected_agent,
            decision_baseline.selected_agent
        );
        assert_eq!(
            decision_with_certs.selected_model,
            decision_baseline.selected_model
        );
    }

    #[test]
    fn test_route_certified_candidate_does_not_mask_exhaustion() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::state::open(Some(&dir.path().join("state.sqlite"))).unwrap();

        let cert_dir = tempfile::tempdir().unwrap();
        let cert_json = r#"{
            "schema_version": 1,
            "certificate_id": "cert-qwen-001",
            "agent": "qwen-code",
            "model": "qwen3.6-flash",
            "task_kind": "documentation",
            "issued_at_unix": 1000,
            "expires_at_unix": 9999999999,
            "receipt": {
                "schema_version": 1,
                "correlation_id": "test",
                "agent": "qwen-code",
                "model": "qwen3.6-flash",
                "task_file": "test",
                "task_sha256": "test",
                "started_at_unix": 1000,
                "finished_at_unix": 1001,
                "duration_ms": 1000,
                "status": "succeeded",
                "exit_code": 0,
                "stdout_tail": "ok",
                "stderr_tail": "",
                "timed_out": false,
                "timeout_seconds": 30,
                "secrets_read": false,
                "cleanup_attempted": true,
                "cleanup_succeeded": true
            }
        }"#;
        std::fs::write(cert_dir.path().join("cert-qwen.json"), cert_json).unwrap();
        let cert_store = crate::certstore::CertificateStore::load_dir(cert_dir.path()).unwrap();

        let config = parse_config(
            r#"{
                "schema_version": 1,
                "approval_required_model_patterns": ["opus"],
                "routes": [{
                    "task_kind": "documentation",
                    "default_agent": "qwen-code",
                    "default_model": "qwen3.6-flash",
                    "cheap_sufficient": "agy/gemini-3.6-flash-low",
                    "escalate_to": "none",
                    "avoid": [],
                    "rationale": "testing that certs do not mask quota exhaustion"
                }]
            }"#,
        )
        .unwrap();

        let detected = DetectReport {
            schema_version: 1,
            agents: vec![
                AgentDetection {
                    name: "qwen-code".to_string(),
                    binary: "qwen-runner".to_string(),
                    detected: true,
                    binary_path: Some("qwen-runner".to_string()),
                    adapter: AdapterStatus::Available,
                    secrets_read: false,
                },
                AgentDetection {
                    name: "agy".to_string(),
                    binary: "agy-runner".to_string(),
                    detected: true,
                    binary_path: Some("agy-runner".to_string()),
                    adapter: AdapterStatus::Available,
                    secrets_read: false,
                },
            ],
            secrets_read: false,
        };

        // Qwen is certified, but has exhausted quota
        let qwen_snapshot = crate::state::QuotaSnapshotInput {
            provider: "qwen".to_string(),
            scope: "general".to_string(),
            remaining_pct: Some(0.0),
            used_pct: Some(100.0),
            status: Some("exhausted".to_string()),
            reset_at_unix: None,
            captured_at_unix: Some(crate::quota::now_unix()),
            metadata_json: None,
        };
        store.insert_quota_snapshot(&qwen_snapshot).unwrap();

        // AGY is healthy (uncertified)
        let agy_snapshot = crate::state::QuotaSnapshotInput {
            provider: "agy".to_string(),
            scope: "gemini-weekly".to_string(),
            remaining_pct: Some(90.0),
            used_pct: Some(10.0),
            status: Some("ok".to_string()),
            reset_at_unix: None,
            captured_at_unix: Some(crate::quota::now_unix()),
            metadata_json: None,
        };
        store.insert_quota_snapshot(&agy_snapshot).unwrap();

        let decision = decide_with_detected(
            &config,
            "documentation",
            false,
            "test",
            &detected,
            Some(&cert_store),
            Some(&store),
            None,
        )
        .unwrap();

        // Fallback to agy because qwen-code is exhausted despite having a certificate!
        assert_eq!(decision.selected_agent, "agy");
        assert_eq!(decision.selected_model, "gemini-3.6-flash-low");
        assert!(decision.fallback_applied);
    }

    #[test]
    fn test_route_snapshot_exhausted_with_expired_reset_does_not_penalize() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::state::open(Some(&dir.path().join("state.sqlite"))).unwrap();

        let config = parse_config(
            r#"{
                "schema_version": 1,
                "approval_required_model_patterns": ["opus"],
                "routes": [{
                    "task_kind": "documentation",
                    "default_agent": "agy",
                    "default_model": "gemini-3.7-flash-high",
                    "cheap_sufficient": "qwen-code/qwen3.6-flash",
                    "escalate_to": "none",
                    "avoid": [],
                    "rationale": "testing expired reset timestamp"
                }]
            }"#,
        )
        .unwrap();

        let detected = DetectReport {
            schema_version: 1,
            agents: vec![
                AgentDetection {
                    name: "agy".to_string(),
                    binary: "agy-runner".to_string(),
                    detected: true,
                    binary_path: Some("agy-runner".to_string()),
                    adapter: AdapterStatus::Available,
                    secrets_read: false,
                },
                AgentDetection {
                    name: "qwen-code".to_string(),
                    binary: "qwen-runner".to_string(),
                    detected: true,
                    binary_path: Some("qwen-runner".to_string()),
                    adapter: AdapterStatus::Available,
                    secrets_read: false,
                },
            ],
            secrets_read: false,
        };

        let now = crate::quota::now_unix();
        // AGY was exhausted, but reset_at_unix has already passed (now - 60s)
        let agy_snapshot = crate::state::QuotaSnapshotInput {
            provider: "agy".to_string(),
            scope: "gemini-five-hour".to_string(),
            remaining_pct: Some(0.0),
            used_pct: Some(100.0),
            status: Some("exhausted".to_string()),
            reset_at_unix: Some(now.saturating_sub(60)),
            captured_at_unix: Some(now.saturating_sub(120)),
            metadata_json: None,
        };
        store.insert_quota_snapshot(&agy_snapshot).unwrap();

        let decision = decide_with_detected(
            &config,
            "documentation",
            false,
            "test",
            &detected,
            None,
            Some(&store),
            None,
        )
        .unwrap();

        // AGY is selected as default because the reset window expired and quota is recovered!
        assert_eq!(decision.selected_agent, "agy");
        assert_eq!(decision.selected_model, "gemini-3.7-flash-high");
        assert!(!decision.fallback_applied);
    }

    #[test]
    fn test_route_snapshot_exhausted_older_than_ttl_does_not_penalize() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::state::open(Some(&dir.path().join("state.sqlite"))).unwrap();

        let config = parse_config(
            r#"{
                "schema_version": 1,
                "approval_required_model_patterns": ["opus"],
                "routes": [{
                    "task_kind": "documentation",
                    "default_agent": "agy",
                    "default_model": "gemini-3.7-flash-high",
                    "cheap_sufficient": "qwen-code/qwen3.6-flash",
                    "escalate_to": "none",
                    "avoid": [],
                    "rationale": "testing snapshot TTL expiry"
                }]
            }"#,
        )
        .unwrap();

        let detected = DetectReport {
            schema_version: 1,
            agents: vec![
                AgentDetection {
                    name: "agy".to_string(),
                    binary: "agy-runner".to_string(),
                    detected: true,
                    binary_path: Some("agy-runner".to_string()),
                    adapter: AdapterStatus::Available,
                    secrets_read: false,
                },
                AgentDetection {
                    name: "qwen-code".to_string(),
                    binary: "qwen-runner".to_string(),
                    detected: true,
                    binary_path: Some("qwen-runner".to_string()),
                    adapter: AdapterStatus::Available,
                    secrets_read: false,
                },
            ],
            secrets_read: false,
        };

        let now = crate::quota::now_unix();
        // AGY was exhausted, but snapshot is older than 24h TTL (captured 100_000 seconds ago)
        let agy_snapshot = crate::state::QuotaSnapshotInput {
            provider: "agy".to_string(),
            scope: "gemini-five-hour".to_string(),
            remaining_pct: Some(0.0),
            used_pct: Some(100.0),
            status: Some("exhausted".to_string()),
            reset_at_unix: None,
            captured_at_unix: Some(now.saturating_sub(100_000)),
            metadata_json: None,
        };
        store.insert_quota_snapshot(&agy_snapshot).unwrap();

        let decision = decide_with_detected(
            &config,
            "documentation",
            false,
            "test",
            &detected,
            None,
            Some(&store),
            None,
        )
        .unwrap();

        // Stale snapshot is ignored; AGY is selected as default
        assert_eq!(decision.selected_agent, "agy");
        assert_eq!(decision.selected_model, "gemini-3.7-flash-high");
        assert!(!decision.fallback_applied);
    }

    #[test]
    fn test_route_avoids_candidate_at_or_below_critical_threshold() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::state::open(Some(&dir.path().join("state.sqlite"))).unwrap();

        let config = parse_config(
            r#"{
                "schema_version": 1,
                "approval_required_model_patterns": ["opus"],
                "routes": [{
                    "task_kind": "documentation",
                    "default_agent": "agy",
                    "default_model": "gemini-3.7-flash-high",
                    "cheap_sufficient": "qwen-code/qwen3.6-flash",
                    "escalate_to": "none",
                    "avoid": [],
                    "rationale": "testing critical threshold penalty"
                }]
            }"#,
        )
        .unwrap();

        let detected = DetectReport {
            schema_version: 1,
            agents: vec![
                AgentDetection {
                    name: "agy".to_string(),
                    binary: "agy-runner".to_string(),
                    detected: true,
                    binary_path: Some("agy-runner".to_string()),
                    adapter: AdapterStatus::Available,
                    secrets_read: false,
                },
                AgentDetection {
                    name: "qwen-code".to_string(),
                    binary: "qwen-runner".to_string(),
                    detected: true,
                    binary_path: Some("qwen-runner".to_string()),
                    adapter: AdapterStatus::Available,
                    secrets_read: false,
                },
            ],
            secrets_read: false,
        };

        // AGY has 10% remaining immediate quota (<= 15% threshold)
        let agy_snapshot = crate::state::QuotaSnapshotInput {
            provider: "agy".to_string(),
            scope: "gemini-five-hour".to_string(),
            remaining_pct: Some(10.0),
            used_pct: Some(90.0),
            status: Some("ok".to_string()),
            reset_at_unix: None,
            captured_at_unix: Some(crate::quota::now_unix()),
            metadata_json: None,
        };
        store.insert_quota_snapshot(&agy_snapshot).unwrap();

        // Qwen is healthy (80%)
        let qwen_snapshot = crate::state::QuotaSnapshotInput {
            provider: "qwen".to_string(),
            scope: "general".to_string(),
            remaining_pct: Some(80.0),
            used_pct: Some(20.0),
            status: Some("ok".to_string()),
            reset_at_unix: None,
            captured_at_unix: Some(crate::quota::now_unix()),
            metadata_json: None,
        };
        store.insert_quota_snapshot(&qwen_snapshot).unwrap();

        let decision = decide_with_detected(
            &config,
            "documentation",
            false,
            "test",
            &detected,
            None,
            Some(&store),
            None,
        )
        .unwrap();

        // Qwen is chosen as fallback because AGY is in critical warning tier (<= 15%)
        assert_eq!(decision.selected_agent, "qwen-code");
        assert_eq!(decision.selected_model, "qwen3.6-flash");
        assert!(decision.fallback_applied);
    }

    #[test]
    fn test_route_status_warning_penalizes_candidate() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::state::open(Some(&dir.path().join("state.sqlite"))).unwrap();

        let config = parse_config(
            r#"{
                "schema_version": 1,
                "approval_required_model_patterns": ["opus"],
                "routes": [{
                    "task_kind": "documentation",
                    "default_agent": "agy",
                    "default_model": "gemini-3.7-flash-high",
                    "cheap_sufficient": "qwen-code/qwen3.6-flash",
                    "escalate_to": "none",
                    "avoid": [],
                    "rationale": "testing warning status penalty"
                }]
            }"#,
        )
        .unwrap();

        let detected = DetectReport {
            schema_version: 1,
            agents: vec![
                AgentDetection {
                    name: "agy".to_string(),
                    binary: "agy-runner".to_string(),
                    detected: true,
                    binary_path: Some("agy-runner".to_string()),
                    adapter: AdapterStatus::Available,
                    secrets_read: false,
                },
                AgentDetection {
                    name: "qwen-code".to_string(),
                    binary: "qwen-runner".to_string(),
                    detected: true,
                    binary_path: Some("qwen-runner".to_string()),
                    adapter: AdapterStatus::Available,
                    secrets_read: false,
                },
            ],
            secrets_read: false,
        };

        // AGY has status "warning"
        let agy_snapshot = crate::state::QuotaSnapshotInput {
            provider: "agy".to_string(),
            scope: "gemini-five-hour".to_string(),
            remaining_pct: Some(40.0),
            used_pct: Some(60.0),
            status: Some("warning".to_string()),
            reset_at_unix: None,
            captured_at_unix: Some(crate::quota::now_unix()),
            metadata_json: None,
        };
        store.insert_quota_snapshot(&agy_snapshot).unwrap();

        // Qwen is ok
        let qwen_snapshot = crate::state::QuotaSnapshotInput {
            provider: "qwen".to_string(),
            scope: "general".to_string(),
            remaining_pct: Some(80.0),
            used_pct: Some(20.0),
            status: Some("ok".to_string()),
            reset_at_unix: None,
            captured_at_unix: Some(crate::quota::now_unix()),
            metadata_json: None,
        };
        store.insert_quota_snapshot(&qwen_snapshot).unwrap();

        let decision = decide_with_detected(
            &config,
            "documentation",
            false,
            "test",
            &detected,
            None,
            Some(&store),
            None,
        )
        .unwrap();

        // Qwen is chosen as fallback
        assert_eq!(decision.selected_agent, "qwen-code");
        assert_eq!(decision.selected_model, "qwen3.6-flash");
        assert!(decision.fallback_applied);
    }

    #[test]
    fn test_route_observability_fields_present_and_accurate() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::state::open(Some(&dir.path().join("state.sqlite"))).unwrap();

        let config = parse_config(
            r#"{
                "schema_version": 1,
                "approval_required_model_patterns": ["opus"],
                "routes": [{
                    "task_kind": "documentation",
                    "default_agent": "agy",
                    "default_model": "gemini-3.7-flash-high",
                    "cheap_sufficient": "qwen-code/qwen3.6-flash",
                    "escalate_to": "none",
                    "avoid": [],
                    "rationale": "testing observability fields"
                }]
            }"#,
        )
        .unwrap();

        let detected = DetectReport {
            schema_version: 1,
            agents: vec![
                AgentDetection {
                    name: "agy".to_string(),
                    binary: "agy-runner".to_string(),
                    detected: true,
                    binary_path: Some("agy-runner".to_string()),
                    adapter: AdapterStatus::Available,
                    secrets_read: false,
                },
                AgentDetection {
                    name: "qwen-code".to_string(),
                    binary: "qwen-runner".to_string(),
                    detected: true,
                    binary_path: Some("qwen-runner".to_string()),
                    adapter: AdapterStatus::Available,
                    secrets_read: false,
                },
            ],
            secrets_read: false,
        };

        let agy_snapshot = crate::state::QuotaSnapshotInput {
            provider: "agy".to_string(),
            scope: "gemini-five-hour".to_string(),
            remaining_pct: Some(0.0),
            used_pct: Some(100.0),
            status: Some("exhausted".to_string()),
            reset_at_unix: None,
            captured_at_unix: Some(crate::quota::now_unix()),
            metadata_json: None,
        };
        store.insert_quota_snapshot(&agy_snapshot).unwrap();

        let decision = decide_with_detected(
            &config,
            "documentation",
            false,
            "test",
            &detected,
            None,
            Some(&store),
            None,
        )
        .unwrap();

        assert_eq!(decision.quota_aware, Some(true));
        assert_eq!(decision.quota_penalized_candidates, vec!["agy".to_string()]);
        assert!(decision
            .selected_policy_reason
            .contains("quota_penalized:agy"));
    }

    #[test]
    fn test_route_quota_unknown_does_not_penalize() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::state::open(Some(&dir.path().join("state.sqlite"))).unwrap();

        let config = parse_config(
            r#"{
                "schema_version": 1,
                "approval_required_model_patterns": ["opus"],
                "routes": [{
                    "task_kind": "analysis",
                    "default_agent": "qwen-code",
                    "default_model": "qwen3.6-flash",
                    "cheap_sufficient": "agy/gemini-3.6-flash-low",
                    "escalate_to": "none",
                    "avoid": [],
                    "rationale": "testing quota_unknown neutrality"
                }]
            }"#,
        )
        .unwrap();

        let detected = DetectReport {
            schema_version: 1,
            agents: vec![
                AgentDetection {
                    name: "qwen-code".to_string(),
                    binary: "qwen-runner".to_string(),
                    detected: true,
                    binary_path: Some("qwen-runner".to_string()),
                    adapter: AdapterStatus::Available,
                    secrets_read: false,
                },
                AgentDetection {
                    name: "agy".to_string(),
                    binary: "agy-runner".to_string(),
                    detected: true,
                    binary_path: Some("agy-runner".to_string()),
                    adapter: AdapterStatus::Available,
                    secrets_read: false,
                },
            ],
            secrets_read: false,
        };

        // Qwen is quota_unknown
        let qwen_snapshot = crate::state::QuotaSnapshotInput {
            provider: "qwen".to_string(),
            scope: "general".to_string(),
            remaining_pct: None,
            used_pct: None,
            status: Some("quota_unknown".to_string()),
            reset_at_unix: None,
            captured_at_unix: Some(1000),
            metadata_json: None,
        };
        store.insert_quota_snapshot(&qwen_snapshot).unwrap();

        // AGY has 90% quota
        let agy_snapshot = crate::state::QuotaSnapshotInput {
            provider: "agy".to_string(),
            scope: "gemini-weekly".to_string(),
            remaining_pct: Some(90.0),
            used_pct: Some(10.0),
            status: Some("ok".to_string()),
            reset_at_unix: None,
            captured_at_unix: Some(1000),
            metadata_json: None,
        };
        store.insert_quota_snapshot(&agy_snapshot).unwrap();

        let decision = decide_with_detected(
            &config,
            "analysis",
            false,
            "test",
            &detected,
            None,
            Some(&store),
            None,
        )
        .unwrap();

        // Qwen is NOT penalized, remains default selected agent
        assert_eq!(decision.selected_agent, "qwen-code");
        assert_eq!(decision.selected_model, "qwen3.6-flash");
        assert!(!decision.fallback_applied);
    }
}
