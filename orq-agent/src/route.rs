use crate::adapters::AdapterStatus;
use crate::detect;
use crate::policy;
use clap::ValueEnum;
use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    Mechanical,
    Documentation,
    SimpleReview,
    WriteTests,
    Debugging,
    SmallRefactor,
    LargeRefactor,
    MonorepoLongContext,
    Frontend,
    Database,
    SysadminLinux,
    SimpleCybersecurity,
    DeepCybersecurity,
    Architecture,
    DeepReasoning,
    RealToolExecution,
    ReadonlyAnalysis,
}

#[derive(Clone, Debug, Serialize)]
pub struct RouteDecision {
    pub schema_version: u8,
    pub task_kind: TaskKind,
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
    pub rationale: String,
}

#[derive(Clone, Copy, Debug)]
struct RouteRule {
    default_agent: &'static str,
    default_model: &'static str,
    cheap_sufficient: &'static str,
    escalate_to: &'static str,
    avoid: &'static [&'static str],
    rationale: &'static str,
}

pub fn decide(task_kind: TaskKind, allow_gated: bool) -> RouteDecision {
    let rule = rule_for(task_kind);
    let detected = detect::detect_agents();
    let selected = select_route(rule, &detected.agents, allow_gated);

    RouteDecision {
        schema_version: 1,
        task_kind,
        selected_agent: selected.agent.to_string(),
        selected_model: selected.model.to_string(),
        selected_policy_reason: selected.policy_reason,
        default_agent: rule.default_agent.to_string(),
        default_model: rule.default_model.to_string(),
        cheap_sufficient: rule.cheap_sufficient.to_string(),
        escalate_to: rule.escalate_to.to_string(),
        avoid: rule.avoid.iter().map(|item| (*item).to_string()).collect(),
        fallback_applied: selected.fallback_applied,
        requires_confirmation: selected.requires_confirmation
            || default_requires_confirmation(rule),
        secrets_read: detected.secrets_read,
        rationale: rule.rationale.to_string(),
    }
}

struct SelectedRoute<'a> {
    agent: &'a str,
    model: &'a str,
    fallback_applied: bool,
    requires_confirmation: bool,
    policy_reason: String,
}

fn select_route<'a>(
    rule: RouteRule,
    detected: &[crate::adapters::AgentDetection],
    allow_gated: bool,
) -> SelectedRoute<'a> {
    let candidates = [
        (rule.default_agent, rule.default_model, false),
        split_route(rule.cheap_sufficient, true),
        split_route(rule.escalate_to, true),
    ];

    for (agent, model, fallback_applied) in candidates {
        if agent == "none" || agent == "cualquier modelo con contexto <300K" {
            continue;
        }
        if let Some(status) = detected_status(detected, agent) {
            let policy = policy::evaluate(agent, model, status, allow_gated);
            if policy.allowed {
                return SelectedRoute {
                    agent,
                    model,
                    fallback_applied,
                    requires_confirmation: requires_confirmation(status, model),
                    policy_reason: policy.reason,
                };
            }
        }
    }

    SelectedRoute {
        agent: rule.default_agent,
        model: rule.default_model,
        fallback_applied: false,
        requires_confirmation: true,
        policy_reason: "no detected allowed route; returning default for explicit human review"
            .to_string(),
    }
}

fn detected_status(
    detected: &[crate::adapters::AgentDetection],
    agent: &str,
) -> Option<AdapterStatus> {
    detected
        .iter()
        .find(|item| item.name == agent && item.detected)
        .map(|item| item.adapter)
}

fn requires_confirmation(status: AdapterStatus, model: &str) -> bool {
    matches!(status, AdapterStatus::Gated)
        || model.to_ascii_lowercase().contains("sonnet")
        || model.to_ascii_lowercase().contains("opus")
}

fn default_requires_confirmation(rule: RouteRule) -> bool {
    rule.default_model.to_ascii_lowercase().contains("sonnet")
        || rule.default_model.to_ascii_lowercase().contains("opus")
}

