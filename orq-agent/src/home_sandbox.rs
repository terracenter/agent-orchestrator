use crate::receipt::now_unix_nanos;
use color_eyre::eyre::{eyre, Result, WrapErr};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const SUPPORTED_SCHEMA_VERSION: u8 = 1;
const DEFAULT_HOME_CAPABILITIES_PATH: &str = "config/home-capabilities.json";
const HOME_CAPABILITIES_ENV: &str = "ORQ_HOME_CAPABILITIES";
/// Overrides where ephemeral sandbox HOME directories are created. Defaults to the OS temp dir.
const SANDBOX_ROOT_ENV: &str = "ORQ_SANDBOX_HOME_ROOT";

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct HomeCapabilitiesConfig {
    pub schema_version: u8,
    pub adapters: HashMap<String, AdapterHomeCapability>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct AdapterHomeCapability {
    #[serde(default)]
    pub home_paths: Vec<String>,
}

#[allow(dead_code)]
pub fn default_config() -> Result<HomeCapabilitiesConfig> {
    let path = default_config_path(HOME_CAPABILITIES_ENV, DEFAULT_HOME_CAPABILITIES_PATH);
    let content = std::fs::read_to_string(&path)
        .wrap_err_with(|| format!("reading home capabilities config {}", path.display()))?;
    parse_config(&content)
}

pub async fn load_config(path: Option<&Path>) -> Result<(HomeCapabilitiesConfig, String)> {
    let path_buf;
    let path = match path {
        Some(path) => path,
        None => {
            path_buf = default_config_path(HOME_CAPABILITIES_ENV, DEFAULT_HOME_CAPABILITIES_PATH);
            path_buf.as_path()
        }
    };
    let content = tokio::fs::read_to_string(path)
        .await
        .wrap_err_with(|| format!("reading home capabilities config {}", path.display()))?;
    Ok((parse_config(&content)?, path.display().to_string()))
}

fn default_config_path(env_name: &str, relative_path: &str) -> PathBuf {
    std::env::var_os(env_name)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative_path))
}

pub fn parse_config(content: &str) -> Result<HomeCapabilitiesConfig> {
    let config: HomeCapabilitiesConfig =
        serde_json::from_str(content).wrap_err("parsing home capabilities config json")?;
    validate_config(&config)?;
    Ok(config)
}

fn validate_config(config: &HomeCapabilitiesConfig) -> Result<()> {
    if config.schema_version != SUPPORTED_SCHEMA_VERSION {
        return Err(eyre!(
            "unsupported home capabilities schema_version {}; expected {}",
            config.schema_version,
            SUPPORTED_SCHEMA_VERSION
        ));
    }
    for (adapter, capability) in &config.adapters {
        if adapter.trim().is_empty() {
            return Err(eyre!("home capabilities adapter key cannot be empty"));
        }
        for relative in &capability.home_paths {
            if let Err(error) = validate_relative_path(relative) {
                return Err(eyre!(
                    "adapter {adapter} home_paths entry {relative}: {error}"
                ));
            }
        }
    }
    Ok(())
}

fn validate_relative_path(relative: &str) -> Result<()> {
    if relative.trim().is_empty() {
        return Err(eyre!("home capability path cannot be empty"));
    }
    let path = Path::new(relative);
    if path.is_absolute() {
        return Err(eyre!("home capability path must be relative to HOME"));
    }
    for component in path.components() {
        if !matches!(component, Component::Normal(_)) {
            return Err(eyre!(
                "home capability path contains an unsafe path component"
            ));
        }
    }
    Ok(())
}

/// Real, ephemeral HOME directory prepared for a single adapter invocation. Only the paths
/// declared for that adapter in `HomeCapabilitiesConfig` are copied in. Always remove it with
/// `cleanup()` on the success path; `Drop` removes it as a last-resort fallback (error/panic).
#[derive(Debug)]
pub struct SandboxHome {
    path: PathBuf,
    cleaned_up: bool,
}

