use super::AgentAdapter;

pub struct OpenClawAdapter;

impl AgentAdapter for OpenClawAdapter {
    fn name(&self) -> &'static str {
        "openclaw"
    }

    fn binary(&self) -> &'static str {
        "openclaw"
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
