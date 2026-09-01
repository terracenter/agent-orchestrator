use super::{AdapterStatus, AgentAdapter};

pub struct HermesAdapter;

impl AgentAdapter for HermesAdapter {
    fn name(&self) -> &'static str {
        "hermes"
    }

    fn binary(&self) -> &'static str {
        "hermes"
    }

    fn status(&self) -> AdapterStatus {
        AdapterStatus::DeprecatedOrQuarantine
    }

    fn build_argv(&self, model: &str, task: &str) -> Vec<String> {
        vec![
            "--model".to_string(),
            model.to_string(),
            "--prompt".to_string(),
            task.to_string(),
        ]
    }
}