impl SandboxHome {
    /// Fails closed: an adapter absent from `config.adapters` refuses to run rather than
    /// silently exposing the full real HOME or an unconfigured empty one.
    pub fn prepare(
        adapter_name: &str,
        config: &HomeCapabilitiesConfig,
        real_home: &Path,
        sandbox_root: &Path,
    ) -> Result<Self> {
        let capability = config.adapters.get(adapter_name).ok_or_else(|| {
            eyre!(
                "no home capability declared for adapter '{adapter_name}'; refusing to expose HOME"
            )
        })?;

        let path = create_unique_dir(sandbox_root, adapter_name)?;
        let sandbox = Self {
            path,
            cleaned_up: false,
        };

        for relative in &capability.home_paths {
            if let Err(error) = copy_into_sandbox(real_home, &sandbox.path, relative) {
                return Err(eyre!(
                    "preparing sandbox HOME for adapter '{adapter_name}', path '{relative}': {error}"
                ));
            }
        }

        Ok(sandbox)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Removes the sandbox directory tree. Returns whether removal succeeded. Safe to call
    /// exactly once; `Drop` will no-op afterwards.
    pub async fn cleanup(mut self) -> bool {
        let path = self.path.clone();
        self.cleaned_up = true;
        tokio::task::spawn_blocking(move || std::fs::remove_dir_all(&path).is_ok())
            .await
            .unwrap_or(false)
    }
}

impl Drop for SandboxHome {
    fn drop(&mut self) {
        if !self.cleaned_up {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}

pub fn sandbox_root() -> PathBuf {
    std::env::var_os(SANDBOX_ROOT_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("orq-agent-sandboxes")
}

fn unique_suffix() -> u64 {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    COUNTER.fetch_add(1, Ordering::Relaxed)
}

#[cfg(unix)]
fn create_unique_dir(base: &Path, adapter_name: &str) -> Result<PathBuf> {
    use std::os::unix::fs::DirBuilderExt;

    std::fs::create_dir_all(base)
        .wrap_err_with(|| format!("creating sandbox home root {}", base.display()))?;

    let safe_adapter: String = adapter_name
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect();

    for _ in 0..64 {
        let candidate = base.join(format!(
            "orq-home-{safe_adapter}-{}-{}",
            now_unix_nanos(),
            unique_suffix()
        ));
        let mut builder = std::fs::DirBuilder::new();
        builder.mode(0o700);
        match builder.create(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(error)
                    .wrap_err_with(|| format!("creating sandbox home {}", candidate.display()))
            }
        }
    }
    Err(eyre!(
        "could not allocate a unique sandbox home directory under {}",
        base.display()
    ))
}

#[cfg(not(unix))]
fn create_unique_dir(base: &Path, adapter_name: &str) -> Result<PathBuf> {
    std::fs::create_dir_all(base)
        .wrap_err_with(|| format!("creating sandbox home root {}", base.display()))?;
    let safe_adapter: String = adapter_name
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect();
    for _ in 0..64 {
        let candidate = base.join(format!(
            "orq-home-{safe_adapter}-{}-{}",
            now_unix_nanos(),
            unique_suffix()
        ));
        match std::fs::create_dir(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(error)
                    .wrap_err_with(|| format!("creating sandbox home {}", candidate.display()))
            }
        }
    }
    Err(eyre!(
        "could not allocate a unique sandbox home directory under {}",
        base.display()
    ))
}

/// Copies one declared relative path from `real_home` into `sandbox_home`. Missing source
/// paths are skipped (a not-yet-authenticated adapter still runs and fails with its own,
/// clearer error). Symlinks and any path escaping `real_home` are refused outright.
fn copy_into_sandbox(real_home: &Path, sandbox_home: &Path, relative: &str) -> Result<()> {
    validate_relative_path(relative)?;
    let rel_path = Path::new(relative);
    let source = real_home.join(rel_path);

    let metadata = match std::fs::symlink_metadata(&source) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(error).wrap_err_with(|| format!("reading metadata for {relative}"))
        }
    };
    if metadata.file_type().is_symlink() {
        return Err(eyre!(
            "home capability path '{relative}' resolves to a symlink; refusing to copy"
        ));
    }

    let canonical_home = real_home
        .canonicalize()
        .wrap_err_with(|| format!("canonicalizing real HOME {}", real_home.display()))?;
    let canonical_source = source
        .canonicalize()
        .wrap_err_with(|| format!("canonicalizing home capability path '{relative}'"))?;
    if !canonical_source.starts_with(&canonical_home) {
        return Err(eyre!(
            "home capability path '{relative}' escapes the real HOME directory"
        ));
    }

    let dest = sandbox_home.join(rel_path);
    if let Some(parent) = dest.parent() {
        create_dir_all_restricted(parent)?;
    }

    if metadata.is_dir() {
        copy_dir_recursive(&source, &dest)
    } else {
        std::fs::copy(&source, &dest)
            .wrap_err_with(|| format!("copying home capability path '{relative}'"))?;
        set_private_file_permissions(&dest)?;
        Ok(())
    }
}

fn copy_dir_recursive(source: &Path, dest: &Path) -> Result<()> {
    create_dir_all_restricted(dest)?;
    for entry in std::fs::read_dir(source)
        .wrap_err_with(|| format!("reading directory {}", source.display()))?
    {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            return Err(eyre!(
                "refusing to copy symlink {} while preparing sandbox HOME",
                entry.path().display()
            ));
        }
        let dest_entry = dest.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_recursive(&entry.path(), &dest_entry)?;
        } else {
            std::fs::copy(entry.path(), &dest_entry)
                .wrap_err_with(|| format!("copying {}", entry.path().display()))?;
            set_private_file_permissions(&dest_entry)?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn create_dir_all_restricted(dir: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(dir).wrap_err_with(|| format!("creating {}", dir.display()))?;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
        .wrap_err_with(|| format!("setting permissions on {}", dir.display()))
}

#[cfg(not(unix))]
fn create_dir_all_restricted(dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir).wrap_err_with(|| format!("creating {}", dir.display()))
}

