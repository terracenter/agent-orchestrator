use crate::adapters::{AdapterStatus, AgentDetection};
use crate::certstore::{is_certified, CertificateStore};
use crate::models::ModelsCatalog;
use crate::policy::{self, PolicyConfig};
use crate::receipt::FailureClass;
use crate::route::RoutingConfig;

pub fn classify_failure(
    reason: Option<&str>,
    stderr_tail: &str,
    exit_code: Option<i32>,
    duration_ms: u128,
    timeout_seconds: u64,
) -> Option<FailureClass> {
    let reason_text = reason.unwrap_or_default().to_ascii_lowercase();
    let stderr_text = stderr_tail.to_ascii_lowercase();

    // 1. Permission Denied (e.g. headless jetski auto-denied, read_file, read_url, permission_denied)
    if reason_text.contains("permission")
        || reason_text.contains("jetski:")
        || reason_text.contains("auto-denied")
        || reason_text.contains("cannot prompt for")
        || stderr_text.contains("permission")
        || stderr_text.contains("jetski:")
        || stderr_text.contains("auto-denied")
        || stderr_text.contains("cannot prompt for")
    {
        return Some(FailureClass::PermissionDenied);
    }

    // 2. Auth Failure (e.g. auth_401, 401 Unauthorized, API key invalid/expired)
    if reason_text.contains("auth_401")
        || reason_text.contains("401")
        || reason_text.contains("unauthorized")
        || reason_text.contains("api_key")
        || reason_text.contains("api key")
        || reason_text.contains("invalid key")
        || reason_text.contains("token expired")
        || reason_text.contains("authentication failed")
        || stderr_text.contains("auth_401")
        || stderr_text.contains("401 unauthorized")
        || stderr_text.contains("unauthorized")
        || stderr_text.contains("invalid api key")
        || stderr_text.contains("invalid key")
        || stderr_text.contains("token expired")
        || stderr_text.contains("authentication failed")
    {
        return Some(FailureClass::AuthFailure);
    }

    // 3. Timeout (e.g. timed_out, timeout_sin_evidencia, duration_ms >= timeout_seconds * 1000)
    if reason_text.contains("timed_out")
        || reason_text.contains("timeout")
        || reason_text.contains("timed out")
        || stderr_text.contains("timed out")
        || (timeout_seconds > 0 && duration_ms >= (timeout_seconds as u128) * 1000)
    {
        return Some(FailureClass::Timeout);
    }

    // 4. Model Unavailable (e.g. model retired, quota exhausted, 429 rate limit, model down, overloaded)
    if reason_text.contains("model_unavailable")
        || reason_text.contains("model not found")
        || reason_text.contains("quota")
        || reason_text.contains("rate limit")
        || reason_text.contains("429")
        || reason_text.contains("model down")
        || reason_text.contains("model retired")
        || reason_text.contains("quarantined")
        || reason_text.contains("overloaded")
        || stderr_text.contains("model_unavailable")
        || stderr_text.contains("model not found")
        || stderr_text.contains("quota exceeded")
        || stderr_text.contains("rate limit")
        || stderr_text.contains("429")
        || stderr_text.contains("model down")
        || stderr_text.contains("model retired")
        || stderr_text.contains("overloaded")
    {
        return Some(FailureClass::ModelUnavailable);
    }

    // 5. Network Error (e.g. connection refused, DNS timeout, TLS handshake failure, connection reset, fetch failed)
    if reason_text.contains("network_error")
        || reason_text.contains("connection refused")
        || reason_text.contains("dns timeout")
        || reason_text.contains("tls handshake")
        || reason_text.contains("network unreachable")
        || reason_text.contains("connection reset")
        || reason_text.contains("fetch failed")
        || stderr_text.contains("network_error")
        || stderr_text.contains("connection refused")
        || stderr_text.contains("dns timeout")
        || stderr_text.contains("tls handshake")
        || stderr_text.contains("network unreachable")
        || stderr_text.contains("connection reset")
        || stderr_text.contains("fetch failed")
        || stderr_text.contains("curl: (7)")
        || stderr_text.contains("curl: (6)")
    {
        return Some(FailureClass::NetworkError);
    }

    // 6. Agent Crash (e.g. panic, segfault, non-zero exit code with non-empty stderr, signal)
    if reason_text.contains("agent_crash")
        || reason_text.contains("panic")
        || reason_text.contains("segfault")
        || stderr_text.contains("panic")
        || stderr_text.contains("segfault")
        || stderr_text.contains("sigsegv")
        || stderr_text.contains("sigkill")
        || exit_code.is_some_and(|c| c != 0)
        || !stderr_tail.trim().is_empty()
        || !reason_text.is_empty()
    {
        return Some(FailureClass::AgentCrash);
    }

    None
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FallbackCandidate {
    pub agent: String,
    pub model: String,
    pub is_certified: bool,
    pub certificate_id: Option<String>,
    pub source_rule: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FallbackSelectionRequest<'a> {
    pub task_kind: &'a str,
    pub failed_agent: &'a str,
    pub failed_model: &'a str,
    pub failure_class: FailureClass,
    pub catalog: &'a ModelsCatalog,
    pub detected: &'a [AgentDetection],
    pub certificate_store: Option<&'a CertificateStore>,
    pub routing_config: Option<&'a RoutingConfig>,
    pub policy_config: Option<&'a PolicyConfig>,
    pub allow_gated: bool,
    pub now_unix: u64,
    pub catalog_ttl_secs: u64,
}

#[derive(Debug, Clone)]
pub struct CandidateEligibilityContext<'a> {
    pub catalog: &'a ModelsCatalog,
    pub detected: &'a [AgentDetection],
    pub policy_config: Option<&'a PolicyConfig>,
    pub allow_gated: bool,
    pub now_unix: u64,
    pub catalog_ttl_secs: u64,
}

