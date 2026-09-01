use super::{AdapterStatus, AgentAdapter};

pub struct ClaudeCodeAdapter;

impl AgentAdapter for ClaudeCodeAdapter {
    fn name(&self) -> &'static str {
        "claude-code"
    }

    fn binary(&self) -> &'static str {
        "claude"
    }

    fn status(&self) -> AdapterStatus {
        AdapterStatus::Gated
    }

    fn build_argv(&self, model: &str, task: &str) -> Vec<String> {
        vec![
            "--model".to_string(),
            model.to_string(),
            "--print".to_string(),
            "--".to_string(),
            task.to_string(),
        ]
    }
}
