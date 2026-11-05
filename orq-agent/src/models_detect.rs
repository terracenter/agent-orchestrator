use crate::adapters::{AdaptersRegistry, ModelsSource, ModelsSpec};
use crate::models::{format_unix_iso8601, ModelsCatalog};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

/// Un modelo expuesto por un agente, con la fuente de donde salio.
/// Alineado con `models[]` de `agents-inventory.schema.json`.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DetectedModel {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verified_at: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ModelsResolution {
    pub source: String,
    pub models: Vec<DetectedModel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ProviderModelsCache {
    #[serde(default)]
    at: Option<f64>,
    #[serde(default)]
    models: Vec<String>,
}

fn source_label(source: ModelsSource) -> &'static str {
    match source {
        ModelsSource::Agent => "agent",
        ModelsSource::Config => "config",
        ModelsSource::ProviderApi => "provider_api",
        ModelsSource::Catalog => "catalog",
        ModelsSource::Aliases => "aliases",
    }
}

fn dedup(models: &mut Vec<DetectedModel>) {
    let mut seen = HashSet::new();
    models.retain(|model| seen.insert((model.provider.clone(), model.id.clone())));
}

fn finish(source: &str, mut models: Vec<DetectedModel>, note: Option<String>) -> ModelsResolution {
    dedup(&mut models);
    ModelsResolution {
        source: source.to_string(),
        models,
        note,
    }
}

fn from_catalog(agent: &str, catalog: &ModelsCatalog, note: Option<String>) -> ModelsResolution {
    let models = catalog
        .agents
        .get(agent)
        .map(|entries| {
            entries
                .iter()
                .map(|model| DetectedModel {
                    id: model.id.clone(),
                    provider: None,
                    source: "catalog".to_string(),
                    verified_at: model.fetched_at.clone(),
                })
                .collect()
        })
        .unwrap_or_default();
    finish("catalog", models, note)
}

fn resolve_config_models(spec: &ModelsSpec) -> ModelsResolution {
    let raw_path = spec
        .path
        .as_deref()
        .unwrap_or("$HERMES_HOME/provider_models_cache.json");
    let Some(path) = expand_path(raw_path) else {
        return finish(
            "config",
            Vec::new(),
            Some(format!("cannot expand config path {raw_path}")),
        );
    };
    let content = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(err) => {
            return finish(
                "config",
                Vec::new(),
                Some(format!(
                    "config source unavailable at {}: {err}",
                    path.display()
                )),
            );
        }
    };
    let cache: BTreeMap<String, ProviderModelsCache> = match serde_json::from_str(&content) {
        Ok(cache) => cache,
        Err(err) => {
            return finish(
                "config",
                Vec::new(),
                Some(format!("invalid config cache at {}: {err}", path.display())),
            );
        }
    };

    let mut models = Vec::new();
    for (provider, entry) in cache {
        let verified_at = entry
            .at
            .map(|timestamp| format_unix_iso8601(timestamp.max(0.0) as u64));
        for id in entry.models {
            models.push(DetectedModel {
                id,
                provider: Some(provider.clone()),
                source: "config".to_string(),
                verified_at: verified_at.clone(),
            });
        }
    }
    finish("config", models, spec.note.clone())
}

fn expand_path(raw: &str) -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let hermes_home = std::env::var("HERMES_HOME").unwrap_or_else(|_| format!("{home}/.hermes"));
    let mut expanded = raw
        .replace("${HERMES_HOME}", &hermes_home)
        .replace("$HERMES_HOME", &hermes_home)
        .replace("${HOME}", &home)
        .replace("$HOME", &home);
    if let Some(rest) = expanded.strip_prefix("~/") {
        expanded = format!("{home}/{rest}");
    }
    Some(PathBuf::from(expanded))
}