impl<'a> From<&'a FallbackSelectionRequest<'a>> for CandidateEligibilityContext<'a> {
    fn from(req: &'a FallbackSelectionRequest<'a>) -> Self {
        Self {
            catalog: req.catalog,
            detected: req.detected,
            policy_config: req.policy_config,
            allow_gated: req.allow_gated,
            now_unix: req.now_unix,
            catalog_ttl_secs: req.catalog_ttl_secs,
        }
    }
}

pub fn is_candidate_eligible(agent: &str, model: &str, ctx: &CandidateEligibilityContext) -> bool {
    // 1. Runtime check (CA-2 d): agent present and functional
    let Some(detection) = ctx.detected.iter().find(|d| d.name == agent) else {
        return false;
    };
    if !detection.detected {
        return false;
    }
    match detection.adapter {
        AdapterStatus::Available => {}
        AdapterStatus::Gated if ctx.allow_gated => {}
        _ => return false,
    }

    // 2. Catalog check (CA-2 a, b, c): exists, active, non-stale
    let Some(models) = ctx.catalog.agents.get(agent) else {
        return false;
    };
    let Some(candidate_model) = models.iter().find(|m| m.id == model) else {
        return false;
    };
    if !candidate_model.is_active() {
        return false;
    }
    if candidate_model.is_stale(ctx.now_unix, ctx.catalog_ttl_secs) {
        return false;
    }

    // 3. Policy check
    if let Some(policy) = ctx.policy_config {
        let decision = policy::evaluate(agent, model, detection.adapter, ctx.allow_gated, policy);
        if !decision.allowed {
            return false;
        }
    }

    true
}

