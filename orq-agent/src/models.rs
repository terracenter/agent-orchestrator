use crate::adapters::{find_adapter_in_registry, AdapterStatus, AdaptersRegistry};
use color_eyre::eyre::{eyre, Result, WrapErr};
use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

pub const SUPPORTED_SCHEMA_VERSION_V1: u8 = 1;
pub const SUPPORTED_SCHEMA_VERSION_V2: u8 = 2;
pub const CURRENT_SCHEMA_VERSION: u8 = 2;
pub const DEFAULT_MODELS_CATALOG_PATH: &str = "config/models-catalog.json";
pub const DEFAULT_MARKET_FEED_PATH: &str = "config/market-feed.json";
pub const MODELS_CATALOG_ENV: &str = "ORQ_MODELS_CATALOG";
pub const MARKET_FEED_ENV: &str = "ORQ_MARKET_FEED";

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ModelsCatalog {
    pub schema_version: u8,
    pub agents: BTreeMap<String, Vec<ModelCandidate>>,
}

#[derive(Debug, Serialize)]
pub struct ModelsReport {
    pub schema_version: u8,
    pub agent: String,
    pub detected: bool,
    pub status: AdapterStatus,
    pub models: Vec<ModelCandidate>,
    pub discovery: DiscoveryStatus,
    pub config_source: String,
    pub secrets_read: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ModelCandidate {
    pub id: String,
    pub source: String,
    pub confidence: String,
    pub notes: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fetched_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_hint: Option<f64>,
    #[serde(
        default,
        deserialize_with = "deserialize_promo",
        skip_serializing_if = "Option::is_none"
    )]
    pub promo: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

impl ModelCandidate {
    #[allow(dead_code)]
    pub fn is_active(&self) -> bool {
        !matches!(
            self.status.as_deref().map(str::to_lowercase).as_deref(),
            Some("deprecated") | Some("down") | Some("disabled") | Some("offline")
        )
    }

    #[allow(dead_code)]
    pub fn is_deprecated(&self) -> bool {
        self.status.as_deref().map(str::to_lowercase).as_deref() == Some("deprecated")
    }

    #[allow(dead_code)]
    pub fn is_down(&self) -> bool {
        matches!(
            self.status.as_deref().map(str::to_lowercase).as_deref(),
            Some("down") | Some("offline")
        )
    }
}

pub fn deserialize_promo<'de, D>(deserializer: D) -> std::result::Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    struct PromoVisitor;

    impl<'de> de::Visitor<'de> for PromoVisitor {
        type Value = Option<String>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a string, boolean, or null")
        }

        fn visit_none<E>(self) -> std::result::Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }

        fn visit_some<D>(self, deserializer: D) -> std::result::Result<Self::Value, D::Error>
        where
            D: Deserializer<'de>,
        {
            deserializer.deserialize_any(PromoVisitor)
        }

        fn visit_bool<E>(self, v: bool) -> std::result::Result<Self::Value, E>
        where
            E: de::Error,
        {
            if v {
                Ok(Some("true".to_string()))
            } else {
                Ok(None)
            }
        }

        fn visit_str<E>(self, v: &str) -> std::result::Result<Self::Value, E>
        where
            E: de::Error,
        {
            let trimmed = v.trim();
            if trimmed.is_empty() {
                Ok(None)
            } else {
                Ok(Some(trimmed.to_string()))
            }
        }

        fn visit_string<E>(self, v: String) -> std::result::Result<Self::Value, E>
        where
            E: de::Error,
        {
            self.visit_str(&v)
        }

        fn visit_unit<E>(self) -> std::result::Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }
    }

    deserializer.deserialize_option(PromoVisitor)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveryStatus {
    ConfigCatalog,
    Unsupported,
}

#[allow(dead_code)]
pub trait MarketFeedSource {
    fn load_feed(&self) -> Result<MarketFeed>;
}

