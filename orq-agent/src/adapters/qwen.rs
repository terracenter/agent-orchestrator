use super::AgentAdapter;

pub struct QwenAdapter;

impl AgentAdapter for QwenAdapter {
    fn name(&self) -> &'static str {
        "qwen-code"
    }

    fn binary(&self) -> &'static str {
        "qwen"
    }

    fn build_argv(&self, model: &str, task: &str) -> Vec<String> {
        vec![
            "--safe-mode".to_string(),
            "-m".to_string(),
            model.to_string(),
            "-p".to_string(),
            task.to_string(),
            "--output-format".to_string(),
            "text".to_string(),
        ]
    }
}