pub fn select_fallback_candidate(
    req: &FallbackSelectionRequest,
) -> Option<(FallbackCandidate, String)> {
    let mut candidate_pool: Vec<(String, String, Option<String>)> = Vec::new();

    // 1. Extract candidates from routing matrix rule for this task_kind
    if let Some(routing) = req.routing_config {
        if let Some(rule) = routing.routes.iter().find(|r| r.task_kind == req.task_kind) {
            // Check cheap_sufficient
            if rule.cheap_sufficient != "none" {
                if let Some((agent, model)) = parse_route_expr(&rule.cheap_sufficient) {
                    candidate_pool.push((
                        agent,
                        model,
                        Some(format!(
                            "routing:cheap_sufficient({})",
                            rule.cheap_sufficient
                        )),
                    ));
                }
            }
            // Check escalate_to
            if rule.escalate_to != "none" {
                if let Some((agent, model)) = parse_route_expr(&rule.escalate_to) {
                    candidate_pool.push((
                        agent,
                        model,
                        Some(format!("routing:escalate_to({})", rule.escalate_to)),
                    ));
                }
            }
            // Check default
            candidate_pool.push((
                rule.default_agent.clone(),
                rule.default_model.clone(),
                Some(format!(
                    "routing:default({}/{})",
                    rule.default_agent, rule.default_model
                )),
            ));
        }
    }

    // 2. Add candidates from catalog
    for (agent, models) in &req.catalog.agents {
        for model in models {
            if !candidate_pool
                .iter()
                .any(|(a, m, _)| a == agent && m == &model.id)
            {
                candidate_pool.push((agent.clone(), model.id.clone(), Some("catalog".to_string())));
            }
        }
    }

    let mut evaluated_candidates: Vec<FallbackCandidate> = Vec::new();

    for (agent, model, source_rule) in candidate_pool {
        // Matriz rule: PermissionDenied must NEVER retry on the same agent
        if req.failure_class == FailureClass::PermissionDenied && agent == req.failed_agent {
            continue;
        }

        // Never retry the exact same (agent, model) pair that just failed
        if agent == req.failed_agent && model == req.failed_model {
            continue;
        }

        // Avoid list check from routing rule
        if let Some(routing) = req.routing_config {
            if let Some(rule) = routing.routes.iter().find(|r| r.task_kind == req.task_kind) {
                let candidate_full = format!("{agent}/{model}");
                if rule.avoid.iter().any(|avoid| {
                    avoid == &candidate_full
                        || avoid == &agent
                        || avoid == &model
                        || (avoid.ends_with('*')
                            && candidate_full.starts_with(avoid.trim_end_matches('*')))
                }) {
                    continue;
                }
            }
        }

        // Catalog & Runtime eligibility check (CA-2)
        let eligibility_ctx = CandidateEligibilityContext::from(req);
        if !is_candidate_eligible(&agent, &model, &eligibility_ctx) {
            continue;
        }

        // Certificate check (CA-3)
        let (is_cert, cert_id) = if let Some(store) = req.certificate_store {
            if let Some(cert) = store.lookup(&agent, &model, req.task_kind) {
                if is_certified(cert) {
                    (true, Some(cert.certificate_id.clone()))
                } else {
                    (false, None)
                }
            } else {
                (false, None)
            }
        } else {
            (false, None)
        };

        evaluated_candidates.push(FallbackCandidate {
            agent,
            model,
            is_certified: is_cert,
            certificate_id: cert_id,
            source_rule,
        });
    }

    if evaluated_candidates.is_empty() {
        return None;
    }

    // Sort candidates:
    // 1. Certified candidates for task_kind first (CA-3)
    // 2. Candidates from routing rule before generic catalog candidates
    evaluated_candidates.sort_by(|a, b| {
        if b.is_certified != a.is_certified {
            return b.is_certified.cmp(&a.is_certified);
        }
        let a_is_routing = a
            .source_rule
            .as_deref()
            .is_some_and(|s| s.starts_with("routing:"));
        let b_is_routing = b
            .source_rule
            .as_deref()
            .is_some_and(|s| s.starts_with("routing:"));
        b_is_routing.cmp(&a_is_routing)
    });

    let chosen = evaluated_candidates.remove(0);
    let reason = if chosen.is_certified {
        format!(
            "certified fallback for task_kind '{}' (cert: {})",
            req.task_kind,
            chosen.certificate_id.as_deref().unwrap_or("none")
        )
    } else {
        format!(
            "verified fallback candidate ({})",
            chosen
                .source_rule
                .as_deref()
                .unwrap_or("eligible_candidate")
        )
    };

    Some((chosen, reason))
}