#[allow(dead_code)]
pub struct FileMarketFeedSource<'a> {
    pub path: &'a Path,
}

#[allow(dead_code)]
impl<'a> MarketFeedSource for FileMarketFeedSource<'a> {
    fn load_feed(&self) -> Result<MarketFeed> {
        let content = std::fs::read_to_string(self.path)
            .wrap_err_with(|| format!("reading market feed from {}", self.path.display()))?;
        parse_market_feed(&content)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MarketFeed {
    #[serde(default = "default_feed_schema_version")]
    pub schema_version: u8,
    #[serde(default)]
    pub feed_source: Option<String>,
    #[serde(default)]
    pub fetched_at: Option<String>,
    #[serde(default)]
    pub agents: BTreeMap<String, Vec<MarketFeedEntry>>,
    #[serde(default)]
    pub entries: Vec<MarketFeedEntryWithAgent>,
}

fn default_feed_schema_version() -> u8 {
    1
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MarketFeedEntry {
    pub id: String,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub confidence: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub cost_hint: Option<f64>,
    #[serde(
        default,
        deserialize_with = "deserialize_promo",
        skip_serializing_if = "Option::is_none"
    )]
    pub promo: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub fetched_at: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MarketFeedEntryWithAgent {
    pub agent: String,
    pub id: String,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub confidence: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub cost_hint: Option<f64>,
    #[serde(
        default,
        deserialize_with = "deserialize_promo",
        skip_serializing_if = "Option::is_none"
    )]
    pub promo: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub fetched_at: Option<String>,
}

impl MarketFeed {
    pub fn all_entries(&self) -> Vec<(String, MarketFeedEntry)> {
        let mut list = Vec::new();
        for (agent, entries) in &self.agents {
            for entry in entries {
                list.push((agent.clone(), entry.clone()));
            }
        }
        for entry in &self.entries {
            list.push((
                entry.agent.clone(),
                MarketFeedEntry {
                    id: entry.id.clone(),
                    source: entry.source.clone(),
                    confidence: entry.confidence.clone(),
                    notes: entry.notes.clone(),
                    cost_hint: entry.cost_hint,
                    promo: entry.promo.clone(),
                    status: entry.status.clone(),
                    fetched_at: entry.fetched_at.clone(),
                },
            ));
        }
        list
    }
}

pub fn parse_market_feed(content: &str) -> Result<MarketFeed> {
    let feed: MarketFeed =
        serde_json::from_str(content).wrap_err("parsing market feed json")?;
    Ok(feed)
}

pub fn now_iso8601() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format_unix_iso8601(secs)
}