/// Resuelve los modelos de un agente segun `detect.models.source` del registry.
/// `None` o spec ausente cae al catalogo (comportamiento historico).
pub fn resolve_models(
    agent: &str,
    spec: Option<&ModelsSpec>,
    catalog: &ModelsCatalog,
    _home: Option<&Path>,
) -> ModelsResolution {
    let Some(spec) = spec else {
        return from_catalog(agent, catalog, None);
    };

    match spec.source {
        ModelsSource::Aliases => {
            let models = spec
                .aliases
                .clone()
                .unwrap_or_default()
                .into_iter()
                .map(|id| DetectedModel {
                    id,
                    provider: None,
                    source: "aliases".to_string(),
                    verified_at: None,
                })
                .collect();
            finish("aliases", models, spec.note.clone())
        }
        ModelsSource::Config => resolve_config_models(spec),
        ModelsSource::Catalog => from_catalog(agent, catalog, spec.note.clone()),
        ModelsSource::Agent | ModelsSource::ProviderApi => finish(
            source_label(spec.source),
            Vec::new(),
            Some("source not implemented in this build".to_string()),
        ),
    }
}

/// Busca el adapter por nombre y resuelve sus modelos con la spec del registry.
pub fn resolve_agent_models(
    registry: &AdaptersRegistry,
    agent: &str,
    catalog: &ModelsCatalog,
    home: Option<&Path>,
) -> ModelsResolution {
    let spec = registry
        .adapters
        .iter()
        .find(|adapter| adapter.name == agent)
        .and_then(|adapter| adapter.detect.as_ref())
        .and_then(|detect| detect.models.as_ref());
    resolve_models(agent, spec, catalog, home)
}

