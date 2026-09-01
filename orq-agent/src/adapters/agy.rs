use super::AgentAdapter;

pub struct AgyAdapter;

impl AgentAdapter for AgyAdapter {
    fn name(&self) -> &'static str {
        "agy"
    }

    fn binary(&self) -> &'static str {
        "agy"
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