pub fn format_unix_iso8601(secs: u64) -> String {
    let days = (secs / 86400) as i64;
    let rem_secs = secs % 86400;
    let hours = rem_secs / 3600;
    let minutes = (rem_secs % 3600) / 60;
    let seconds = rem_secs % 60;

    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    format!("{y:04}-{m:02}-{d:02}T{hours:02}:{minutes:02}:{seconds:02}Z")
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ModelsRefreshSummary {
    pub schema_version: u8,
    pub feed_source: String,
    pub catalog_path: String,
    pub fetched_at: String,
    pub added: usize,
    pub updated: usize,
    pub deprecated: usize,
    pub total_models: usize,
}

pub fn merge_feed_into_catalog(
    catalog: &mut ModelsCatalog,
    feed: &MarketFeed,
    feed_source_label: &str,
    now_iso: &str,
) -> ModelsRefreshSummary {
    catalog.schema_version = CURRENT_SCHEMA_VERSION;

    let mut added = 0;
    let mut updated = 0;
    let mut deprecated = 0;

    for (agent, feed_entry) in feed.all_entries() {
        let agent_models = catalog.agents.entry(agent).or_default();
        if let Some(existing) = agent_models.iter_mut().find(|m| m.id == feed_entry.id) {
            let mut changed = false;
            let was_deprecated = existing
                .status
                .as_deref()
                .map(str::to_lowercase)
                .as_deref()
                == Some("deprecated");

            if let Some(ref status) = feed_entry.status {
                if existing.status.as_ref() != Some(status) {
                    existing.status = Some(status.clone());
                    changed = true;
                }
            }
            let is_deprecated_now = existing
                .status
                .as_deref()
                .map(str::to_lowercase)
                .as_deref()
                == Some("deprecated");
            if !was_deprecated && is_deprecated_now {
                deprecated += 1;
            }

            if feed_entry.cost_hint.is_some() && existing.cost_hint != feed_entry.cost_hint {
                existing.cost_hint = feed_entry.cost_hint;
                changed = true;
            }
            if feed_entry.promo.is_some() && existing.promo != feed_entry.promo {
                existing.promo = feed_entry.promo.clone();
                changed = true;
            }
            if let Some(ref src) = feed_entry.source {
                if &existing.source != src {
                    existing.source = src.clone();
                    changed = true;
                }
            }
            if let Some(ref conf) = feed_entry.confidence {
                if &existing.confidence != conf {
                    existing.confidence = conf.clone();
                    changed = true;
                }
            }
            if let Some(ref notes) = feed_entry.notes {
                if &existing.notes != notes {
                    existing.notes = notes.clone();
                    changed = true;
                }
            }

            let entry_fetched_at = feed_entry.fetched_at.as_deref().unwrap_or(now_iso);
            existing.fetched_at = Some(entry_fetched_at.to_string());

            if changed && !is_deprecated_now {
                updated += 1;
            }
        } else {
            let is_depr = feed_entry
                .status
                .as_deref()
                .map(str::to_lowercase)
                .as_deref()
                == Some("deprecated");
            let new_candidate = ModelCandidate {
                id: feed_entry.id.clone(),
                source: feed_entry
                    .source
                    .unwrap_or_else(|| "market_feed".to_string()),
                confidence: feed_entry
                    .confidence
                    .unwrap_or_else(|| "market_feed_candidate".to_string()),
                notes: feed_entry.notes.unwrap_or_default(),
                fetched_at: Some(
                    feed_entry
                        .fetched_at
                        .unwrap_or_else(|| now_iso.to_string()),
                ),
                cost_hint: feed_entry.cost_hint,
                promo: feed_entry.promo,
                status: feed_entry.status.or_else(|| Some("active".to_string())),
            };
            agent_models.push(new_candidate);
            if is_depr {
                deprecated += 1;
            } else {
                added += 1;
            }
        }
    }

    let total_models = catalog.agents.values().map(|v| v.len()).sum();

    ModelsRefreshSummary {
        schema_version: catalog.schema_version,
        feed_source: feed
            .feed_source
            .clone()
            .unwrap_or_else(|| feed_source_label.to_string()),
        catalog_path: String::new(),
        fetched_at: now_iso.to_string(),
        added,
        updated,
        deprecated,
        total_models,
    }
}

#[allow(dead_code)]
pub fn default_catalog() -> Result<ModelsCatalog> {
    let path = default_config_path(MODELS_CATALOG_ENV, DEFAULT_MODELS_CATALOG_PATH);
    let content = std::fs::read_to_string(&path)
        .wrap_err_with(|| format!("reading models catalog {}", path.display()))?;
    parse_catalog(&content)
}

pub async fn load_catalog(path: Option<&Path>) -> Result<(ModelsCatalog, String)> {
    let path_buf;
    let path = match path {
        Some(path) => path,
        None => {
            path_buf = default_config_path(MODELS_CATALOG_ENV, DEFAULT_MODELS_CATALOG_PATH);
            path_buf.as_path()
        }
    };
    let content = tokio::fs::read_to_string(path)
        .await
        .wrap_err_with(|| format!("reading models catalog {}", path.display()))?;
    Ok((parse_catalog(&content)?, path.display().to_string()))
}

pub async fn load_market_feed(path: Option<&Path>) -> Result<(MarketFeed, String)> {
    let path_buf;
    let path = match path {
        Some(path) => path,
        None => {
            path_buf = default_config_path(MARKET_FEED_ENV, DEFAULT_MARKET_FEED_PATH);
            path_buf.as_path()
        }
    };
    let content = tokio::fs::read_to_string(path)
        .await
        .wrap_err_with(|| format!("reading market feed {}", path.display()))?;
    Ok((parse_market_feed(&content)?, path.display().to_string()))
}

fn default_config_path(env_name: &str, relative_path: &str) -> std::path::PathBuf {
    std::env::var_os(env_name)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative_path))
}

