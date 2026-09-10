//! Event DB local auditable para delegaciones de `orq` (issue #172).
//!
//! Persiste en JSONL append-only cada transición de estado de `orq delegate`
//! (planificado, bloqueado, ejecutado, validado, fallido, timeout) y expone un
//! watchdog anti-loop minimo que impide reintentos peligrosos sobre el mismo
//! `task_id`. Es la base auditable para reroute adaptivo (#183/#217/#187); no
//! implementa el reroute en si.

use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use color_eyre::eyre::{eyre, Result, WrapErr};
use serde::{Deserialize, Serialize};

use crate::receipt::{DelegateReceipt, DelegateStatus, FailureClass};

pub const EVENT_LOG_PATH_ENV: &str = "ORQ_EVENT_LOG_PATH";
pub const EVENT_LOG_SCHEMA_VERSION: u8 = 1;

/// Ventana y umbral por defecto del watchdog anti-loop: >= N eventos
/// fallidos/bloqueados/timeout para el mismo `task_id` dentro de la ventana
/// bloquea el siguiente intento.
pub const DEFAULT_LOOP_WINDOW_SECONDS: u64 = 600;
pub const DEFAULT_LOOP_THRESHOLD: usize = 3;
pub const LOOP_DETECTED_REASON_PREFIX: &str = "loop_detected";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    DelegationPlanned,
    DelegationCommandGenerated,
    DelegationBlocked,
    DelegationExecuted,
    DelegationValidated,
    DelegationFailed,
    DelegationTimeout,
    WatchdogLoopDetected,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct OrqEvent {
    pub schema_version: u8,
    pub event_kind: EventKind,
    pub timestamp_unix: u64,
    pub correlation_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    pub agent: String,
    pub model: String,
    pub status: String,
    pub verdict: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_class: Option<FailureClass>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
}

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Resuelve la ruta del event log: arg explicito > `ORQ_EVENT_LOG_PATH` >
/// default seguro bajo `$HOME/.local/state/orq-agent/events.jsonl`.
pub fn resolve_log_path(explicit: Option<&str>) -> Result<PathBuf> {
    if let Some(p) = explicit {
        let trimmed = p.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }
    if let Ok(env_path) = std::env::var(EVENT_LOG_PATH_ENV) {
        let trimmed = env_path.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }
    let home = std::env::var_os("HOME")
        .ok_or_else(|| eyre!("HOME is not set; set {EVENT_LOG_PATH_ENV} explicitly"))?;
    Ok(PathBuf::from(home)
        .join(".local")
        .join("state")
        .join("orq-agent")
        .join("events.jsonl"))
}

fn event_kind_for(status: &DelegateStatus, reason: Option<&str>) -> EventKind {
    if reason
        .map(|r| r.starts_with(LOOP_DETECTED_REASON_PREFIX))
        .unwrap_or(false)
    {
        return EventKind::WatchdogLoopDetected;
    }
    match status {
        DelegateStatus::Planned => EventKind::DelegationPlanned,
        DelegateStatus::CommandGenerated => EventKind::DelegationCommandGenerated,
        DelegateStatus::Blocked => EventKind::DelegationBlocked,
        DelegateStatus::Executed => EventKind::DelegationExecuted,
        DelegateStatus::Validated => EventKind::DelegationValidated,
        DelegateStatus::Failed => {
            if reason == Some("timeout_sin_evidencia") {
                EventKind::DelegationTimeout
            } else {
                EventKind::DelegationFailed
            }
        }
    }
}

/// Construye el evento auditable a partir de un `DelegateReceipt` ya emitido.
/// Nunca incluye secretos: solo reusa los campos ya saneados del recibo
/// (`stdout_tail`/`stderr_tail` quedan fuera deliberadamente).
pub fn event_from_receipt(receipt: &DelegateReceipt) -> OrqEvent {
    let status_label = serde_json::to_value(&receipt.status)
        .ok()
        .and_then(|v| v.as_str().map(ToString::to_string))
        .unwrap_or_else(|| "unknown".to_string());
    let verdict_label = serde_json::to_value(&receipt.verdict)
        .ok()
        .and_then(|v| v.as_str().map(ToString::to_string))
        .unwrap_or_else(|| "indeterminado".to_string());
    OrqEvent {
        schema_version: EVENT_LOG_SCHEMA_VERSION,
        event_kind: event_kind_for(&receipt.status, receipt.reason.as_deref()),
        timestamp_unix: now_unix(),
        correlation_id: receipt.correlation_id.clone(),
        task_id: receipt.task_id.clone(),
        agent: receipt.agent.clone(),
        model: receipt.model.clone(),
        status: status_label,
        verdict: verdict_label,
        failure_class: receipt.failure_class,
        reason: receipt.reason.clone(),
        exit_code: receipt.exit_code,
    }
}