fn split_route(route: &'static str, fallback_applied: bool) -> (&'static str, &'static str, bool) {
    route
        .split_once('/')
        .map(|(agent, model)| (agent, model, fallback_applied))
        .unwrap_or(("none", route, fallback_applied))
}

fn rule_for(task_kind: TaskKind) -> RouteRule {
    match task_kind {
        TaskKind::Mechanical => RouteRule {
            default_agent: "claude-code",
            default_model: "claude-haiku-4-5",
            cheap_sufficient: "qwen-code/qwen3.6-flash",
            escalate_to: "agy/gemini-3.6-flash-medium",
            avoid: &[
                "claude-code/claude-opus-5",
                "claude-code/claude-fable-5",
                "qwen-code/qwen3.8-max",
            ],
            rationale: "Deterministic repetitive work should use the cheapest sufficient route.",
        },
        TaskKind::Documentation => RouteRule {
            default_agent: "qwen-code",
            default_model: "qwen3.6-flash",
            cheap_sufficient: "agy/gemini-3.5-flash-low",
            escalate_to: "claude-code/claude-sonnet-5",
            avoid: &["claude-code/claude-fable-5", "claude-code/claude-opus-5"],
            rationale: "Long vault notes fit Qwen/AGY context without spending Claude quota.",
        },
        TaskKind::SimpleReview => RouteRule {
            default_agent: "agy",
            default_model: "gemini-3.6-flash-medium",
            cheap_sufficient: "qwen-code/qwen3.6-flash",
            escalate_to: "claude-code/claude-sonnet-5",
            avoid: &["claude-code/claude-opus-5", "qwen-code/qwen3.8-max"],
            rationale: "Diff and lint review rarely requires deep reasoning.",
        },
        TaskKind::WriteTests => RouteRule {
            default_agent: "qwen-code",
            default_model: "qwen3-coder-plus",
            cheap_sufficient: "claude-code/claude-haiku-4-5",
            escalate_to: "agy/gemini-3.7-flash-high",
            avoid: &["claude-code/claude-opus-5", "claude-code/claude-fable-5"],
            rationale: "Large context helps generate complete suites.",
        },
        TaskKind::Debugging => RouteRule {
            default_agent: "agy",
            default_model: "gemini-3.7-flash-high",
            cheap_sufficient: "qwen-code/qwen3.6-flash",
            escalate_to: "claude-code/claude-sonnet-5",
            avoid: &["qwen-code/qwen3-coder-next"],
            rationale: "Use thinking plus real execution for root-cause isolation.",
        },
        TaskKind::SmallRefactor => RouteRule {
            default_agent: "agy",
            default_model: "gemini-3.6-flash-high",
            cheap_sufficient: "qwen-code/qwen3.6-flash",
            escalate_to: "claude-code/claude-sonnet-5",
            avoid: &["claude-code/claude-opus-5"],
            rationale: "Atomic edits with verification do not require top-tier models.",
        },
        TaskKind::LargeRefactor => RouteRule {
            default_agent: "claude-code",
            default_model: "claude-sonnet-5",
            cheap_sufficient: "agy/gemini-3.1-pro-high",
            escalate_to: "claude-code/claude-opus-5",
            avoid: &["qwen-code/qwen3-coder-next", "qwen-code/glm-4.7"],
            rationale: "Large multi-file invariants need engineering-grade review.",
        },
        TaskKind::MonorepoLongContext => RouteRule {
            default_agent: "qwen-code",
            default_model: "qwen3-coder-plus",
            cheap_sufficient: "agy/gemini-3.7-flash-medium",
            escalate_to: "qwen-code/qwen3.8-max",
            avoid: &["context_lt_300k"],
            rationale: "1M context avoids truncating large repositories.",
        },
        TaskKind::Frontend => RouteRule {
            default_agent: "qwen-code",
            default_model: "qwen3.6-plus",
            cheap_sufficient: "agy/gemini-3.5-flash-medium",
            escalate_to: "claude-code/claude-sonnet-5",
            avoid: &["qwen-code/qwen3-coder-plus"],
            rationale: "Frontend may need multimodal/mockup understanding.",
        },
        TaskKind::Database => RouteRule {
            default_agent: "qwen-code",
            default_model: "qwen3-coder-plus",
            cheap_sufficient: "agy/gemini-3.6-flash-medium",
            escalate_to: "qwen-code/qwen3.8-max",
            avoid: &["qwen-code/glm-5"],
            rationale: "Large context can include schema and queries together.",
        },
        TaskKind::SysadminLinux => RouteRule {
            default_agent: "agy",
            default_model: "gemini-3.7-flash-high",
            cheap_sufficient: "qwen-code/qwen3.6-flash",
            escalate_to: "claude-code/claude-sonnet-5",
            avoid: &["agents_without_real_tools"],
            rationale: "Sysadmin tasks require real tools and strict sudo policy.",
        },
        TaskKind::SimpleCybersecurity => RouteRule {
            default_agent: "qwen-code",
            default_model: "qwen3.6-flash",
            cheap_sufficient: "agy/gemini-3.5-flash-medium",
            escalate_to: "agy/gemini-3.7-flash-high",
            avoid: &["qwen-code/qwen3.8-max"],
            rationale: "Routine dependency/lint security checks do not need flagship models.",
        },
        TaskKind::DeepCybersecurity => RouteRule {
            default_agent: "qwen-code",
            default_model: "qwen3.8-max",
            cheap_sufficient: "agy/gemini-3.1-pro-high",
            escalate_to: "claude-code/claude-opus-5",
            avoid: &["models_without_thinking_or_short_context"],
            rationale: "Deep threat analysis needs formal reasoning and long context.",
        },
        TaskKind::Architecture => RouteRule {
            default_agent: "claude-code",
            default_model: "claude-opus-5",
            cheap_sufficient: "agy/gemini-3.1-pro-high",
            escalate_to: "none",
            avoid: &["gemini-3.5-flash-*", "qwen-code/qwen3-coder-next"],
            rationale: "Architecture uses scarce top-tier models only with explicit evidence.",
        },
        TaskKind::DeepReasoning => RouteRule {
            default_agent: "agy",
            default_model: "gemini-3.1-pro-high",
            cheap_sufficient: "qwen-code/qwen3.8-max",
            escalate_to: "claude-code/claude-opus-5",
            avoid: &["qwen-code/qwen3-coder-plus"],
            rationale: "Use shared AGY quota before scarce Claude quota.",
        },
        TaskKind::RealToolExecution => RouteRule {
            default_agent: "agy",
            default_model: "gemini-3.6-flash-medium",
            cheap_sufficient: "claude-code/claude-haiku-4-5",
            escalate_to: "claude-code/claude-sonnet-5",
            avoid: &["pi", "readonly_runners"],
            rationale: "Real execution requires tool-capable agents and receipts.",
        },
        TaskKind::ReadonlyAnalysis => RouteRule {
            default_agent: "qwen-code",
            default_model: "qwen3.6-flash",
            cheap_sufficient: "agy/gemini-3.5-flash-low",
            escalate_to: "qwen-code/glm-5",
            avoid: &["claude-code/claude-opus-5", "claude-code/claude-fable-5"],
            rationale: "Passive analysis should not spend expensive quotas.",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{decide, rule_for, TaskKind};

    #[test]
    fn documentation_defaults_to_qwen_flash() {
        let rule = rule_for(TaskKind::Documentation);
        assert_eq!(rule.default_agent, "qwen-code");
        assert_eq!(rule.default_model, "qwen3.6-flash");
    }

    #[test]
    fn deep_security_reserves_qwen_max() {
        let rule = rule_for(TaskKind::DeepCybersecurity);
        assert_eq!(rule.default_model, "qwen3.8-max");
        assert!(rule.rationale.contains("Deep threat"));
    }

    #[test]
    fn architecture_requires_confirmation_without_gated_approval() {
        let decision = decide(TaskKind::Architecture, false);
        assert!(decision.requires_confirmation);
        assert_eq!(decision.secrets_read, false);
    }
}