pub fn parse_catalog(content: &str) -> Result<ModelsCatalog> {
    let mut catalog: ModelsCatalog =
        serde_json::from_str(content).wrap_err("parsing models catalog json")?;
    validate_catalog(&catalog)?;
    // Normalize schema_version to CURRENT_SCHEMA_VERSION if parsing v1
    if catalog.schema_version == SUPPORTED_SCHEMA_VERSION_V1 {
        catalog.schema_version = CURRENT_SCHEMA_VERSION;
    }
    Ok(catalog)
}

fn validate_catalog(catalog: &ModelsCatalog) -> Result<()> {
    if catalog.schema_version != SUPPORTED_SCHEMA_VERSION_V1
        && catalog.schema_version != SUPPORTED_SCHEMA_VERSION_V2
    {
        return Err(eyre!(
            "unsupported models catalog schema_version {}; expected 1 or 2",
            catalog.schema_version
        ));
    }
    if catalog.agents.is_empty() {
        return Err(eyre!("models catalog must define at least one agent"));
    }
    for (agent, models) in &catalog.agents {
        if agent.trim().is_empty() {
            return Err(eyre!("models catalog agent name cannot be empty"));
        }
        for model in models {
            if model.id.trim().is_empty() {
                return Err(eyre!("models catalog entry for {agent} has empty id"));
            }
        }
    }
    Ok(())
}

