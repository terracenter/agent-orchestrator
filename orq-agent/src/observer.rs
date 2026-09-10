use std::path::{Path, PathBuf};

use color_eyre::eyre::{eyre, Result, WrapErr};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::models;
use crate::runtime::{self, ModelsSnapshot};

#[derive(Clone, Debug, Default)]
pub struct ObserverEmitArgs {
    pub endpoint: Option<String>,
    pub token_file: Option<String>,
    pub dry_run: bool,
    pub output: Option<String>,
    pub host_ip: Option<String>,
    pub adapters_config: Option<String>,
    pub models_config: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ObserverAgentSummary {
    pub agent_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_expires_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ObserverPayload {
    pub snapshot_id: String,
    pub discovered_agents_count: usize,
    pub active_models_count: usize,
    pub agents_summary: Vec<ObserverAgentSummary>,
    pub verification_signature: String,
}

#[derive(Serialize)]
struct ObserverPayloadCore<'a> {
    snapshot_id: &'a str,
    discovered_agents_count: usize,
    active_models_count: usize,
    agents_summary: &'a [ObserverAgentSummary],
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ObserverEvent {
    pub event_type: String,
    pub timestamp: String,
    pub host: String,
    pub host_ip: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    pub payload: ObserverPayload,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ObserverEmitReport {
    pub status: String,
    pub event_type: String,
    pub snapshot_id: String,
    pub endpoint: String,
    pub discovered_agents_count: usize,
    pub active_models_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_status: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_path: Option<String>,
    pub secrets_read: bool,
}

fn hex_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn hostname_label() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("HOST"))
        .unwrap_or_else(|_| "host01".to_string())
}

pub fn build_observer_event(
    snapshot: &ModelsSnapshot,
    host_ip: Option<&str>,
) -> Result<ObserverEvent> {
    let mut active_models_count = 0;
    let mut agents_summary = Vec::new();

    for agent in &snapshot.agents {
        for provider in &agent.providers {
            for model in &provider.models {
                if model.status == "active" {
                    active_models_count += 1;
                }
            }
        }

        let first_provider = agent.providers.first();
        let provider_id = first_provider.map(|p| p.id.clone());

        let active_model = agent
            .providers
            .iter()
            .flat_map(|p| p.models.iter())
            .find(|m| m.status == "active")
            .map(|m| m.id.clone())
            .or_else(|| {
                first_provider
                    .and_then(|p| p.models.first())
                    .map(|m| m.id.clone())
            });

        let plan_status = first_provider
            .and_then(|p| p.plan.as_ref())
            .map(|pl| pl.name.clone());

        let plan_expires_at = first_provider
            .and_then(|p| p.plan.as_ref())
            .and_then(|pl| pl.period_end.clone());

        agents_summary.push(ObserverAgentSummary {
            agent_id: agent.id.clone(),
            version: agent.binary.version.clone(),
            provider: provider_id,
            active_model,
            plan_status,
            plan_expires_at,
        });
    }

    let core = ObserverPayloadCore {
        snapshot_id: &snapshot.snapshot_id,
        discovered_agents_count: snapshot.agents.len(),
        active_models_count,
        agents_summary: &agents_summary,
    };
    let core_bytes =
        serde_json::to_vec(&core).wrap_err("serializing observer payload core for signature")?;
    let verification_signature = format!("sha256:{}", hex_sha256(&core_bytes));

    let payload = ObserverPayload {
        snapshot_id: snapshot.snapshot_id.clone(),
        discovered_agents_count: snapshot.agents.len(),
        active_models_count,
        agents_summary,
        verification_signature,
    };

    Ok(ObserverEvent {
        event_type: "agent.discovery.snapshot".to_string(),
        timestamp: models::now_iso8601(),
        host: hostname_label(),
        host_ip: host_ip.unwrap_or("127.0.0.1").to_string(),
        task_id: None,
        payload,
    })
}

pub fn load_host_token(path: &Path) -> Result<String> {
    if !path.exists() {
        return Err(eyre!("host token file not found at {}", path.display()));
    }
    let content = std::fs::read_to_string(path)
        .wrap_err_with(|| format!("reading host token file from {}", path.display()))?;
    let token = content.trim().to_string();
    if token.is_empty() {
        return Err(eyre!("host token file is empty at {}", path.display()));
    }
    Ok(token)
}

/// Resuelve el path del token file. Chequea, en orden: flag explícito, `ORQ_OBSERVER_TOKEN_FILE`,
/// `ORQ_OBSERVER_HOST_TOKEN_FILE` y el default. La segunda variable es un alias de
/// compatibilidad: `docs/deploy-cwp.md` y `docs/produccion-local.md` de observer-llm documentan
/// (y el `client.env` real de este host usa) `ORQ_OBSERVER_HOST_TOKEN_FILE`, no
/// `ORQ_OBSERVER_TOKEN_FILE` -- descubierto al validar end-to-end el issue #33: sin este alias,
/// la config de producción ya desplegada queda huérfana y el binario cae siempre al default.
pub fn resolve_token_file(arg: Option<&str>) -> PathBuf {
    if let Some(path) = arg {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    if let Ok(env_path) = std::env::var("ORQ_OBSERVER_TOKEN_FILE") {
        let trimmed = env_path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    if let Ok(env_path) = std::env::var("ORQ_OBSERVER_HOST_TOKEN_FILE") {
        let trimmed = env_path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".config")
        .join("sge-observer")
        .join("agent-orchestrator.host-token")
}

/// Resuelve el endpoint. Chequea, en orden: flag explícito, `ORQ_OBSERVER_ENDPOINT`,
/// `ORQ_OBSERVER_URL` y el default. Mismo motivo que `resolve_token_file`: `ORQ_OBSERVER_URL` es
/// el nombre documentado/desplegado realmente (`client.env`), no `ORQ_OBSERVER_ENDPOINT`.
pub fn resolve_endpoint(arg: Option<&str>) -> String {
    if let Some(ep) = arg {
        let trimmed = ep.trim();
        if !trimmed.is_empty() {
            return trimmed.trim_end_matches('/').to_string();
        }
    }
    if let Ok(ep) = std::env::var("ORQ_OBSERVER_ENDPOINT") {
        let trimmed = ep.trim();
        if !trimmed.is_empty() {
            return trimmed.trim_end_matches('/').to_string();
        }
    }
    if let Ok(ep) = std::env::var("ORQ_OBSERVER_URL") {
        let trimmed = ep.trim();
        if !trimmed.is_empty() {
            return trimmed.trim_end_matches('/').to_string();
        }
    }
    "https://sge-panel.humanbyte.net".to_string()
}

pub async fn emit(args: ObserverEmitArgs) -> Result<ObserverEmitReport> {
    let adapters_config_path = args.adapters_config.as_deref().map(Path::new);
    let models_config_path = args.models_config.as_deref().map(Path::new);

    let snapshot =
        runtime::run_models_snapshot(None, adapters_config_path, models_config_path).await?;
    let event = build_observer_event(&snapshot, args.host_ip.as_deref())?;
    let endpoint = resolve_endpoint(args.endpoint.as_deref());

    if args.dry_run {
        if let Some(ref out_path) = args.output {
            let json_str = serde_json::to_string_pretty(&event)
                .wrap_err("serializing observer event for output file")?;
            if let Some(parent) = Path::new(out_path).parent() {
                if !parent.as_os_str().is_empty() {
                    tokio::fs::create_dir_all(parent)
                        .await
                        .wrap_err_with(|| format!("creating directory for {}", out_path))?;
                }
            }
            tokio::fs::write(out_path, json_str)
                .await
                .wrap_err_with(|| format!("writing observer event to {}", out_path))?;
        }

        return Ok(ObserverEmitReport {
            status: "dry_run".to_string(),
            event_type: event.event_type,
            snapshot_id: event.payload.snapshot_id,
            endpoint,
            discovered_agents_count: event.payload.discovered_agents_count,
            active_models_count: event.payload.active_models_count,
            http_status: None,
            output_path: args.output,
            secrets_read: false,
        });
    }

    let token_path = resolve_token_file(args.token_file.as_deref());
    let token = load_host_token(&token_path)?;

    let url = format!("{endpoint}/api/events/ingest");
    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .header("X-Host-Token", token)
        .header("Content-Type", "application/json")
        .json(&event)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .wrap_err_with(|| format!("sending observer event to {url}"))?;

    let http_status = response.status();
    if !http_status.is_success() {
        let body = response.text().await.unwrap_or_default();
        let body_tail = crate::receipt::tail_sanitized(body.as_bytes(), 512);
        return Err(eyre!(
            "observer event ingestion failed with HTTP status {}: {}",
            http_status,
            body_tail
        ));
    }

    if let Some(ref out_path) = args.output {
        let json_str = serde_json::to_string_pretty(&event)
            .wrap_err("serializing observer event for output file")?;
        if let Some(parent) = Path::new(out_path).parent() {
            if !parent.as_os_str().is_empty() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .wrap_err_with(|| format!("creating directory for {}", out_path))?;
            }
        }
        tokio::fs::write(out_path, json_str)
            .await
            .wrap_err_with(|| format!("writing observer event to {}", out_path))?;
    }

    Ok(ObserverEmitReport {
        status: "sent".to_string(),
        event_type: event.event_type,
        snapshot_id: event.payload.snapshot_id,
        endpoint,
        discovered_agents_count: event.payload.discovered_agents_count,
        active_models_count: event.payload.active_models_count,
        http_status: Some(http_status.as_u16()),
        output_path: args.output,
        secrets_read: false,
    })
}

/// Item de snapshot de capacidad, compatible con el contrato Go
/// `postgres.CapacitySnapshotInput` de observer-llm (`POST /api/capacity/snapshots`).
/// Los nombres de campo deben coincidir exactamente con los tags `json:"..."` de esa
/// struct (verificado contra `pkg/store/postgres/queries.go` en observer-llm, issue #33).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CapacitySnapshotItem {
    pub agent: String,
    pub provider_group: String,
    pub model_group: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remaining_percent: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub used_percent: Option<f64>,
    pub window: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resets_at: Option<String>,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub captured_at: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct ObserverSnapshotArgs {
    pub endpoint: Option<String>,
    pub token_file: Option<String>,
    pub dry_run: bool,
    pub output: Option<String>,
    pub db_path: Option<String>,
    pub budget_config: Option<String>,
    pub skip_rtk: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ObserverSnapshotReport {
    pub status: String,
    pub endpoint: String,
    pub snapshots: Vec<CapacitySnapshotItem>,
    pub notes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_status: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inserted: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_path: Option<String>,
    pub secrets_read: bool,
}

fn clamp_percent(value: f64) -> f64 {
    if value.is_nan() {
        return 0.0;
    }
    value.clamp(0.0, 100.0)
}

struct BudgetSnapshotOutcome {
    items: Vec<CapacitySnapshotItem>,
    notes: Vec<String>,
}

/// Construye snapshots de presupuesto diario/mensual desde el ledger real de gasto
/// (`state::budget_spent_today_usd` / `budget_spent_this_month_usd`, issue #183) contra los
/// techos configurados en `budget.json`. Nunca reporta gasto absoluto en dinero por tarea --
/// solo el porcentaje agregado usado/restante contra el techo configurado. Si no hay techo
/// configurado (default del binario, sin `--budget-config`), reporta `not_reported` en vez de
/// inventar un porcentaje.
async fn build_budget_snapshot_items(
    db_path: Option<&Path>,
    budget_config: Option<&Path>,
) -> BudgetSnapshotOutcome {
    let mut items = Vec::new();
    let mut notes = Vec::new();

    let loaded_budget = match crate::budget::load_config(budget_config).await {
        Ok(loaded) => loaded,
        Err(err) => {
            notes.push(format!(
                "budget: not_reported (error cargando budget config: {err})"
            ));
            return BudgetSnapshotOutcome { items, notes };
        }
    };

    let (spent_today_usd, spent_month_usd) = match crate::state::open(db_path) {
        Ok(store) => (
            store.budget_spent_today_usd().unwrap_or(0.0),
            store.budget_spent_this_month_usd().unwrap_or(0.0),
        ),
        Err(err) => {
            notes.push(format!(
                "budget: not_reported (no se pudo abrir state DB: {err})"
            ));
            return BudgetSnapshotOutcome { items, notes };
        }
    };

    let now = models::now_iso8601();

    match loaded_budget.config.daily_limit_usd {
        Some(limit) if limit > 0.0 => {
            let used = clamp_percent(spent_today_usd / limit * 100.0);
            items.push(CapacitySnapshotItem {
                agent: "orq".to_string(),
                provider_group: "budget".to_string(),
                model_group: "daily".to_string(),
                remaining_percent: Some(clamp_percent(100.0 - used)),
                used_percent: Some(used),
                window: "1d".to_string(),
                resets_at: None,
                source: "agent-orchestrator".to_string(),
                captured_at: Some(now.clone()),
            });
        }
        _ => notes.push(
            "budget.daily: not_reported (sin daily_limit_usd configurado en budget.json)"
                .to_string(),
        ),
    }

    match loaded_budget.config.monthly_limit_usd {
        Some(limit) if limit > 0.0 => {
            let used = clamp_percent(spent_month_usd / limit * 100.0);
            items.push(CapacitySnapshotItem {
                agent: "orq".to_string(),
                provider_group: "budget".to_string(),
                model_group: "monthly".to_string(),
                remaining_percent: Some(clamp_percent(100.0 - used)),
                used_percent: Some(used),
                window: "30d".to_string(),
                resets_at: None,
                source: "agent-orchestrator".to_string(),
                captured_at: Some(now),
            });
        }
        _ => notes.push(
            "budget.monthly: not_reported (sin monthly_limit_usd configurado en budget.json)"
                .to_string(),
        ),
    }

    BudgetSnapshotOutcome { items, notes }
}

#[derive(Deserialize)]
struct RtkGainSummary {
    avg_savings_pct: f64,
}

#[derive(Deserialize)]
struct RtkGainOutput {
    summary: RtkGainSummary,
}

/// Ahorro agregado de RTK (`rtk gain --format json`, `summary.avg_savings_pct`): un único
/// porcentaje agregado histórico, sin comandos individuales, prompts ni respuestas (issue #33,
/// "métricas RTK si están agregadas y sin prompts/respuestas"). Si el binario `rtk` no está en
/// PATH, falla, o su salida no parsea, reporta `not_reported` -- nunca inventa un valor.
async fn build_rtk_snapshot_item(skip: bool) -> (Option<CapacitySnapshotItem>, String) {
    if skip {
        return (None, "rtk: not_reported (--skip-rtk)".to_string());
    }

    let output = match tokio::process::Command::new("rtk")
        .args(["gain", "--format", "json"])
        .output()
        .await
    {
        Ok(output) => output,
        Err(_) => {
            return (
                None,
                "rtk: not_reported (binario rtk no encontrado en PATH)".to_string(),
            )
        }
    };

    if !output.status.success() {
        return (
            None,
            "rtk: not_reported (rtk gain --format json salió con error)".to_string(),
        );
    }

    let parsed: RtkGainOutput = match serde_json::from_slice(&output.stdout) {
        Ok(parsed) => parsed,
        Err(_) => {
            return (
                None,
                "rtk: not_reported (no se pudo parsear la salida de rtk gain)".to_string(),
            )
        }
    };

    let used = clamp_percent(parsed.summary.avg_savings_pct);
    let item = CapacitySnapshotItem {
        agent: "rtk".to_string(),
        provider_group: "rtk".to_string(),
        model_group: "token_savings_pct".to_string(),
        remaining_percent: Some(clamp_percent(100.0 - used)),
        used_percent: Some(used),
        window: "all_time".to_string(),
        resets_at: None,
        source: "agent-orchestrator/rtk".to_string(),
        captured_at: Some(models::now_iso8601()),
    };
    (Some(item), "rtk: reported".to_string())
}

#[derive(Deserialize, Default)]
struct IngestAck {
    #[serde(default)]
    inserted: Option<i64>,
}

/// Construye y opcionalmente envía snapshots agregados de capacidad (presupuesto de orq +
/// ahorro RTK) a `POST {endpoint}/api/capacity/snapshots` de observer-llm, reutilizando el
/// mismo token de host y resolución de endpoint que `emit()`.
///
/// Nunca bloquea al invocador: cualquier fallo de red, HTTP no-2xx, o ausencia de métrica se
/// captura en `notes`/`status` del reporte devuelto (`Ok(...)`), no en un `Err` -- este comando
/// es explícito y manual (issue #33 AC2: "si Observer está caído, no rompe la tarea principal").
/// La única vía de `Err` real es un fallo de I/O local irrecuperable (escritura de `--output`, o
/// token de host ilegible cuando se intenta un envío real no-dry-run).
pub async fn snapshot(args: ObserverSnapshotArgs) -> Result<ObserverSnapshotReport> {
    let db_path = args.db_path.as_deref().map(Path::new);
    let budget_config_path = args.budget_config.as_deref().map(Path::new);

    let budget_outcome = build_budget_snapshot_items(db_path, budget_config_path).await;
    let (rtk_item, rtk_note) = build_rtk_snapshot_item(args.skip_rtk).await;

    let mut items = budget_outcome.items;
    let mut notes = budget_outcome.notes;
    notes.push(rtk_note);
    if let Some(item) = rtk_item {
        items.push(item);
    }

    let endpoint = resolve_endpoint(args.endpoint.as_deref());

    if let Some(ref out_path) = args.output {
        let json_str = serde_json::to_string_pretty(&items)
            .wrap_err("serializing capacity snapshot items for output file")?;
        if let Some(parent) = Path::new(out_path).parent() {
            if !parent.as_os_str().is_empty() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .wrap_err_with(|| format!("creating directory for {}", out_path))?;
            }
        }
        tokio::fs::write(out_path, json_str)
            .await
            .wrap_err_with(|| format!("writing capacity snapshot items to {}", out_path))?;
    }

    if args.dry_run || items.is_empty() {
        let status = if items.is_empty() {
            "skipped_no_metrics"
        } else {
            "dry_run"
        };
        return Ok(ObserverSnapshotReport {
            status: status.to_string(),
            endpoint,
            snapshots: items,
            notes,
            http_status: None,
            inserted: None,
            output_path: args.output,
            secrets_read: false,
        });
    }

    let token_path = resolve_token_file(args.token_file.as_deref());
    let token = load_host_token(&token_path)?;

    let url = format!("{endpoint}/api/capacity/snapshots");
    let client = reqwest::Client::new();
    let send_result = client
        .post(&url)
        .header("X-Host-Token", token)
        .header("Content-Type", "application/json")
        .json(&items)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await;

    let response = match send_result {
        Ok(resp) => resp,
        Err(err) => {
            notes.push(format!(
                "observer: no disponible, snapshot no enviado ({err})"
            ));
            return Ok(ObserverSnapshotReport {
                status: "observer_unavailable".to_string(),
                endpoint,
                snapshots: items,
                notes,
                http_status: None,
                inserted: None,
                output_path: args.output,
                secrets_read: false,
            });
        }
    };

    let http_status = response.status();
    if !http_status.is_success() {
        let body = response.text().await.unwrap_or_default();
        let body_tail = crate::receipt::tail_sanitized(body.as_bytes(), 512);
        notes.push(format!(
            "observer: respondió HTTP {http_status} al enviar snapshot: {body_tail}"
        ));
        return Ok(ObserverSnapshotReport {
            status: "send_failed".to_string(),
            endpoint,
            snapshots: items,
            notes,
            http_status: Some(http_status.as_u16()),
            inserted: None,
            output_path: args.output,
            secrets_read: false,
        });
    }

    let ack: IngestAck = response.json().await.unwrap_or_default();

    Ok(ObserverSnapshotReport {
        status: "sent".to_string(),
        endpoint,
        snapshots: items,
        notes,
        http_status: Some(http_status.as_u16()),
        inserted: ack.inserted,
        output_path: args.output,
        secrets_read: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::{
        RuntimeAgentSnapshot, RuntimeBinaryInfo, RuntimeConfigMetadata, RuntimeModel, RuntimePlan,
        RuntimeProvider,
    };
    use tempfile::NamedTempFile;
    use tokio::sync::Mutex;

    static TEST_ENV_MUTEX: Mutex<()> = Mutex::const_new(());

    fn sample_snapshot() -> ModelsSnapshot {
        ModelsSnapshot {
            schema_url: None,
            snapshot_id: "snap-20260904-213500-host01".to_string(),
            fetched_at: "2026-09-04T21:35:00Z".to_string(),
            user_id: "freddy".to_string(),
            source: "runtime_doctor".to_string(),
            agents: vec![
                RuntimeAgentSnapshot {
                    id: "qwen-code".to_string(),
                    name: "Qwen Code".to_string(),
                    vendor: "bailian".to_string(),
                    binary: RuntimeBinaryInfo {
                        command: "qwen".to_string(),
                        path: Some("/usr/local/bin/qwen".to_string()),
                        version: Some("1.4.2".to_string()),
                        detected: true,
                    },
                    config_metadata: RuntimeConfigMetadata {
                        settings_path: None,
                        has_credentials: true,
                        auth_type: Some("api_key".to_string()),
                        secrets_read: false,
                    },
                    providers: vec![RuntimeProvider {
                        id: "bailian".to_string(),
                        name: "Alibaba Cloud Bailian".to_string(),
                        endpoint: None,
                        plan: Some(RuntimePlan {
                            name: "Qwen Standard Plan".to_string(),
                            active: true,
                            period_start: None,
                            period_end: Some("2026-09-24T12:00:00Z".to_string()),
                            auto_renewal: None,
                        }),
                        models: vec![
                            RuntimeModel {
                                id: "qwen3.8-max".to_string(),
                                status: "active".to_string(),
                                source_type: "catalog".to_string(),
                                verified: true,
                                last_verified_at: None,
                                cost_hint: None,
                                capabilities: None,
                            },
                            RuntimeModel {
                                id: "qwen3.8-plus".to_string(),
                                status: "active".to_string(),
                                source_type: "catalog".to_string(),
                                verified: false,
                                last_verified_at: None,
                                cost_hint: None,
                                capabilities: None,
                            },
                            RuntimeModel {
                                id: "qwen-turbo-old".to_string(),
                                status: "deprecated".to_string(),
                                source_type: "catalog".to_string(),
                                verified: false,
                                last_verified_at: None,
                                cost_hint: None,
                                capabilities: None,
                            },
                        ],
                    }],
                    modes: vec![],
                    tools: vec![],
                    probed_at: "2026-09-04T21:35:00Z".to_string(),
                    source: "runtime_doctor".to_string(),
                    secrets_read: false,
                },
                RuntimeAgentSnapshot {
                    id: "claude-code".to_string(),
                    name: "Claude Code".to_string(),
                    vendor: "anthropic".to_string(),
                    binary: RuntimeBinaryInfo {
                        command: "claude".to_string(),
                        path: Some("/usr/local/bin/claude".to_string()),
                        version: Some("1.0.18".to_string()),
                        detected: true,
                    },
                    config_metadata: RuntimeConfigMetadata {
                        settings_path: None,
                        has_credentials: true,
                        auth_type: Some("api_key".to_string()),
                        secrets_read: false,
                    },
                    providers: vec![RuntimeProvider {
                        id: "anthropic".to_string(),
                        name: "Anthropic API".to_string(),
                        endpoint: None,
                        plan: None,
                        models: vec![RuntimeModel {
                            id: "claude-3-7-sonnet".to_string(),
                            status: "active".to_string(),
                            source_type: "catalog".to_string(),
                            verified: true,
                            last_verified_at: None,
                            cost_hint: None,
                            capabilities: None,
                        }],
                    }],
                    modes: vec![],
                    tools: vec![],
                    probed_at: "2026-09-04T21:35:00Z".to_string(),
                    source: "runtime_doctor".to_string(),
                    secrets_read: false,
                },
            ],
            secrets_read: false,
        }
    }

    #[test]
    fn build_observer_event_maps_counts_and_summary() {
        let snapshot = sample_snapshot();
        let event = build_observer_event(&snapshot, Some("192.168.1.50")).unwrap();

        assert_eq!(event.event_type, "agent.discovery.snapshot");
        assert_eq!(event.host_ip, "192.168.1.50");
        assert_eq!(event.payload.snapshot_id, "snap-20260904-213500-host01");
        assert_eq!(event.payload.discovered_agents_count, 2);
        // qwen has 2 active models (1 deprecated), claude has 1 active model -> total 3 active models
        assert_eq!(event.payload.active_models_count, 3);
        assert_eq!(event.payload.agents_summary.len(), 2);

        let qwen_sum = &event.payload.agents_summary[0];
        assert_eq!(qwen_sum.agent_id, "qwen-code");
        assert_eq!(qwen_sum.version.as_deref(), Some("1.4.2"));
        assert_eq!(qwen_sum.provider.as_deref(), Some("bailian"));
        assert_eq!(qwen_sum.active_model.as_deref(), Some("qwen3.8-max"));
        assert_eq!(qwen_sum.plan_status.as_deref(), Some("Qwen Standard Plan"));
        assert_eq!(
            qwen_sum.plan_expires_at.as_deref(),
            Some("2026-09-24T12:00:00Z")
        );

        let claude_sum = &event.payload.agents_summary[1];
        assert_eq!(claude_sum.agent_id, "claude-code");
        assert_eq!(claude_sum.version.as_deref(), Some("1.0.18"));
        assert_eq!(claude_sum.provider.as_deref(), Some("anthropic"));
        assert_eq!(
            claude_sum.active_model.as_deref(),
            Some("claude-3-7-sonnet")
        );
        assert_eq!(claude_sum.plan_status, None);
        assert_eq!(claude_sum.plan_expires_at, None);

        assert!(event.payload.verification_signature.starts_with("sha256:"));
        assert_eq!(event.payload.verification_signature.len(), 7 + 64);
    }

    #[test]
    fn verification_signature_is_sha256_and_stable() {
        let snapshot = sample_snapshot();
        let event1 = build_observer_event(&snapshot, None).unwrap();
        let event2 = build_observer_event(&snapshot, None).unwrap();

        assert_eq!(
            event1.payload.verification_signature,
            event2.payload.verification_signature
        );
        let sig = &event1.payload.verification_signature;
        assert!(sig.starts_with("sha256:"));
        let hex_part = &sig[7..];
        assert_eq!(hex_part.len(), 64);
        assert!(hex_part.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn load_host_token_trims_and_handles_errors_safely() {
        let mut temp = NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut temp, b"  secret-host-token-xyz \n\r\n").unwrap();
        let token = load_host_token(temp.path()).unwrap();
        assert_eq!(token, "secret-host-token-xyz");

        // Empty file error
        let mut empty_temp = NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut empty_temp, b"   \n\t ").unwrap();
        let empty_err = load_host_token(empty_temp.path()).unwrap_err();
        let empty_err_msg = format!("{empty_err}");
        assert!(empty_err_msg.contains("host token file is empty"));
        assert!(!empty_err_msg.contains("secret"));

        // Non existent file error
        let missing_path = Path::new("/tmp/definitely-non-existent-host-token-file-xyz123");
        let err = load_host_token(missing_path).unwrap_err();
        let err_msg = format!("{err}");
        assert!(err_msg.contains("host token file not found"));
        assert!(!err_msg.contains("secret"));
    }

    #[tokio::test]
    async fn resolve_token_file_priority() {
        let _guard = TEST_ENV_MUTEX.lock().await;

        // 1. Explicit arg
        let res_arg = resolve_token_file(Some("/custom/token/file"));
        assert_eq!(res_arg, PathBuf::from("/custom/token/file"));

        // 2. Env var
        std::env::set_var("ORQ_OBSERVER_TOKEN_FILE", "/env/token/path");
        let res_env = resolve_token_file(None);
        assert_eq!(res_env, PathBuf::from("/env/token/path"));
        std::env::remove_var("ORQ_OBSERVER_TOKEN_FILE");

        // 3. Alias env var ORQ_OBSERVER_HOST_TOKEN_FILE (nombre real usado por client.env /
        // docs/deploy-cwp.md / docs/produccion-local.md de observer-llm)
        std::env::set_var("ORQ_OBSERVER_HOST_TOKEN_FILE", "/env/alias/token/path");
        let res_alias = resolve_token_file(None);
        assert_eq!(res_alias, PathBuf::from("/env/alias/token/path"));
        std::env::remove_var("ORQ_OBSERVER_HOST_TOKEN_FILE");

        // 4. Default fallback to ~/.config/sge-observer/agent-orchestrator.host-token
        let res_default = resolve_token_file(None);
        assert!(res_default.ends_with(".config/sge-observer/agent-orchestrator.host-token"));
    }

    #[tokio::test]
    async fn resolve_endpoint_priority() {
        let _guard = TEST_ENV_MUTEX.lock().await;

        // 1. Explicit arg
        let res_arg = resolve_endpoint(Some("https://custom.endpoint.local/"));
        assert_eq!(res_arg, "https://custom.endpoint.local");

        // 2. Env var
        std::env::set_var("ORQ_OBSERVER_ENDPOINT", "https://env.endpoint.local/");
        let res_env = resolve_endpoint(None);
        assert_eq!(res_env, "https://env.endpoint.local");
        std::env::remove_var("ORQ_OBSERVER_ENDPOINT");

        // 3. Alias env var ORQ_OBSERVER_URL (nombre real usado por client.env /
        // docs/deploy-cwp.md / docs/produccion-local.md de observer-llm)
        std::env::set_var("ORQ_OBSERVER_URL", "https://alias.endpoint.local/");
        let res_alias = resolve_endpoint(None);
        assert_eq!(res_alias, "https://alias.endpoint.local");
        std::env::remove_var("ORQ_OBSERVER_URL");

        // 4. Default fallback
        let res_default = resolve_endpoint(None);
        assert_eq!(res_default, "https://sge-panel.humanbyte.net");
    }

    #[test]
    fn observer_event_serializes_task_id_when_present() {
        let event_without_task = ObserverEvent {
            event_type: "test".to_string(),
            timestamp: "2026-09-08T00:00:00Z".to_string(),
            host: "host1".to_string(),
            host_ip: "127.0.0.1".to_string(),
            task_id: None,
            payload: ObserverPayload {
                snapshot_id: "snap1".to_string(),
                discovered_agents_count: 0,
                active_models_count: 0,
                agents_summary: Vec::new(),
                verification_signature: "sig".to_string(),
            },
        };
        let json_without = serde_json::to_string(&event_without_task).unwrap();
        assert!(!json_without.contains("task_id"));

        let event_with_task = ObserverEvent {
            task_id: Some("task-456".to_string()),
            ..event_without_task
        };
        let json_with = serde_json::to_string(&event_with_task).unwrap();
        assert!(json_with.contains("\"task_id\":\"task-456\""));
    }

    #[test]
    fn clamp_percent_bounds_and_rejects_nan() {
        assert_eq!(clamp_percent(-10.0), 0.0);
        assert_eq!(clamp_percent(50.0), 50.0);
        assert_eq!(clamp_percent(150.0), 100.0);
        assert_eq!(clamp_percent(f64::NAN), 0.0);
    }

    #[tokio::test]
    async fn budget_snapshot_reports_not_reported_without_limits() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("state.sqlite");

        let outcome = build_budget_snapshot_items(Some(&db_path), None).await;

        assert!(outcome.items.is_empty());
        assert!(outcome
            .notes
            .iter()
            .any(|n| n.contains("budget.daily: not_reported")));
        assert!(outcome
            .notes
            .iter()
            .any(|n| n.contains("budget.monthly: not_reported")));
    }

    #[tokio::test]
    async fn budget_snapshot_computes_percent_against_configured_limits() {
        use crate::state::{open, BudgetSpendInput};

        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("state.sqlite");
        {
            let store = open(Some(&db_path)).expect("open state");
            store
                .record_budget_spend(&BudgetSpendInput {
                    correlation_id: "corr-1".to_string(),
                    agent_id: "agy".to_string(),
                    model_id: "gemini-3.1-pro-high".to_string(),
                    task_kind: "architecture".to_string(),
                    cost_usd: 2.5,
                    created_at_unix: None,
                })
                .expect("record spend");
        }

        let mut budget_config_path = dir.path().to_path_buf();
        budget_config_path.push("budget.json");
        tokio::fs::write(
            &budget_config_path,
            r#"{"schema_version":1,"currency":"USD","daily_limit_usd":5.0,"monthly_limit_usd":50.0}"#,
        )
        .await
        .expect("write budget config");

        let outcome = build_budget_snapshot_items(Some(&db_path), Some(&budget_config_path)).await;

        assert_eq!(outcome.items.len(), 2);
        assert!(outcome.notes.is_empty());

        let daily = outcome
            .items
            .iter()
            .find(|i| i.model_group == "daily")
            .expect("daily item");
        assert_eq!(daily.agent, "orq");
        assert_eq!(daily.provider_group, "budget");
        assert_eq!(daily.window, "1d");
        assert_eq!(daily.used_percent, Some(50.0));
        assert_eq!(daily.remaining_percent, Some(50.0));

        let monthly = outcome
            .items
            .iter()
            .find(|i| i.model_group == "monthly")
            .expect("monthly item");
        assert_eq!(monthly.window, "30d");
        assert_eq!(monthly.used_percent, Some(5.0));
        assert_eq!(monthly.remaining_percent, Some(95.0));
    }

    #[test]
    fn capacity_snapshot_item_serializes_with_go_compatible_field_names() {
        let item = CapacitySnapshotItem {
            agent: "orq".to_string(),
            provider_group: "budget".to_string(),
            model_group: "daily".to_string(),
            remaining_percent: Some(50.0),
            used_percent: Some(50.0),
            window: "1d".to_string(),
            resets_at: None,
            source: "agent-orchestrator".to_string(),
            captured_at: Some("2026-09-10T00:00:00Z".to_string()),
        };
        let json = serde_json::to_string(&item).unwrap();
        // Nombres de campo deben coincidir exacto con postgres.CapacitySnapshotInput
        // (observer-llm): agent, provider_group, model_group, remaining_percent,
        // used_percent, window, resets_at, source, captured_at.
        assert!(json.contains("\"agent\":\"orq\""));
        assert!(json.contains("\"provider_group\":\"budget\""));
        assert!(json.contains("\"model_group\":\"daily\""));
        assert!(json.contains("\"remaining_percent\":50.0"));
        assert!(json.contains("\"used_percent\":50.0"));
        assert!(json.contains("\"window\":\"1d\""));
        assert!(!json.contains("resets_at"));
        assert!(json.contains("\"source\":\"agent-orchestrator\""));
        assert!(json.contains("\"captured_at\":\"2026-09-10T00:00:00Z\""));
    }

    #[tokio::test]
    async fn rtk_snapshot_skip_flag_reports_not_reported() {
        let (item, note) = build_rtk_snapshot_item(true).await;
        assert!(item.is_none());
        assert_eq!(note, "rtk: not_reported (--skip-rtk)");
    }

    #[tokio::test]
    async fn rtk_snapshot_missing_binary_reports_not_reported() {
        let _guard = TEST_ENV_MUTEX.lock().await;
        let original_path = std::env::var("PATH").ok();
        std::env::set_var("PATH", "/definitely-nonexistent-rtk-test-path");

        let (item, note) = build_rtk_snapshot_item(false).await;

        if let Some(path) = original_path {
            std::env::set_var("PATH", path);
        } else {
            std::env::remove_var("PATH");
        }

        assert!(item.is_none());
        assert!(note.contains("rtk no encontrado en PATH"));
    }

    #[tokio::test]
    async fn snapshot_dry_run_never_touches_network_and_reports_metrics() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("state.sqlite");
        let budget_config_path = dir.path().join("budget.json");
        tokio::fs::write(
            &budget_config_path,
            r#"{"schema_version":1,"currency":"USD","daily_limit_usd":5.0,"monthly_limit_usd":50.0}"#,
        )
        .await
        .expect("write budget config");

        let report = snapshot(ObserverSnapshotArgs {
            endpoint: Some("https://observer.invalid.example".to_string()),
            token_file: None,
            dry_run: true,
            output: None,
            db_path: Some(db_path.display().to_string()),
            budget_config: Some(budget_config_path.display().to_string()),
            skip_rtk: true,
        })
        .await
        .expect("dry run snapshot");

        assert_eq!(report.status, "dry_run");
        assert_eq!(report.snapshots.len(), 2);
        assert!(!report.secrets_read);
        assert!(report.notes.iter().any(|n| n.contains("--skip-rtk")));
    }

    #[tokio::test]
    async fn snapshot_skips_send_when_no_metrics_available() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("state.sqlite");

        let report = snapshot(ObserverSnapshotArgs {
            endpoint: Some("https://observer.invalid.example".to_string()),
            token_file: None,
            dry_run: false,
            output: None,
            db_path: Some(db_path.display().to_string()),
            budget_config: None,
            skip_rtk: true,
        })
        .await
        .expect("snapshot without metrics");

        assert_eq!(report.status, "skipped_no_metrics");
        assert!(report.snapshots.is_empty());
    }
}
