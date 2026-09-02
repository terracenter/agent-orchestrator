use color_eyre::eyre::Result;

use crate::{adapters, certstore, detect, route as domain_route, state};

pub(crate) struct RouteArgs {
    pub(crate) task_kind: String,
    pub(crate) config: Option<String>,
    pub(crate) allow_gated: bool,
    pub(crate) adapters_config: Option<String>,
    pub(crate) cert_dir: Option<String>,
    pub(crate) db_path: Option<String>,
}

pub(crate) async fn run(args: RouteArgs) -> Result<domain_route::RouteDecision> {
    let config_path = args.config.as_deref().map(std::path::Path::new);
    let (routing_config, config_source) = domain_route::load_config(config_path).await?;
    let adapters_config_path = args.adapters_config.as_deref().map(std::path::Path::new);
    let (adapters_registry, _) = adapters::load_registry(adapters_config_path).await?;
    let cert_store = match args.cert_dir.as_deref().map(std::path::Path::new) {
        Some(path) => Some(certstore::CertificateStore::load_dir(path)?),
        None => None,
    };
    let state_store = match args.db_path.as_deref().map(std::path::Path::new) {
        Some(path) => Some(state::open(Some(path))?),
        None => state::open(None).ok(),
    };
    let detected = detect::detect_agents_from_registry(&adapters_registry);
    domain_route::decide_with_detected(
        &routing_config,
        &args.task_kind,
        args.allow_gated,
        &config_source,
        &detected,
        cert_store.as_ref(),
        state_store.as_ref(),
    )
}
