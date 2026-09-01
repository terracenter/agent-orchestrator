use crate::adapters::AdapterStatus;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct PolicyDecision {
    pub allowed: bool,
    pub reason: String,
}

pub fn evaluate(
    agent: &str,
    model: &str,
    status: AdapterStatus,
    allow_gated: bool,
) -> PolicyDecision {
    if matches!(status, AdapterStatus::DeprecatedOrQuarantine) {
        return deny(format!("agent {agent} is deprecated_or_quarantine"));
    }

    if matches!(status, AdapterStatus::Gated) && !allow_gated {
        return deny(format!(
            "agent {agent} is gated; pass --allow-gated after human approval"
        ));
    }

    let model_lc = model.to_ascii_lowercase();
    if (model_lc.contains("sonnet") || model_lc.contains("opus")) && !allow_gated {
        return deny(format!("model {model} requires explicit human approval"));
    }

    PolicyDecision {
        allowed: true,
        reason: "allowed".to_string(),
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
    use super::evaluate;
    use crate::adapters::AdapterStatus;

    #[test]
    fn blocks_gated_without_approval() {
        let decision = evaluate("claude-code", "haiku", AdapterStatus::Gated, false);
        assert!(!decision.allowed);
    }

    #[test]
    fn blocks_sonnet_without_approval() {
        let decision = evaluate("pi", "claude-sonnet-4", AdapterStatus::Available, false);
        assert!(!decision.allowed);
    }
}