#[cfg(unix)]
fn secure_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(path)
        .wrap_err_with(|| format!("reading metadata for {}", path.display()))?
        .permissions();
    perms.set_mode(0o600);
    fs::set_permissions(path, perms)
        .wrap_err_with(|| format!("securing permissions for {}", path.display()))
}

#[cfg(not(unix))]
fn secure_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

/// Agrega un evento al event log JSONL append-only. Crea el directorio padre
/// y fija permisos 0600 en la creacion del archivo.
pub fn append_event(path: &Path, event: &OrqEvent) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .wrap_err_with(|| format!("creating directory for {}", parent.display()))?;
        }
    }
    let is_new = !path.exists();
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .wrap_err_with(|| format!("opening event log at {}", path.display()))?;
    if is_new {
        secure_permissions(path)?;
    }
    let line = serde_json::to_string(event).wrap_err("serializing event")?;
    writeln!(file, "{line}").wrap_err_with(|| format!("appending event to {}", path.display()))?;
    Ok(())
}

/// Lee todos los eventos del log. Lineas corruptas se saltan con warning en
/// stderr (no rompen la lectura completa); un log inexistente es una lista
/// vacia, no un error.
pub fn read_events(path: &Path) -> Result<Vec<OrqEvent>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let file = fs::File::open(path)
        .wrap_err_with(|| format!("opening event log at {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut events = Vec::new();
    for (idx, line) in reader.lines().enumerate() {
        let line =
            line.wrap_err_with(|| format!("reading line {} of {}", idx + 1, path.display()))?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match serde_json::from_str::<OrqEvent>(trimmed) {
            Ok(event) => events.push(event),
            Err(err) => {
                eprintln!(
                    "warning: skipping malformed event log line {} in {}: {err}",
                    idx + 1,
                    path.display()
                );
            }
        }
    }
    Ok(events)
}

/// Watchdog anti-loop minimo: cuenta eventos fallidos/bloqueados/timeout con
/// el mismo `task_id` dentro de la ventana dada. Solo aplica cuando hay
/// `task_id` explicito (sin el, no hay clave estable para agrupar reintentos
/// sin producir falsos positivos entre tareas distintas del mismo agente).
pub fn detect_loop(
    events: &[OrqEvent],
    task_id: &str,
    now_unix: u64,
    window_seconds: u64,
    threshold: usize,
) -> bool {
    let window_start = now_unix.saturating_sub(window_seconds);
    let count = events
        .iter()
        .filter(|e| {
            e.task_id.as_deref() == Some(task_id)
                && e.timestamp_unix >= window_start
                && matches!(
                    e.event_kind,
                    EventKind::DelegationBlocked
                        | EventKind::DelegationFailed
                        | EventKind::DelegationTimeout
                        | EventKind::WatchdogLoopDetected
                )
        })
        .count();
    count >= threshold
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn sample_event(task_id: &str, kind: EventKind, timestamp_unix: u64) -> OrqEvent {
        OrqEvent {
            schema_version: EVENT_LOG_SCHEMA_VERSION,
            event_kind: kind,
            timestamp_unix,
            correlation_id: format!("corr-{timestamp_unix}"),
            task_id: Some(task_id.to_string()),
            agent: "qwen-code".to_string(),
            model: "qwen3.6-flash".to_string(),
            status: "failed".to_string(),
            verdict: "non_util".to_string(),
            failure_class: Some(FailureClass::AgentCrash),
            reason: Some("exit_non_zero".to_string()),
            exit_code: Some(1),
        }
    }

    #[test]
    fn append_and_read_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("events.jsonl");

        let e1 = sample_event("task-1", EventKind::DelegationFailed, 1000);
        let e2 = sample_event("task-1", EventKind::DelegationValidated, 1010);
        append_event(&path, &e1).unwrap();
        append_event(&path, &e2).unwrap();

        let events = read_events(&path).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0], e1);
        assert_eq!(events[1], e2);
    }

    #[test]
    fn read_events_missing_file_is_empty() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("does-not-exist.jsonl");
        let events = read_events(&path).unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn read_events_skips_malformed_lines() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        let good = sample_event("task-1", EventKind::DelegationFailed, 1000);
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .unwrap();
        writeln!(file, "{}", serde_json::to_string(&good).unwrap()).unwrap();
        writeln!(file, "{{not valid json").unwrap();
        writeln!(file, "{}", serde_json::to_string(&good).unwrap()).unwrap();
        drop(file);

        let events = read_events(&path).unwrap();
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn append_event_does_not_serialize_secrets_fields() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        let event = sample_event("task-1", EventKind::DelegationBlocked, 1000);
        append_event(&path, &event).unwrap();
        let raw = fs::read_to_string(&path).unwrap();
        assert!(!raw.contains("stdout_tail"));
        assert!(!raw.contains("stderr_tail"));
        assert!(!raw.contains("token"));
        assert!(!raw.contains("secret"));
    }

    #[cfg(unix)]
    #[test]
    fn append_event_sets_secure_permissions_on_create() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        let event = sample_event("task-1", EventKind::DelegationValidated, 1000);
        append_event(&path, &event).unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[test]
    fn event_from_receipt_maps_blocked_status() {
        let receipt = DelegateReceipt {
            schema_version: 1,
            correlation_id: "corr-1".to_string(),
            agent: "claude-code".to_string(),
            model: "claude-opus-5".to_string(),
            command: vec![],
            status: DelegateStatus::Blocked,
            reason: Some("approval_required".to_string()),
            policy_source: "builtin".to_string(),
            policy_path: "builtin".to_string(),
            policy_sha256: "sha".to_string(),
            verdict: crate::receipt::DelegateVerdict::NonUtil,
            evidence: "none".to_string(),
            stdout_tail: String::new(),
            stderr_tail: String::new(),
            started_at_unix: 1,
            duration_ms: 2,
            timeout_seconds: 3,
            exit_code: None,
            secrets_read: false,
            cleanup_attempted: false,
            cleanup_succeeded: false,
            task_id: Some("task-9".to_string()),
            failure_class: None,
            fallback_agent: None,
            fallback_model: None,
            fallback_reason: None,
            fallback_attempts: Vec::new(),
        };
        let event = event_from_receipt(&receipt);
        assert_eq!(event.event_kind, EventKind::DelegationBlocked);
        assert_eq!(event.status, "blocked");
        assert_eq!(event.task_id.as_deref(), Some("task-9"));
    }

    #[test]
    fn event_from_receipt_maps_loop_detected_reason() {
        let mut receipt_status = DelegateStatus::Blocked;
        let reason = Some(format!(
            "{LOOP_DETECTED_REASON_PREFIX}: task-9 exceeded threshold"
        ));
        assert_eq!(
            event_kind_for(&receipt_status, reason.as_deref()),
            EventKind::WatchdogLoopDetected
        );
        receipt_status = DelegateStatus::Failed;
        assert_eq!(
            event_kind_for(&receipt_status, reason.as_deref()),
            EventKind::WatchdogLoopDetected
        );
    }

    #[test]
    fn detect_loop_does_not_trigger_below_threshold() {
        let events = vec![
            sample_event("task-1", EventKind::DelegationFailed, 1000),
            sample_event("task-1", EventKind::DelegationFailed, 1010),
        ];
        assert!(!detect_loop(&events, "task-1", 1020, 600, 3));
    }

    #[test]
    fn detect_loop_triggers_at_threshold_within_window() {
        let events = vec![
            sample_event("task-1", EventKind::DelegationFailed, 1000),
            sample_event("task-1", EventKind::DelegationBlocked, 1010),
            sample_event("task-1", EventKind::DelegationTimeout, 1020),
        ];
        assert!(detect_loop(&events, "task-1", 1030, 600, 3));
    }

    #[test]
    fn detect_loop_ignores_events_outside_window() {
        let events = vec![
            sample_event("task-1", EventKind::DelegationFailed, 1000),
            sample_event("task-1", EventKind::DelegationFailed, 1010),
            sample_event("task-1", EventKind::DelegationFailed, 1020),
        ];
        // now_unix muy adelante: la ventana de 600s ya no cubre estos eventos.
        assert!(!detect_loop(&events, "task-1", 100_000, 600, 3));
    }

    #[test]
    fn detect_loop_ignores_other_task_ids_and_successes() {
        let events = vec![
            sample_event("task-1", EventKind::DelegationFailed, 1000),
            sample_event("task-2", EventKind::DelegationFailed, 1005),
            sample_event("task-1", EventKind::DelegationValidated, 1010),
        ];
        assert!(!detect_loop(&events, "task-1", 1020, 600, 3));
    }

    #[test]
    fn resolve_log_path_explicit_arg_wins() {
        let resolved = resolve_log_path(Some("/custom/events.jsonl")).unwrap();
        assert_eq!(resolved, PathBuf::from("/custom/events.jsonl"));
    }
}
