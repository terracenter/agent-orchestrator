use color_eyre::eyre::{eyre, Result, WrapErr};
use std::path::{Path, PathBuf};

/// Resolve a live JSON catalog without putting its contents in the executable.
/// Explicit paths and environment variables are authoritative; otherwise the
/// user-level locations are preferred over repository development paths.
pub fn resolve_path(explicit: Option<&Path>, env_var: &str, file_name: &str) -> Result<PathBuf> {
    if let Some(path) = explicit {
        return Ok(path.to_path_buf());
    }
    if let Some(path) = non_empty_env_path(env_var) {
        return Ok(path);
    }

    let mut candidates = Vec::new();
    if let Some(config_dir) = non_empty_env_path("ORQ_CONFIG_DIR") {
        candidates.push(config_dir.join(file_name));
    }
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        candidates.push(home.join("Workspace/.agents/orq").join(file_name));
        candidates.push(home.join(".config/orq").join(file_name));
    }
    candidates.push(PathBuf::from("config").join(file_name));
    candidates.push(PathBuf::from("orq-agent/config").join(file_name));
    candidates.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("config")
            .join(file_name),
    );

    if let Some(path) = candidates.iter().find(|path| path.is_file()) {
        return Ok(path.clone());
    }

    let searched = candidates
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    Err(eyre!(
        "missing external ORQ config {file_name}; set {env_var} or ORQ_CONFIG_DIR, or create one of: {searched}"
    ))
}

fn non_empty_env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

pub async fn read_json(
    explicit: Option<&Path>,
    env_var: &str,
    file_name: &str,
    label: &str,
) -> Result<(String, String)> {
    let path = resolve_path(explicit, env_var, file_name)?;
    let content = tokio::fs::read_to_string(&path)
        .await
        .wrap_err_with(|| format!("reading external {label} {}", path.display()))?;
    Ok((content, path.display().to_string()))
}
