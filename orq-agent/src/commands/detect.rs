use color_eyre::eyre::Result;

use crate::{adapters, detect};

pub(crate) struct DetectArgs {
    pub(crate) adapters_config: Option<String>,
}

pub(crate) async fn run(args: DetectArgs) -> Result<detect::DetectReport> {
    let adapters_config_path = args.adapters_config.as_deref().map(std::path::Path::new);
    let (adapters_registry, _) = adapters::load_registry(adapters_config_path).await?;
    Ok(detect::detect_agents_from_registry(&adapters_registry))
}