fn parse_route_expr(expr: &str) -> Option<(String, String)> {
    let (agent, model) = expr.split_once('/')?;
    Some((agent.trim().to_string(), model.trim().to_string()))
}

pub fn validate_safe_command(cmd: &str) -> bool {
    !cmd.contains("--dangerously-skip-permissions")
        && !cmd.contains("--skip-permissions")
        && !cmd.contains("--no-permissions")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::{AdapterStatus, AgentDetection};
    use crate::certify::{Certificate, CertificateStatus};
    use crate::certstore::CertificateStore;
    use crate::models::{ModelCandidate, ModelsCatalog};
    use crate::receipt::{
        receipt_sha256, DelegateReceipt, DelegateStatus, DelegateVerdict, ExecReceipt, ExecStatus,
        FallbackAttempt,
    };
    use crate::route::{RouteRule, RoutingConfig};
    use std::collections::BTreeMap;
    use std::fs;

    fn make_test_detection(name: &str, status: AdapterStatus, detected: bool) -> AgentDetection {
        AgentDetection {
            name: name.to_string(),
            binary: name.to_string(),
            detected,
            binary_path: if detected {
                Some(format!("/usr/local/bin/{name}"))
            } else {
                None
            },
            adapter: status,
            secrets_read: false,
        }
    }

    fn make_test_catalog(now_iso: &str) -> ModelsCatalog {
        let mut agents = BTreeMap::new();
        agents.insert(
            "qwen-code".to_string(),
            vec![
                ModelCandidate {
                    id: "qwen3.6-flash".to_string(),
                    source: "catalog".to_string(),
                    confidence: "high".to_string(),
                    notes: "active flash".to_string(),
                    fetched_at: Some(now_iso.to_string()),
                    cost_hint: Some(0.0001),
                    promo: None,
                    status: Some("active".to_string()),
                },
                ModelCandidate {
                    id: "qwen3.8-max".to_string(),
                    source: "catalog".to_string(),
                    confidence: "high".to_string(),
                    notes: "active max".to_string(),
                    fetched_at: Some(now_iso.to_string()),
                    cost_hint: Some(0.005),
                    promo: None,
                    status: Some("active".to_string()),
                },
            ],
        );
        agents.insert(
            "agy".to_string(),
            vec![
                ModelCandidate {
                    id: "gemini-3.7-flash-high".to_string(),
                    source: "catalog".to_string(),
                    confidence: "high".to_string(),
                    notes: "active gemini flash".to_string(),
                    fetched_at: Some(now_iso.to_string()),
                    cost_hint: Some(0.0005),
                    promo: None,
                    status: Some("active".to_string()),
                },
                ModelCandidate {
                    id: "gemini-3.1-pro-high".to_string(),
                    source: "catalog".to_string(),
                    confidence: "high".to_string(),
                    notes: "active gemini pro".to_string(),
                    fetched_at: Some(now_iso.to_string()),
                    cost_hint: Some(0.002),
                    promo: None,
                    status: Some("active".to_string()),
                },
            ],
        );
        agents.insert(
            "claude-code".to_string(),
            vec![ModelCandidate {
                id: "claude-sonnet-5".to_string(),
                source: "catalog".to_string(),
                confidence: "high".to_string(),
                notes: "active claude sonnet".to_string(),
                fetched_at: Some(now_iso.to_string()),
                cost_hint: Some(0.015),
                promo: None,
                status: Some("active".to_string()),
            }],
        );

        ModelsCatalog {
            schema_version: 2,
            agents,
        }
    }

    fn make_test_routing() -> RoutingConfig {
        RoutingConfig {
            schema_version: 1,
            approval_required_model_patterns: vec!["sonnet".to_string(), "opus".to_string()],
            routes: vec![
                RouteRule {
                    task_kind: "architecture".to_string(),
                    default_agent: "agy".to_string(),
                    default_model: "gemini-3.1-pro-high".to_string(),
                    cheap_sufficient: "qwen-code/qwen3.6-flash".to_string(),
                    escalate_to: "claude-code/claude-sonnet-5".to_string(),
                    avoid: vec!["qwen-code/qwen3.8-max".to_string()],
                    rationale: "architecture routing".to_string(),
                },
                RouteRule {
                    task_kind: "coding".to_string(),
                    default_agent: "qwen-code".to_string(),
                    default_model: "qwen3.6-flash".to_string(),
                    cheap_sufficient: "agy/gemini-3.7-flash-high".to_string(),
                    escalate_to: "claude-code/claude-sonnet-5".to_string(),
                    avoid: vec![],
                    rationale: "coding routing".to_string(),
                },
            ],
        }
    }

    fn make_test_cert_store(dir: &std::path::Path) -> CertificateStore {
        fs::create_dir_all(dir).unwrap();
        let receipt = ExecReceipt {
            schema_version: 1,
            correlation_id: "cert-test-receipt".to_string(),
            agent: "qwen-code".to_string(),
            model: "qwen3.6-flash".to_string(),
            command: vec!["qwen".to_string()],
            status: ExecStatus::Succeeded,
            policy_reason: "allowed".to_string(),
            started_at_unix: 1000,
            duration_ms: 200,
            timeout_seconds: 30,
            exit_code: Some(0),
            stdout_tail: "ok".to_string(),
            stderr_tail: "".to_string(),
            secrets_read: false,
            cleanup_attempted: true,
            cleanup_succeeded: true,
            failure_class: None,
            fallback_agent: None,
            fallback_model: None,
            fallback_reason: None,
            fallback_attempts: Vec::new(),
        };

        let cert = Certificate {
            schema_version: 1,
            certificate_id: "cert-12345-qwen-code-qwen3.6-flash-architecture".to_string(),
            created_at_unix: 1000,
            agent: "qwen-code".to_string(),
            model: "qwen3.6-flash".to_string(),
            task_kind: "architecture".to_string(),
            status: CertificateStatus::Certified,
            receipt_sha256: receipt_sha256(&receipt).unwrap(),
            receipt,
            output_path: None,
            secrets_read: false,
        };

        fs::write(
            dir.join("qwen-arch.json"),
            serde_json::to_string(&cert).unwrap(),
        )
        .unwrap();

        CertificateStore::load_dir(dir).unwrap()
    }

    #[test]
    fn failure_classification() {
        // 1. Permission Denied
        assert_eq!(
            classify_failure(
                Some("permission_denied: read_file auto-denied"),
                "",
                Some(0),
                100,
                120
            ),
            Some(FailureClass::PermissionDenied)
        );
        assert_eq!(
            classify_failure(
                None,
                "jetski: no output produced — a tool required the \"read_file\" permission",
                Some(1),
                100,
                120
            ),
            Some(FailureClass::PermissionDenied)
        );

        // 2. Auth Failure
        assert_eq!(
            classify_failure(Some("auth_401"), "", Some(1), 100, 120),
            Some(FailureClass::AuthFailure)
        );
        assert_eq!(
            classify_failure(
                None,
                "HTTP 401 Unauthorized: Invalid API Key",
                Some(1),
                100,
                120
            ),
            Some(FailureClass::AuthFailure)
        );

        // 3. Timeout
        assert_eq!(
            classify_failure(Some("timeout_sin_evidencia"), "", None, 120000, 120),
            Some(FailureClass::Timeout)
        );
        assert_eq!(
            classify_failure(None, "timed out after 120 seconds", None, 120001, 120),
            Some(FailureClass::Timeout)
        );

        // 4. Model Unavailable
        assert_eq!(
            classify_failure(Some("quota exceeded"), "", Some(1), 50, 120),
            Some(FailureClass::ModelUnavailable)
        );
        assert_eq!(
            classify_failure(
                None,
                "Error 429: rate limit reached for model",
                Some(1),
                50,
                120
            ),
            Some(FailureClass::ModelUnavailable)
        );

        // 5. Network Error
        assert_eq!(
            classify_failure(Some("connection refused"), "", Some(1), 50, 120),
            Some(FailureClass::NetworkError)
        );
        assert_eq!(
            classify_failure(
                None,
                "curl: (7) Failed to connect to host",
                Some(7),
                50,
                120
            ),
            Some(FailureClass::NetworkError)
        );

        // 6. Agent Crash
        assert_eq!(
            classify_failure(
                Some("exit_non_zero"),
                "thread 'main' panicked at 'crash'",
                Some(101),
                50,
                120
            ),
            Some(FailureClass::AgentCrash)
        );
    }

    #[test]
    fn fallback_consults_catalog_and_runtime() {
        let now_iso = crate::models::now_iso8601();
        let now_unix = crate::quota::now_unix();
        let mut catalog = make_test_catalog(&now_iso);

        // Add a stale candidate and a down candidate
        catalog
            .agents
            .get_mut("qwen-code")
            .unwrap()
            .push(ModelCandidate {
                id: "qwen-stale".to_string(),
                source: "catalog".to_string(),
                confidence: "high".to_string(),
                notes: "stale".to_string(),
                fetched_at: Some("2020-01-01T00:00:00Z".to_string()),
                cost_hint: None,
                promo: None,
                status: Some("active".to_string()),
            });
        catalog
            .agents
            .get_mut("qwen-code")
            .unwrap()
            .push(ModelCandidate {
                id: "qwen-down".to_string(),
                source: "catalog".to_string(),
                confidence: "high".to_string(),
                notes: "down".to_string(),
                fetched_at: Some(now_iso.clone()),
                cost_hint: None,
                promo: None,
                status: Some("down".to_string()),
            });

        let detected = vec![
            make_test_detection("qwen-code", AdapterStatus::Available, true),
            make_test_detection("agy", AdapterStatus::Available, true),
            make_test_detection("claude-code", AdapterStatus::Gated, true),
            make_test_detection("missing-agent", AdapterStatus::Missing, false),
        ];

        let ctx = CandidateEligibilityContext {
            catalog: &catalog,
            detected: &detected,
            policy_config: None,
            allow_gated: false,
            now_unix,
            catalog_ttl_secs: 86400,
        };

        // Fresh active candidate on available agent -> eligible
        assert!(is_candidate_eligible("qwen-code", "qwen3.6-flash", &ctx,));

        // Stale candidate -> NOT eligible (CA-2 c)
        assert!(!is_candidate_eligible("qwen-code", "qwen-stale", &ctx,));

        // Down/inactive candidate -> NOT eligible (CA-2 b)
        assert!(!is_candidate_eligible("qwen-code", "qwen-down", &ctx,));

        // Gated candidate without allow_gated -> NOT eligible
        assert!(!is_candidate_eligible(
            "claude-code",
            "claude-sonnet-5",
            &ctx,
        ));

        // Missing agent in runtime -> NOT eligible (CA-2 d)
        assert!(!is_candidate_eligible("missing-agent", "any-model", &ctx,));
    }

    #[test]
    fn fallback_selects_certified_candidate() {
        let now_iso = crate::models::now_iso8601();
        let now_unix = crate::quota::now_unix();
        let catalog = make_test_catalog(&now_iso);
        let routing = make_test_routing();
        let detected = vec![
            make_test_detection("qwen-code", AdapterStatus::Available, true),
            make_test_detection("agy", AdapterStatus::Available, true),
            make_test_detection("claude-code", AdapterStatus::Available, true),
        ];

        let temp_dir = tempfile::tempdir().unwrap();
        let cert_store = make_test_cert_store(temp_dir.path());

        // AGY failed with permission_denied for task_kind 'architecture'
        let req = FallbackSelectionRequest {
            task_kind: "architecture",
            failed_agent: "agy",
            failed_model: "gemini-3.1-pro-high",
            failure_class: FailureClass::PermissionDenied,
            catalog: &catalog,
            detected: &detected,
            certificate_store: Some(&cert_store),
            routing_config: Some(&routing),
            policy_config: None,
            allow_gated: false,
            now_unix,
            catalog_ttl_secs: 86400,
        };

        let (chosen, reason) = select_fallback_candidate(&req).expect("fallback candidate found");
        // Certified candidate qwen-code / qwen3.6-flash is chosen!
        assert_eq!(chosen.agent, "qwen-code");
        assert_eq!(chosen.model, "qwen3.6-flash");
        assert!(chosen.is_certified);
        assert!(reason.contains("certified"));
    }

    #[test]
    fn receipt_records_fallback_chain() {
        let mut receipt = DelegateReceipt {
            schema_version: 1,
            correlation_id: "corr-chain-test".to_string(),
            agent: "agy".to_string(),
            model: "gemini-3.1-pro-high".to_string(),
            command: vec!["rtk agy".to_string()],
            status: DelegateStatus::Failed,
            reason: Some("permission_denied: read_file auto-denied".to_string()),
            verdict: DelegateVerdict::NonUtil,
            evidence: "none".to_string(),
            stdout_tail: "".to_string(),
            stderr_tail: "jetski: auto-denied".to_string(),
            started_at_unix: 1000,
            duration_ms: 500,
            timeout_seconds: 60,
            exit_code: Some(0),
            secrets_read: false,
            cleanup_attempted: true,
            cleanup_succeeded: true,
            failure_class: Some(FailureClass::PermissionDenied),
            fallback_agent: Some("qwen-code".to_string()),
            fallback_model: Some("qwen3.6-flash".to_string()),
            fallback_reason: Some("certified fallback for task_kind 'architecture'".to_string()),
            fallback_attempts: vec![
                FallbackAttempt {
                    attempt_number: 1,
                    agent: "agy".to_string(),
                    model: "gemini-3.1-pro-high".to_string(),
                    status: "failed".to_string(),
                    failure_class: Some(FailureClass::PermissionDenied),
                    reason: Some("permission_denied: read_file auto-denied".to_string()),
                    exit_code: Some(0),
                    duration_ms: 500,
                },
                FallbackAttempt {
                    attempt_number: 2,
                    agent: "qwen-code".to_string(),
                    model: "qwen3.6-flash".to_string(),
                    status: "validated".to_string(),
                    failure_class: None,
                    reason: None,
                    exit_code: Some(0),
                    duration_ms: 300,
                },
            ],
        };

        // Update to validated on successful fallback retry
        receipt.status = DelegateStatus::Validated;
        receipt.verdict = DelegateVerdict::Util;

        let json = serde_json::to_string_pretty(&receipt).unwrap();
        assert!(json.contains("\"failure_class\": \"permission_denied\""));
        assert!(json.contains("\"fallback_agent\": \"qwen-code\""));
        assert!(json.contains("\"fallback_model\": \"qwen3.6-flash\""));
        assert!(json.contains("\"fallback_attempts\""));
        assert!(json.contains("\"attempt_number\": 1"));
        assert!(json.contains("\"attempt_number\": 2"));
    }

    #[test]
    fn never_uses_dangerously_skip_permissions() {
        let command_safe = "rtk agy --model gemini-3.7-flash --mode accept-edits";
        let command_unsafe = "rtk agy --model gemini-3.7-flash --dangerously-skip-permissions";

        assert!(validate_safe_command(command_safe));
        assert!(!validate_safe_command(command_unsafe));
    }

    #[test]
    fn fallback_chain_bounded() {
        let now_iso = crate::models::now_iso8601();
        let now_unix = crate::quota::now_unix();
        let catalog = make_test_catalog(&now_iso);
        let routing = make_test_routing();
        let detected = vec![make_test_detection(
            "qwen-code",
            AdapterStatus::Available,
            true,
        )];

        // Attempt 1: qwen-code fails with permission_denied
        let req = FallbackSelectionRequest {
            task_kind: "coding",
            failed_agent: "qwen-code",
            failed_model: "qwen3.6-flash",
            failure_class: FailureClass::PermissionDenied,
            catalog: &catalog,
            detected: &detected,
            certificate_store: None,
            routing_config: Some(&routing),
            policy_config: None,
            allow_gated: false,
            now_unix,
            catalog_ttl_secs: 86400,
        };

        // No other agent detected -> candidate list exhausted -> None (bounded, stops immediately)
        let fallback = select_fallback_candidate(&req);
        assert!(fallback.is_none());
    }
}
