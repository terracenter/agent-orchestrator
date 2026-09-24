use color_eyre::eyre::{Result, WrapErr};
use serde::{Deserialize, Serialize};

pub const AGENT_PROFILES_ENV: &str = "ORQ_AGENT_PROFILES";
pub const AGENT_PROFILES_FILE: &str = "agent-profiles.json";

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
    let path = crate::config::resolve_path(None, AGENT_PROFILES_ENV, AGENT_PROFILES_FILE)?;
    let content = std::fs::read_to_string(&path)
        .wrap_err_with(|| format!("reading external agent profiles {}", path.display()))?;
    serde_json::from_str(&content).wrap_err("parsing external agent profiles json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_profiles_parse_and_contain_known_agent() {
        let profiles = default_profiles().unwrap();
        assert!(!profiles.is_empty());
        assert!(profiles.iter().any(|p| p.agent == "pi"));
    }
}
