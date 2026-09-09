use color_eyre::eyre::{Result, WrapErr};
use serde::{Deserialize, Serialize};

/// Bundled static agent profiles catalog, mirrored from `config/agent-profiles.json`
/// (the same file the legacy Go `orq agents --format json` used via `agentpkg.LoadProfiles`).
pub const BUILTIN_AGENT_PROFILES_JSON: &str = include_str!("../config/agent-profiles.json");

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AgentProfile {
    pub agent: String,
    pub provider: String,
    pub model: String,
    pub cost_level: i64,
    pub use_for: String,
    pub review_only: bool,
    pub verified: bool,
}

pub fn default_profiles() -> Result<Vec<AgentProfile>> {
    serde_json::from_str(BUILTIN_AGENT_PROFILES_JSON)
        .wrap_err("parsing bundled agent profiles json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_profiles_parse_and_contain_known_agent() {
        let profiles = default_profiles().unwrap();
        assert!(!profiles.is_empty());
        assert!(profiles.iter().any(|p| p.agent == "pi"));
    }
}