/// Merge the models resolved from each adapter's `detect.models` spec into
/// `catalog`, so routing sees the same inventory as `detect --models`.
/// Existing catalog entries win; resolved entries are only appended.
pub fn merge_resolved_models(catalog: &mut ModelsCatalog, registry: &AdaptersRegistry) {
    let mut additions: Vec<(String, DetectedModel, Option<String>)> = Vec::new();
    for adapter in &registry.adapters {
        let resolution = resolve_agent_models(registry, &adapter.name, catalog, None);
        for model in resolution.models {
            additions.push((adapter.name.clone(), model, resolution.note.clone()));
        }
    }

    for (agent, model, note) in additions {
        let entry = catalog.agents.entry(agent).or_default();
        if entry.iter().any(|existing| existing.id == model.id) {
            continue;
        }
        entry.push(crate::models::ModelCandidate {
            id: model.id,
            source: model.source,
            confidence: "detected".to_string(),
            notes: note.unwrap_or_default(),
            fetched_at: model.verified_at,
            cost_hint: None,
            promo: None,
            status: Some("active".to_string()),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{expand_path, merge_resolved_models, resolve_agent_models, resolve_models};
    use crate::adapters::ModelsSpec;
    use crate::models::parse_catalog;
    use std::io::Write;

    fn spec(json: &str) -> ModelsSpec {
        serde_json::from_str(json).expect("models spec")
    }

    fn catalog() -> crate::models::ModelsCatalog {
        parse_catalog(r#"{"schema_version":1,"agents":{"other":[]}}"#).expect("catalog")
    }

    #[test]
    fn aliases_source_returns_registry_aliases() {
        let spec = spec(r#"{"source":"aliases","aliases":["fable","opus","sonnet"]}"#);
        let resolution = resolve_models("claude-code", Some(&spec), &catalog(), None);
        assert_eq!(resolution.source, "aliases");
        assert_eq!(resolution.models.len(), 3);
        assert_eq!(resolution.models[0].id, "fable");
        assert_eq!(resolution.models[0].source, "aliases");
    }

    #[test]
    fn config_source_reads_provider_cache() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("provider_models_cache.json");
        let mut file = std::fs::File::create(&path).expect("create cache");
        write!(
            file,
            r#"{{"deepseek":{{"at":1793912359.0,"models":["deepseek-flash","deepseek-v4-pro"]}}}}"#
        )
        .expect("write cache");

        let spec = spec(
            &serde_json::json!({
                "source": "config",
                "path": path.display().to_string()
            })
            .to_string(),
        );
        let resolution = resolve_models("hermes", Some(&spec), &catalog(), None);
        assert_eq!(resolution.source, "config");
        assert_eq!(resolution.models.len(), 2);
        assert!(resolution
            .models
            .iter()
            .all(|model| model.provider.as_deref() == Some("deepseek")));
        assert_eq!(resolution.models[0].id, "deepseek-flash");
        assert!(resolution.models[0].verified_at.is_some());
    }

    #[test]
    fn config_source_missing_file_is_empty_with_note() {
        let spec =
            spec(r#"{"source":"config","path":"/tmp/definitely-missing-orq-models-cache.json"}"#);
        let resolution = resolve_models("hermes", Some(&spec), &catalog(), None);
        assert!(resolution.models.is_empty());
        assert!(resolution
            .note
            .expect("note")
            .contains("config source unavailable"));
    }

    #[test]
    fn catalog_source_reads_catalog_entries() {
        let spec = spec(r#"{"source":"catalog"}"#);
        let catalog = parse_catalog(
            r#"{"schema_version":1,"agents":{"qwen-code":[{"id":"qwen3.6-flash","source":"s","confidence":"c","notes":"n"}]}}"#,
        )
        .expect("catalog");
        let resolution = resolve_models("qwen-code", Some(&spec), &catalog, None);
        assert_eq!(resolution.source, "catalog");
        assert_eq!(resolution.models.len(), 1);
        assert_eq!(resolution.models[0].id, "qwen3.6-flash");
    }

    #[test]
    fn missing_spec_falls_back_to_catalog() {
        let catalog = parse_catalog(
            r#"{"schema_version":1,"agents":{"qwen-code":[{"id":"qwen3.6-flash","source":"s","confidence":"c","notes":"n"}]}}"#,
        )
        .expect("catalog");
        let resolution = resolve_models("qwen-code", None, &catalog, None);
        assert_eq!(resolution.source, "catalog");
        assert_eq!(resolution.models.len(), 1);
    }

    #[test]
    fn resolve_agent_models_uses_registry_spec() {
        let registry = crate::adapters::parse_registry(
            r#"{"schema_version":1,"adapters":[{"name":"claude-code","binary":"claude","status":"gated","argv":["--model","$MODEL","--print","$TASK"],"detect":{"models":{"source":"aliases","aliases":["fable","opus"]}}}]}"#,
        )
        .expect("registry");
        let resolution = resolve_agent_models(&registry, "claude-code", &catalog(), None);
        assert_eq!(resolution.source, "aliases");
        assert_eq!(resolution.models.len(), 2);
    }

    #[test]
    fn merge_resolved_models_appends_config_models_and_keeps_existing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("provider_models_cache.json");
        let mut file = std::fs::File::create(&path).expect("create cache");
        write!(
            file,
            r#"{{"deepseek":{{"at":1793912359.0,"models":["deepseek-flash","deepseek-v4-pro"]}}}}"#
        )
        .expect("write cache");

        let registry = crate::adapters::parse_registry(
            &serde_json::json!({
                "schema_version": 1,
                "adapters": [{
                    "name": "hermes",
                    "binary": "hermes",
                    "status": "available",
                    "argv": ["-m", "$MODEL", "-z", "$TASK"],
                    "detect": {"models": {"source": "config", "path": path.display().to_string()}}
                }]
            })
            .to_string(),
        )
        .expect("registry");

        let mut catalog = parse_catalog(
            r#"{"schema_version":2,"agents":{"hermes":[{"id":"deepseek-flash","source":"manual","confidence":"x","notes":"n","fetched_at":"2026-11-05T00:00:00Z","status":"active"}]}}"#,
        )
        .expect("catalog");

        merge_resolved_models(&mut catalog, &registry);

        let models = catalog.agents.get("hermes").expect("hermes models");
        assert_eq!(models.len(), 2);
        let manual = models
            .iter()
            .find(|model| model.id == "deepseek-flash")
            .expect("manual model");
        assert_eq!(manual.source, "manual");
        let added = models
            .iter()
            .find(|model| model.id == "deepseek-v4-pro")
            .expect("added model");
        assert_eq!(added.source, "config");
        assert!(added.fetched_at.is_some());
    }

    #[test]
    fn expand_path_expands_home() {
        let home = std::env::var("HOME").expect("HOME");
        let path = expand_path("$HOME/.hermes/provider_models_cache.json").expect("expand");
        assert_eq!(
            path,
            std::path::PathBuf::from(format!("{home}/.hermes/provider_models_cache.json"))
        );
    }
}
