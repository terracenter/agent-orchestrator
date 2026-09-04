use color_eyre::eyre::Result;

use crate::{adapters, models as domain_models};

pub(crate) struct ModelsArgs {
    pub(crate) agent: String,
    pub(crate) config: Option<String>,
    pub(crate) adapters_config: Option<String>,
}

pub(crate) struct ModelsRefreshArgs {
    pub(crate) feed: Option<String>,
    pub(crate) catalog: Option<String>,
}

pub(crate) async fn run(args: ModelsArgs) -> Result<domain_models::ModelsReport> {
    let config_path = args.config.as_deref().map(std::path::Path::new);
    let (catalog, config_source) = domain_models::load_catalog(config_path).await?;
    let adapters_config_path = args.adapters_config.as_deref().map(std::path::Path::new);
    let (adapters_registry, _) = adapters::load_registry(adapters_config_path).await?;
    domain_models::list(&args.agent, &catalog, &adapters_registry, &config_source)
}

pub(crate) async fn run_refresh(
    args: ModelsRefreshArgs,
) -> Result<domain_models::ModelsRefreshSummary> {
    let feed_path = args.feed.as_deref().map(std::path::Path::new);
    let (feed, feed_source) = domain_models::load_market_feed(feed_path).await?;

    let catalog_path_opt = args.catalog.as_deref().map(std::path::Path::new);
    let (mut catalog, catalog_path) = domain_models::load_catalog(catalog_path_opt).await?;

    let now_iso = domain_models::now_iso8601();
    let mut summary =
        domain_models::merge_feed_into_catalog(&mut catalog, &feed, &feed_source, &now_iso);
    summary.catalog_path = catalog_path.clone();

    // Persist updated catalog back to file
    let json_bytes = serde_json::to_string_pretty(&catalog)?;
    tokio::fs::write(&catalog_path, json_bytes).await?;

    Ok(summary)
}