#[cfg(unix)]
fn set_private_file_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .wrap_err_with(|| format!("setting permissions on {}", path.display()))
}

#[cfg(not(unix))]
fn set_private_file_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    fn config_with(adapter: &str, home_paths: &[&str]) -> HomeCapabilitiesConfig {
        let mut adapters = HashMap::new();
        adapters.insert(
            adapter.to_string(),
            AdapterHomeCapability {
                home_paths: home_paths.iter().map(|s| s.to_string()).collect(),
            },
        );
        HomeCapabilitiesConfig {
            schema_version: 1,
            adapters,
        }
    }

    #[test]
    fn default_config_declares_claude_and_agy() {
        let config = default_config().unwrap();
        assert!(config.adapters.contains_key("claude-code"));
        assert!(config.adapters.contains_key("agy"));
        assert!(config.adapters.contains_key("qwen-code"));
        assert!(config.adapters.contains_key("hermes"));
    }

    #[test]
    fn rejects_absolute_home_path() {
        let err =
            parse_config(r#"{"schema_version":1,"adapters":{"x":{"home_paths":["/etc/passwd"]}}}"#)
                .unwrap_err();
        assert!(format!("{err:?}").contains("relative"));
    }

    #[test]
    fn rejects_traversal_home_path() {
        let err =
            parse_config(r#"{"schema_version":1,"adapters":{"x":{"home_paths":["../outside"]}}}"#)
                .unwrap_err();
        assert!(format!("{err:?}").contains("unsafe path component"));
    }

    #[tokio::test]
    async fn prepare_fails_closed_for_undeclared_adapter() {
        let real_home = tempdir().unwrap();
        let sandbox_root = tempdir().unwrap();
        let config = config_with("known-adapter", &[]);

        let err = SandboxHome::prepare(
            "unknown-adapter",
            &config,
            real_home.path(),
            sandbox_root.path(),
        )
        .unwrap_err();
        assert!(err.to_string().contains("no home capability declared"));
    }

    #[tokio::test]
    async fn prepare_copies_only_declared_files_with_restrictive_permissions() {
        let real_home = tempdir().unwrap();
        std::fs::write(real_home.path().join("allowed.json"), "allowed-content").unwrap();
        std::fs::write(real_home.path().join("sentinel.secret"), "must-not-copy").unwrap();
        let sandbox_root = tempdir().unwrap();
        let config = config_with("test-adapter", &["allowed.json"]);

        let sandbox = SandboxHome::prepare(
            "test-adapter",
            &config,
            real_home.path(),
            sandbox_root.path(),
        )
        .unwrap();

        assert!(sandbox.path().join("allowed.json").exists());
        assert!(!sandbox.path().join("sentinel.secret").exists());

        let dir_mode = std::fs::metadata(sandbox.path())
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(dir_mode, 0o700);
        let file_mode = std::fs::metadata(sandbox.path().join("allowed.json"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(file_mode, 0o600);

        let path = sandbox.path().to_path_buf();
        assert!(sandbox.cleanup().await);
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn prepare_skips_missing_declared_path() {
        let real_home = tempdir().unwrap();
        let sandbox_root = tempdir().unwrap();
        let config = config_with("test-adapter", &["not-authenticated-yet.json"]);

        let sandbox = SandboxHome::prepare(
            "test-adapter",
            &config,
            real_home.path(),
            sandbox_root.path(),
        )
        .unwrap();

        assert!(!sandbox.path().join("not-authenticated-yet.json").exists());
        sandbox.cleanup().await;
    }

    #[tokio::test]
    async fn prepare_refuses_symlink_source() {
        let real_home = tempdir().unwrap();
        let outside = tempdir().unwrap();
        std::fs::write(outside.path().join("secret"), "outside-content").unwrap();
        std::os::unix::fs::symlink(
            outside.path().join("secret"),
            real_home.path().join("linked.json"),
        )
        .unwrap();
        let sandbox_root = tempdir().unwrap();
        let config = config_with("test-adapter", &["linked.json"]);

        let err = SandboxHome::prepare(
            "test-adapter",
            &config,
            real_home.path(),
            sandbox_root.path(),
        )
        .unwrap_err();
        assert!(format!("{err:?}").contains("symlink"));
    }

    #[tokio::test]
    async fn cleanup_removes_directory_on_drop_without_explicit_cleanup() {
        let real_home = tempdir().unwrap();
        let sandbox_root = tempdir().unwrap();
        let config = config_with("test-adapter", &[]);

        let sandbox = SandboxHome::prepare(
            "test-adapter",
            &config,
            real_home.path(),
            sandbox_root.path(),
        )
        .unwrap();
        let path = sandbox.path().to_path_buf();
        assert!(path.exists());
        drop(sandbox);
        assert!(!path.exists());
    }
}
