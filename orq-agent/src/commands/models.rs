use color_eyre::eyre::Result;

use crate::{adapters, models as domain_models};

pub(crate) struct ModelsArgs {
    pub(crate) agent: String,
    pub(crate) config: Option<String>,
    pub(crate) adapters_config: Option<String>,
}

pub(crate) async fn run(args: ModelsArgs) -> Result<domain_models::ModelsReport> {
    let config_path = args.config.as_deref().map(std::path::Path::new);
    let (catalog, config_source) = domain_models::load_catalog(config_path).await?;
    let adapters_config_path = args.adapters_config.as_deref().map(std::path::Path::new);
    let (adapters_registry, _) = adapters::load_registry(adapters_config_path).await?;
    domain_models::list(&args.agent, &catalog, &adapters_registry, &config_source)
}