pub fn list(
    agent: &str,
    catalog: &ModelsCatalog,
    adapters_registry: &AdaptersRegistry,
    config_source: &str,
) -> Result<ModelsReport> {
    let adapter = find_adapter_in_registry(agent, adapters_registry)
        .ok_or_else(|| eyre!("unknown agent adapter: {agent}"))?;
    let detected = adapter.binary_path().is_some();
    let models = catalog
        .agents
        .get(adapter.name())
        .cloned()
        .unwrap_or_default();
    let discovery = if models.is_empty() {
        DiscoveryStatus::Unsupported
    } else {
        DiscoveryStatus::ConfigCatalog
    };

    Ok(ModelsReport {
        schema_version: catalog.schema_version,
        agent: adapter.name().to_string(),
        detected,
        status: adapter.status(),
        models,
        discovery,
        config_source: config_source.to_string(),
        secrets_read: false,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        default_catalog, list, merge_feed_into_catalog, parse_catalog, parse_market_feed,
    };

    #[test]
    fn default_catalog_reports_qwen_flash() {
        let catalog = default_catalog().unwrap();
        let adapters_registry = crate::adapters::default_registry().unwrap();
        let report = list("qwen-code", &catalog, &adapters_registry, "test").unwrap();
        assert!(report
            .models
            .iter()
            .any(|model| model.id == "qwen3.6-flash"));
    }

    #[test]
    fn accepts_schema_version_1_and_migrates_to_2() {
        let catalog = parse_catalog(
            r#"{"schema_version":1,"agents":{"qwen-code":[{"id":"qwen3.6-flash","source":"s","confidence":"c","notes":"n"}]}}"#,
        )
        .unwrap();
        assert_eq!(catalog.schema_version, 2);
    }

    #[test]
    fn accepts_schema_version_2_with_market_fields() {
        let catalog = parse_catalog(
            r#"{"schema_version":2,"agents":{"qwen-code":[{"id":"qwen3.6-flash","source":"s","confidence":"c","notes":"n","fetched_at":"2026-09-04T18:00:00Z","cost_hint":0.0001,"promo":"anthropic+50%","status":"active"}]}}"#,
        )
        .unwrap();
        assert_eq!(catalog.schema_version, 2);
        let m = &catalog.agents["qwen-code"][0];
        assert_eq!(m.cost_hint, Some(0.0001));
        assert_eq!(m.promo, Some("anthropic+50%".to_string()));
        assert_eq!(m.status, Some("active".to_string()));
        assert!(m.is_active());
    }

    #[test]
    fn promo_handles_boolean_true_and_false() {
        let catalog = parse_catalog(
            r#"{"schema_version":2,"agents":{"qwen-code":[{"id":"m1","source":"s","confidence":"c","notes":"n","promo":true},{"id":"m2","source":"s","confidence":"c","notes":"n","promo":false}]}}"#,
        )
        .unwrap();
        assert_eq!(catalog.agents["qwen-code"][0].promo, Some("true".to_string()));
        assert_eq!(catalog.agents["qwen-code"][1].promo, None);
    }

    #[test]
    fn rejects_invalid_schema() {
        let err = parse_catalog(r#"{"schema_version":3,"agents":{}}"#).unwrap_err();
        assert!(err
            .to_string()
            .contains("unsupported models catalog schema_version"));
    }

    #[test]
    fn rejects_empty_model_id() {
        let err = parse_catalog(
            r#"{"schema_version":1,"agents":{"qwen-code":[{"id":"","source":"s","confidence":"c","notes":"n"}]}}"#,
        )
        .unwrap_err();
        assert!(err.to_string().contains("empty id"));
    }

    #[test]
    fn test_merge_feed_into_catalog_is_idempotent() {
        let mut catalog = parse_catalog(
            r#"{"schema_version":1,"agents":{"qwen-code":[{"id":"qwen3.6-flash","source":"policy","confidence":"cand","notes":"daily"}]}}"#,
        )
        .unwrap();

        let feed = parse_market_feed(
            r#"{
                "schema_version": 1,
                "feed_source": "test_feed",
                "agents": {
                    "qwen-code": [
                        {"id": "qwen3.6-flash", "cost_hint": 0.0001, "promo": "special_offer"},
                        {"id": "qwen3.8-max", "cost_hint": 0.005, "status": "active"},
                        {"id": "old-qwen", "status": "deprecated"}
                    ]
                }
            }"#,
        )
        .unwrap();

        let summary1 = merge_feed_into_catalog(
            &mut catalog,
            &feed,
            "test_feed",
            "2026-09-04T18:00:00Z",
        );
        assert_eq!(summary1.added, 1); // qwen3.8-max
        assert_eq!(summary1.updated, 1); // qwen3.6-flash
        assert_eq!(summary1.deprecated, 1); // old-qwen
        assert_eq!(summary1.total_models, 3);

        // Check model fields
        let m_flash = catalog.agents["qwen-code"]
            .iter()
            .find(|m| m.id == "qwen3.6-flash")
            .unwrap();
        assert_eq!(m_flash.cost_hint, Some(0.0001));
        assert_eq!(m_flash.promo, Some("special_offer".to_string()));
        assert_eq!(m_flash.fetched_at, Some("2026-09-04T18:00:00Z".to_string()));

        // Second run with the same feed must be idempotent (0 added, 0 newly deprecated)
        let summary2 = merge_feed_into_catalog(
            &mut catalog,
            &feed,
            "test_feed",
            "2026-09-04T18:05:00Z",
        );
        assert_eq!(summary2.added, 0);
        assert_eq!(summary2.deprecated, 0);
        assert_eq!(summary2.total_models, 3);
        assert_eq!(catalog.agents["qwen-code"].len(), 3);
    }
}
