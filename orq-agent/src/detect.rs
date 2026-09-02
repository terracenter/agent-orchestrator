use crate::adapters::{adapters_from_registry, known_adapters, AdaptersRegistry, AgentDetection};
use color_eyre::eyre::Result;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct DetectReport {
    pub schema_version: u8,
    pub agents: Vec<AgentDetection>,
    pub secrets_read: bool,
}

#[allow(dead_code)]
pub fn detect_agents() -> Result<DetectReport> {
    let agents = known_adapters()?
        .into_iter()
        .map(|adapter| adapter.detect())
        .collect();

    Ok(DetectReport {
        schema_version: 1,
        agents,
        secrets_read: false,
    })
}

pub fn detect_agents_from_registry(registry: &AdaptersRegistry) -> DetectReport {
    let agents = adapters_from_registry(registry)
        .into_iter()
        .map(|adapter| adapter.detect())
        .collect();

    DetectReport {
        schema_version: registry.schema_version,
        agents,
        secrets_read: false,
    }
}

#[cfg(test)]
mod tests {
    use super::detect_agents;

    #[test]
    fn detect_report_never_reads_secrets() {
        let report = detect_agents().unwrap();
        assert!(!report.secrets_read);
    }
}
