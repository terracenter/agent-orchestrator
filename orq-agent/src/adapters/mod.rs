use serde::Serialize;

mod agy;
mod claude_code;
mod hermes;
mod openclaw;
mod pi;
mod qwen;

pub trait AgentAdapter: Send + Sync {
    fn name(&self) -> &'static str;
    fn binary(&self) -> &'static str;
    fn status(&self) -> AdapterStatus {
        AdapterStatus::Available
    }

    fn build_argv(&self, model: &str, task: &str) -> Vec<String>;

    fn binary_path(&self) -> Option<String> {
        let env_name = format!(
            "ORQ_AGENT_BIN_{}",
            self.name().replace('-', "_").to_ascii_uppercase()
        );
        std::env::var(env_name).ok().or_else(|| {
            which::which(self.binary())
                .ok()
                .map(|p| p.display().to_string())
        })
    }

    fn detect(&self) -> AgentDetection {
        let binary_path = self.binary_path();
        AgentDetection {
            name: self.name(),
            binary: self.binary(),
            detected: binary_path.is_some(),
            binary_path,
            adapter: self.status(),
            secrets_read: false,
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterStatus {
    Available,
    Missing,
    DeprecatedOrQuarantine,
    Gated,
}

#[derive(Debug, Serialize)]
pub struct AgentDetection {
    pub name: &'static str,
    pub binary: &'static str,
    pub detected: bool,
    pub binary_path: Option<String>,
    pub adapter: AdapterStatus,
    pub secrets_read: bool,
}

pub fn known_adapters() -> Vec<Box<dyn AgentAdapter>> {
    vec![
        Box::new(pi::PiAdapter),
        Box::new(openclaw::OpenClawAdapter),
        Box::new(agy::AgyAdapter),
        Box::new(qwen::QwenAdapter),
        Box::new(hermes::HermesAdapter),
        Box::new(claude_code::ClaudeCodeAdapter),
    ]
}

pub fn find_adapter(name: &str) -> Option<Box<dyn AgentAdapter>> {
    known_adapters()
        .into_iter()
        .find(|adapter| adapter.name() == name)
}
