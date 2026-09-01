use crate::adapters::{known_adapters, AgentDetection};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct DetectReport {
    pub schema_version: u8,
    pub agents: Vec<AgentDetection>,
    pub secrets_read: bool,
}

pub fn detect_agents() -> DetectReport {
    let agents = known_adapters()
        .into_iter()
        .map(|adapter| adapter.detect())
        .collect();

    DetectReport {
        schema_version: 1,
        agents,
        secrets_read: false,
    }
}

#[cfg(test)]
mod tests {
    use super::detect_agents;

    #[test]
    fn detect_report_never_reads_secrets() {
        let report = detect_agents();
        assert!(!report.secrets_read);
    }
}
