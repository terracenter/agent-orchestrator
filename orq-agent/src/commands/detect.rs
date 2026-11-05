use color_eyre::eyre::Result;
use serde::Serialize;
use std::path::Path;

use crate::models;
use crate::{adapters, detect};

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub(crate) enum DetectOutput {
    Agents(detect::DetectReport),
    WithModels(detect::DetectModelsReport),
}

pub(crate) struct DetectArgs {
    pub(crate) adapters_config: Option<String>,
    pub(crate) models: bool,
    pub(crate) models_config: Option<String>,
}

pub(crate) async fn run(args: DetectArgs) -> Result<DetectOutput> {
    let adapters_config_path = args.adapters_config.as_deref().map(Path::new);
    let (adapters_registry, _) = adapters::load_registry(adapters_config_path).await?;

    if !args.models {
        return Ok(DetectOutput::Agents(detect::detect_agents_from_registry(
            &adapters_registry,
        )));
    }

    let models_config_path = args.models_config.as_deref().map(Path::new);
    let catalog = match models::load_catalog(models_config_path).await {
        Ok((catalog, _)) => catalog,
        Err(_) => models::ModelsCatalog {
            schema_version: models::CURRENT_SCHEMA_VERSION,
            agents: Default::default(),
        },
    };

    Ok(DetectOutput::WithModels(
        detect::detect_agents_with_models_from_registry(&adapters_registry, &catalog, None),
    ))
}
