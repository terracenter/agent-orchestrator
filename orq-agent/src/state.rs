use crate::receipt::{delegate_receipt_sha256, receipt_sha256, DelegateReceipt, ExecReceipt};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

pub const STATE_DB_ENV: &str = "ORQ_STATE_DB";
const LATEST_SCHEMA_VERSION: i64 = 5;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("{0}")]
    Config(String),
    #[error("filesystem error at {path}: {source}")]
    Fs {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("sqlite error at {context}: {source}")]
    Sqlite {
        context: &'static str,
        #[source]
        source: rusqlite::Error,
    },
    #[error("serialization error at {context}: {source}")]
    Serialization {
        context: &'static str,
        #[source]
        source: serde_json::Error,
    },
}

pub type Result<T> = std::result::Result<T, StoreError>;

#[derive(Debug, Clone, Serialize)]
pub struct StateStatus {
    pub path: String,
    pub schema_version: i64,
    pub latest_migration: i64,
    pub tables_present: Vec<String>,
    pub migrations_applied: Vec<i64>,
    pub secrets_read: bool,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct AgentRecord {
    pub agent_id: String,
    pub display_name: String,
    pub adapter_status: String,
    pub metadata_json: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ModelRecord {
    pub agent_id: String,
    pub model_id: String,
    pub task_kind: String,
    pub gated: bool,
    pub active: bool,
    pub metadata_json: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakerOutcome {
    Success,
    TimedOut,
    #[allow(dead_code)]
    CommandAborted,
    Auth401,
    AdapterError,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredReceipt {
    pub correlation_id: String,
    pub agent_id: String,
    pub model_id: String,
    pub task_kind: String,
    pub status: String,
    pub duration_ms: u128,
    pub secrets_read: bool,
    pub receipt_hash: String,
    pub receipt: ExecReceipt,
    pub created_at_unix: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CircuitBreakerRecord {
    pub agent_id: String,
    pub model_id: String,
    pub state: String,
    pub failure_streak: i64,
    pub opened_until_unix: Option<u64>,
    pub updated_at_unix: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuotaSnapshotRecord {
    pub id: i64,
    pub provider: String,
    pub scope: String,
    pub remaining_pct: Option<f64>,
    pub used_pct: Option<f64>,
    pub status: String,
    pub reset_at_unix: Option<u64>,
    pub captured_at_unix: u64,
    pub metadata_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuotaSnapshotInput {
    pub provider: String,
    pub scope: String,
    pub remaining_pct: Option<f64>,
    pub used_pct: Option<f64>,
    pub status: Option<String>,
    pub reset_at_unix: Option<u64>,
    pub captured_at_unix: Option<u64>,
    pub metadata_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EmpiricalRecord {
    pub id: i64,
    pub correlation_id: String,
    pub user_id: String,
    pub repo: String,
    pub language_stack: String,
    pub task_type: String,
    pub risk_level: String,
    pub agent_id: String,
    pub provider_id: String,
    pub model_id: String,
    pub mode: String,
    pub timestamp_bucket: String,
    pub s_rec: f64,
    pub c_law: f64,
    pub q_tech: f64,
    pub d_doc: f64,
    pub e_cost: f64,
    pub h_inv: f64,
    pub penalties: f64,
    pub score: f64,
    pub created_at_unix: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EmpiricalRecordInput {
    pub correlation_id: String,
    pub user_id: String,
    pub repo: String,
    pub language_stack: String,
    pub task_type: String,
    pub risk_level: String,
    pub agent_id: String,
    pub provider_id: String,
    pub model_id: String,
    pub mode: String,
    pub timestamp_bucket: String,
    pub s_rec: f64,
    pub c_law: f64,
    pub q_tech: f64,
    pub d_doc: f64,
    pub e_cost: f64,
    pub h_inv: f64,
    pub penalties: f64,
    pub score: f64,
    pub created_at_unix: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EmpiricalHistoryFilter {
    pub agent_id: Option<String>,
    pub model_id: Option<String>,
    pub repo: Option<String>,
    pub task_type: Option<String>,
    pub user_id: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AggregatedScore {
    pub agent_id: String,
    pub model_id: String,
    pub repo: Option<String>,
    pub task_type: Option<String>,
    pub sample_count: usize,
    pub mean_score: f64,
    pub decay_weighted_score: f64,
    pub mean_s_rec: f64,
    pub mean_c_law: f64,
    pub mean_q_tech: f64,
    pub mean_d_doc: f64,
    pub mean_e_cost: f64,
    pub mean_h_inv: f64,
    pub mean_penalties: f64,
    pub latest_timestamp_unix: Option<u64>,
    pub decay_half_life_days: f64,
}

pub fn default_db_path() -> Result<PathBuf> {
    if let Some(value) = std::env::var_os(STATE_DB_ENV) {
        return Ok(PathBuf::from(value));
    }
    let home = std::env::var_os("HOME").ok_or_else(|| {
        StoreError::Config("HOME is not set; set ORQ_STATE_DB explicitly".to_string())
    })?;
    Ok(PathBuf::from(home)
        .join(".local")
        .join("state")
        .join("orq-agent")
        .join("orq-state.sqlite"))
}

pub fn open(path: Option<&Path>) -> Result<StateStore> {
    let path = match path {
        Some(path) => path.to_path_buf(),
        None => default_db_path()?,
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| StoreError::Fs {
            path: parent.display().to_string(),
            source,
        })?;
    }
    let conn = Connection::open(&path).map_err(|source| StoreError::Sqlite {
        context: "open state database",
        source,
    })?;
    configure(&conn)?;
    secure_permissions(&path)?;
    let store = StateStore { conn, path };
    store.migrate()?;
    Ok(store)
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn backoff_seconds(outcome: BreakerOutcome, failure_streak: i64) -> u64 {
    let base = match outcome {
        BreakerOutcome::Success => 0,
        BreakerOutcome::TimedOut | BreakerOutcome::CommandAborted => 60,
        BreakerOutcome::Auth401 => 3600,
        BreakerOutcome::AdapterError => 300,
    };
    let exponent = failure_streak.clamp(1, 6) as u32 - 1;
    base * 2_u64.saturating_pow(exponent)
}

fn configure(conn: &Connection) -> Result<()> {
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(|source| StoreError::Sqlite {
            context: "set journal_mode",
            source,
        })?;
    conn.pragma_update(None, "busy_timeout", 5000)
        .map_err(|source| StoreError::Sqlite {
            context: "set busy_timeout",
            source,
        })?;
    conn.pragma_update(None, "foreign_keys", "ON")
        .map_err(|source| StoreError::Sqlite {
            context: "set foreign_keys",
            source,
        })?;
    conn.pragma_update(None, "synchronous", "NORMAL")
        .map_err(|source| StoreError::Sqlite {
            context: "set synchronous",
            source,
        })?;
    Ok(())
}

#[cfg(unix)]
fn secure_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = std::fs::metadata(path)
        .map_err(|source| StoreError::Fs {
            path: path.display().to_string(),
            source,
        })?
        .permissions();
    permissions.set_mode(0o600);
    std::fs::set_permissions(path, permissions).map_err(|source| StoreError::Fs {
        path: path.display().to_string(),
        source,
    })
}

#[cfg(not(unix))]
fn secure_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

pub struct StateStore {
    conn: Connection,
    path: PathBuf,
}

impl StateStore {
    pub fn migrate(&self) -> Result<()> {
        self.conn
            .execute_batch(INITIAL_MIGRATION)
            .map_err(|source| StoreError::Sqlite {
                context: "apply initial migration",
                source,
            })?;
        self.conn
            .execute(
                "INSERT OR IGNORE INTO schema_migrations(version, applied_at_unix) VALUES (1, strftime('%s','now'))",
                [],
            )
            .map_err(|source| StoreError::Sqlite {
                context: "record initial migration",
                source,
            })?;
        self.ensure_receipts_hash_column()?;
        self.conn
            .execute(
                "INSERT OR IGNORE INTO schema_migrations(version, applied_at_unix) VALUES (2, strftime('%s','now'))",
                [],
            )
            .map_err(|source| StoreError::Sqlite {
                context: "record migration 2",
                source,
            })?;
        self.conn
            .execute(
                "INSERT OR IGNORE INTO schema_migrations(version, applied_at_unix) VALUES (3, strftime('%s','now'))",
                [],
            )
            .map_err(|source| StoreError::Sqlite { context: "record migration 3", source })?;
        self.conn
            .execute_batch(MIGRATION_V4)
            .map_err(|source| StoreError::Sqlite {
                context: "apply migration 4 (delegate_receipts)",
                source,
            })?;
        self.conn
            .execute(
                "INSERT OR IGNORE INTO schema_migrations(version, applied_at_unix) VALUES (4, strftime('%s','now'))",
                [],
            )
            .map_err(|source| StoreError::Sqlite { context: "record migration 4", source })?;
        self.conn
            .execute_batch(MIGRATION_V5)
            .map_err(|source| StoreError::Sqlite {
                context: "apply migration 5 (empirical_history)",
                source,
            })?;
        self.conn
            .execute(
                "INSERT OR IGNORE INTO schema_migrations(version, applied_at_unix) VALUES (?1, strftime('%s','now'))",
                params![LATEST_SCHEMA_VERSION],
            )
            .map_err(|source| StoreError::Sqlite { context: "record migration 5", source })?;
        Ok(())
    }

    fn ensure_receipts_hash_column(&self) -> Result<()> {
        if self.table_has_column("receipts", "receipt_hash")? {
            return Ok(());
        }
        self.conn
            .execute(
                "ALTER TABLE receipts ADD COLUMN receipt_hash TEXT NOT NULL DEFAULT ''",
                [],
            )
            .map_err(|source| StoreError::Sqlite {
                context: "add receipts receipt_hash column",
                source,
            })?;
        Ok(())
    }

    fn table_has_column(&self, table: &str, column: &str) -> Result<bool> {
        let quoted_table = quote_sql_identifier(table)?;
        let sql = format!("PRAGMA table_info({quoted_table})");
        let mut stmt = self
            .conn
            .prepare(&sql)
            .map_err(|source| StoreError::Sqlite {
                context: "prepare table info query",
                source,
            })?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .map_err(|source| StoreError::Sqlite {
                context: "query table info",
                source,
            })?;
        for row in rows {
            let name = row.map_err(|source| StoreError::Sqlite {
                context: "read table info row",
                source,
            })?;
            if name == column {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn status(&self) -> Result<StateStatus> {
        Ok(StateStatus {
            path: self.path.display().to_string(),
            schema_version: self.schema_version()?,
            latest_migration: LATEST_SCHEMA_VERSION,
            tables_present: self.tables_present()?,
            migrations_applied: self.migrations_applied()?,
            secrets_read: false,
        })
    }

    #[allow(dead_code)]
    pub fn insert_receipt(&self, receipt: &ExecReceipt, task_kind: &str) -> Result<StoredReceipt> {
        let receipt_hash = receipt_sha256(receipt)
            .map_err(|error| StoreError::Config(format!("hash receipt: {error}")))?;
        let receipt_json =
            serde_json::to_string(receipt).map_err(|source| StoreError::Serialization {
                context: "serialize receipt",
                source,
            })?;
        let duration_ms = i64::try_from(receipt.duration_ms).map_err(|_| {
            StoreError::Config("receipt duration_ms exceeds SQLite INTEGER range".to_string())
        })?;
        let created_at_unix = now_unix();
        let created_at_i64 = i64::try_from(created_at_unix).map_err(|_| {
            StoreError::Config("receipt created_at_unix exceeds SQLite INTEGER range".to_string())
        })?;
        let status = receipt_status_label(receipt)?;
        let secrets_read = i64::from(receipt.secrets_read);

        self.conn
            .execute(
                "INSERT INTO receipts(correlation_id, agent_id, model_id, task_kind, status,
                 duration_ms, secrets_read, receipt_json, created_at_unix, receipt_hash)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    receipt.correlation_id,
                    receipt.agent,
                    receipt.model,
                    task_kind,
                    status,
                    duration_ms,
                    secrets_read,
                    receipt_json,
                    created_at_i64,
                    receipt_hash,
                ],
            )
            .map_err(|source| StoreError::Sqlite {
                context: "insert receipt",
                source,
            })?;

        Ok(StoredReceipt {
            correlation_id: receipt.correlation_id.clone(),
            agent_id: receipt.agent.clone(),
            model_id: receipt.model.clone(),
            task_kind: task_kind.to_string(),
            status,
            duration_ms: receipt.duration_ms,
            secrets_read: receipt.secrets_read,
            receipt_hash,
            receipt: receipt.clone(),
            created_at_unix,
        })
    }

    #[allow(dead_code)]
    pub fn find_receipt(&self, correlation_id: &str) -> Result<Option<StoredReceipt>> {
        self.conn
            .query_row(
                "SELECT correlation_id, agent_id, model_id, task_kind, status, duration_ms,
                 secrets_read, receipt_json, created_at_unix, receipt_hash
                 FROM receipts WHERE correlation_id=?1",
                params![correlation_id],
                |row| {
                    let duration_ms_i64: i64 = row.get(5)?;
                    let created_at_i64: i64 = row.get(8)?;
                    let receipt_json: String = row.get(7)?;
                    let receipt = serde_json::from_str(&receipt_json).map_err(|source| {
                        rusqlite::Error::FromSqlConversionFailure(
                            7,
                            rusqlite::types::Type::Text,
                            Box::new(source),
                        )
                    })?;
                    let duration_ms = u128::try_from(duration_ms_i64).map_err(|source| {
                        rusqlite::Error::FromSqlConversionFailure(
                            5,
                            rusqlite::types::Type::Integer,
                            Box::new(source),
                        )
                    })?;
                    let created_at_unix = u64::try_from(created_at_i64).map_err(|source| {
                        rusqlite::Error::FromSqlConversionFailure(
                            8,
                            rusqlite::types::Type::Integer,
                            Box::new(source),
                        )
                    })?;
                    Ok(StoredReceipt {
                        correlation_id: row.get(0)?,
                        agent_id: row.get(1)?,
                        model_id: row.get(2)?,
                        task_kind: row.get(3)?,
                        status: row.get(4)?,
                        duration_ms,
                        secrets_read: row.get::<_, i64>(6)? != 0,
                        receipt,
                        created_at_unix,
                        receipt_hash: row.get(9)?,
                    })
                },
            )
            .optional()
            .map_err(|source| StoreError::Sqlite {
                context: "find receipt",
                source,
            })
    }

    pub fn insert_delegate_receipt(
        &self,
        receipt: &DelegateReceipt,
        task_kind: &str,
    ) -> Result<StoredReceipt> {
        let receipt_hash = delegate_receipt_sha256(receipt)
            .map_err(|error| StoreError::Config(format!("hash delegate receipt: {error}")))?;
        let receipt_json =
            serde_json::to_string(receipt).map_err(|source| StoreError::Serialization {
                context: "serialize delegate receipt",
                source,
            })?;
        let duration_ms = i64::try_from(receipt.duration_ms).map_err(|_| {
            StoreError::Config("receipt duration_ms exceeds SQLite INTEGER range".to_string())
        })?;
        let created_at_unix = now_unix();
        let created_at_i64 = i64::try_from(created_at_unix).map_err(|_| {
            StoreError::Config("receipt created_at_unix exceeds SQLite INTEGER range".to_string())
        })?;
        let status = serde_json::to_value(&receipt.status)
            .ok()
            .and_then(|v| v.as_str().map(ToString::to_string))
            .unwrap_or_else(|| "unknown".to_string());
        let verdict = serde_json::to_value(&receipt.verdict)
            .ok()
            .and_then(|v| v.as_str().map(ToString::to_string))
            .unwrap_or_else(|| "indeterminado".to_string());
        let secrets_read = i64::from(receipt.secrets_read);

        self.conn
            .execute(
                "INSERT INTO delegate_receipts(correlation_id, agent_id, model_id, task_kind, status,
                 verdict, evidence, reason, duration_ms, secrets_read, receipt_json, created_at_unix, receipt_hash)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![
                    receipt.correlation_id,
                    receipt.agent,
                    receipt.model,
                    task_kind,
                    status,
                    verdict,
                    receipt.evidence,
                    receipt.reason,
                    duration_ms,
                    secrets_read,
                    receipt_json,
                    created_at_i64,
                    receipt_hash,
                ],
            )
            .map_err(|source| StoreError::Sqlite {
                context: "insert delegate receipt",
                source,
            })?;

        // Also insert into receipts table for backwards compatibility
        let legacy_exec_status = match receipt.status {
            crate::receipt::DelegateStatus::Validated
            | crate::receipt::DelegateStatus::Executed => crate::receipt::ExecStatus::Succeeded,
            crate::receipt::DelegateStatus::Planned
            | crate::receipt::DelegateStatus::CommandGenerated => {
                crate::receipt::ExecStatus::Blocked
            }
            crate::receipt::DelegateStatus::Failed => {
                if receipt.reason.as_deref() == Some("timeout_sin_evidencia") {
                    crate::receipt::ExecStatus::TimedOut
                } else {
                    crate::receipt::ExecStatus::Failed
                }
            }
        };
        let legacy_status = match legacy_exec_status {
            crate::receipt::ExecStatus::Succeeded => "succeeded",
            crate::receipt::ExecStatus::Blocked => "blocked",
            crate::receipt::ExecStatus::TimedOut => "timed_out",
            crate::receipt::ExecStatus::Failed => "failed",
            crate::receipt::ExecStatus::SpawnFailed => "spawn_failed",
            crate::receipt::ExecStatus::InvalidRequest => "invalid_request",
        };
        let legacy_exec_receipt = ExecReceipt {
            schema_version: receipt.schema_version,
            correlation_id: receipt.correlation_id.clone(),
            agent: receipt.agent.clone(),
            model: receipt.model.clone(),
            command: receipt.command.clone(),
            status: legacy_exec_status,
            policy_reason: receipt.reason.clone().unwrap_or_default(),
            started_at_unix: receipt.started_at_unix,
            duration_ms: receipt.duration_ms,
            timeout_seconds: receipt.timeout_seconds,
            exit_code: receipt.exit_code,
            stdout_tail: receipt.stdout_tail.clone(),
            stderr_tail: receipt.stderr_tail.clone(),
            secrets_read: receipt.secrets_read,
            cleanup_attempted: false,
            cleanup_succeeded: false,
        };
        let legacy_hash = receipt_sha256(&legacy_exec_receipt)
            .map_err(|error| StoreError::Config(format!("hash legacy receipt: {error}")))?;
        let legacy_receipt_json =
            serde_json::to_string(&legacy_exec_receipt).map_err(|source| {
                StoreError::Serialization {
                    context: "serialize legacy exec receipt",
                    source,
                }
            })?;
        let _ = self.conn.execute(
            "INSERT OR REPLACE INTO receipts(correlation_id, agent_id, model_id, task_kind, status,
             duration_ms, secrets_read, receipt_json, created_at_unix, receipt_hash)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                receipt.correlation_id,
                receipt.agent,
                receipt.model,
                task_kind,
                legacy_status,
                duration_ms,
                secrets_read,
                legacy_receipt_json,
                created_at_i64,
                legacy_hash,
            ],
        );

        Ok(StoredReceipt {
            correlation_id: receipt.correlation_id.clone(),
            agent_id: receipt.agent.clone(),
            model_id: receipt.model.clone(),
            task_kind: task_kind.to_string(),
            status: legacy_status.to_string(),
            duration_ms: receipt.duration_ms,
            secrets_read: receipt.secrets_read,
            receipt_hash: legacy_hash,
            receipt: legacy_exec_receipt,
            created_at_unix,
        })
    }

    #[allow(dead_code)]
    pub fn find_delegate_receipt(&self, correlation_id: &str) -> Result<Option<DelegateReceipt>> {
        self.conn
            .query_row(
                "SELECT receipt_json FROM delegate_receipts WHERE correlation_id=?1",
                params![correlation_id],
                |row| {
                    let json: String = row.get(0)?;
                    let receipt = serde_json::from_str(&json).map_err(|source| {
                        rusqlite::Error::FromSqlConversionFailure(
                            0,
                            rusqlite::types::Type::Text,
                            Box::new(source),
                        )
                    })?;
                    Ok(receipt)
                },
            )
            .optional()
            .map_err(|source| StoreError::Sqlite {
                context: "find delegate receipt",
                source,
            })
    }

    pub fn list_delegate_receipts(&self) -> Result<Vec<DelegateReceipt>> {
        let mut stmt = self
            .conn
            .prepare("SELECT receipt_json FROM delegate_receipts ORDER BY created_at_unix ASC")
            .map_err(|source| StoreError::Sqlite {
                context: "prepare list delegate receipts",
                source,
            })?;
        let rows = stmt
            .query_map([], |row| {
                let json: String = row.get(0)?;
                let receipt = serde_json::from_str(&json).map_err(|source| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(source),
                    )
                })?;
                Ok(receipt)
            })
            .map_err(|source| StoreError::Sqlite {
                context: "query list delegate receipts",
                source,
            })?;
        collect_rows(rows, "list delegate receipts")
    }

    #[allow(dead_code)]
    pub fn upsert_agent(&self, agent: &AgentRecord) -> Result<()> {
        self.conn.execute(
            "INSERT INTO agents(agent_id, display_name, adapter_status, metadata_json, updated_at_unix)
             VALUES (?1, ?2, ?3, ?4, strftime('%s','now'))
             ON CONFLICT(agent_id) DO UPDATE SET display_name=excluded.display_name,
             adapter_status=excluded.adapter_status, metadata_json=excluded.metadata_json,
             updated_at_unix=excluded.updated_at_unix",
            params![agent.agent_id, agent.display_name, agent.adapter_status, agent.metadata_json],
        ).map_err(|source| StoreError::Sqlite { context: "upsert agent", source })?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn find_agent(&self, agent_id: &str) -> Result<Option<AgentRecord>> {
        self.conn
            .query_row(
                "SELECT agent_id, display_name, adapter_status, metadata_json FROM agents WHERE agent_id=?1",
                params![agent_id],
                |row| {
                    Ok(AgentRecord {
                        agent_id: row.get(0)?,
                        display_name: row.get(1)?,
                        adapter_status: row.get(2)?,
                        metadata_json: row.get(3)?,
                    })
                },
            )
            .optional()
            .map_err(|source| StoreError::Sqlite {
                context: "find agent",
                source,
            })
    }

    #[allow(dead_code)]
    pub fn upsert_model(&self, model: &ModelRecord) -> Result<()> {
        self.conn.execute(
            "INSERT INTO models(agent_id, model_id, task_kind, gated, active, metadata_json, updated_at_unix)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, strftime('%s','now'))
             ON CONFLICT(agent_id, model_id, task_kind) DO UPDATE SET gated=excluded.gated,
             active=excluded.active, metadata_json=excluded.metadata_json, updated_at_unix=excluded.updated_at_unix",
            params![model.agent_id, model.model_id, model.task_kind, model.gated as i64, model.active as i64, model.metadata_json],
        ).map_err(|source| StoreError::Sqlite { context: "upsert model", source })?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn record_breaker_outcome(
        &self,
        agent_id: &str,
        model_id: &str,
        outcome: BreakerOutcome,
    ) -> Result<()> {
        let now = now_unix();
        match outcome {
            BreakerOutcome::Success => {
                self.conn
                    .execute(
                        "INSERT INTO circuit_breakers(agent_id, model_id, state, failure_streak, opened_until_unix, updated_at_unix)
                         VALUES (?1, ?2, 'closed', 0, NULL, ?3)
                         ON CONFLICT(agent_id, model_id) DO UPDATE SET state='closed', failure_streak=0,
                         opened_until_unix=NULL, updated_at_unix=excluded.updated_at_unix",
                        params![agent_id, model_id, now],
                    )
                    .map_err(|source| StoreError::Sqlite {
                        context: "record successful breaker outcome",
                        source,
                    })?;
            }
            failure => {
                let current_streak = self
                    .breaker_record(agent_id, model_id)?
                    .map(|record| record.failure_streak)
                    .unwrap_or(0);
                let failure_streak = current_streak.saturating_add(1);
                let opened_until = now.saturating_add(backoff_seconds(failure, failure_streak));
                self.conn
                    .execute(
                        "INSERT INTO circuit_breakers(agent_id, model_id, state, failure_streak, opened_until_unix, updated_at_unix)
                         VALUES (?1, ?2, 'open', ?3, ?4, ?5)
                         ON CONFLICT(agent_id, model_id) DO UPDATE SET state='open',
                         failure_streak=excluded.failure_streak, opened_until_unix=excluded.opened_until_unix,
                         updated_at_unix=excluded.updated_at_unix",
                        params![agent_id, model_id, failure_streak, opened_until, now],
                    )
                    .map_err(|source| StoreError::Sqlite {
                        context: "record failed breaker outcome",
                        source,
                    })?;
            }
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub fn breaker_record(
        &self,
        agent_id: &str,
        model_id: &str,
    ) -> Result<Option<CircuitBreakerRecord>> {
        self.conn
            .query_row(
                "SELECT agent_id, model_id, state, failure_streak, opened_until_unix, updated_at_unix
                 FROM circuit_breakers WHERE agent_id=?1 AND model_id=?2",
                params![agent_id, model_id],
                |row| {
                    Ok(CircuitBreakerRecord {
                        agent_id: row.get(0)?,
                        model_id: row.get(1)?,
                        state: row.get(2)?,
                        failure_streak: row.get(3)?,
                        opened_until_unix: row.get(4)?,
                        updated_at_unix: row.get(5)?,
                    })
                },
            )
            .optional()
            .map_err(|source| StoreError::Sqlite {
                context: "read circuit breaker",
                source,
            })
    }

    #[allow(dead_code)]
    pub fn breaker_allows_model(&self, agent_id: &str, model_id: &str) -> Result<bool> {
        let Some(record) = self.breaker_record(agent_id, model_id)? else {
            return Ok(true);
        };
        if record.state != "open" {
            return Ok(true);
        }
        Ok(record
            .opened_until_unix
            .map(|opened_until| opened_until <= now_unix())
            .unwrap_or(true))
    }

    #[allow(dead_code)]
    pub fn find_model(
        &self,
        agent_id: &str,
        model_id: &str,
        task_kind: &str,
    ) -> Result<Option<ModelRecord>> {
        self.conn.query_row(
            "SELECT agent_id, model_id, task_kind, gated, active, metadata_json FROM models WHERE agent_id=?1 AND model_id=?2 AND task_kind=?3",
            params![agent_id, model_id, task_kind],
            |row| Ok(ModelRecord {
                agent_id: row.get(0)?,
                model_id: row.get(1)?,
                task_kind: row.get(2)?,
                gated: row.get::<_, i64>(3)? != 0,
                active: row.get::<_, i64>(4)? != 0,
                metadata_json: row.get(5)?,
            }),
        ).optional().map_err(|source| StoreError::Sqlite { context: "find model", source })
    }

    #[cfg(test)]
    pub fn force_open_breaker_until(
        &self,
        agent_id: &str,
        model_id: &str,
        opened_until_unix: u64,
    ) -> Result<()> {
        self.conn
            .execute(
                "INSERT INTO circuit_breakers(agent_id, model_id, state, failure_streak, opened_until_unix, updated_at_unix)
                 VALUES (?1, ?2, 'open', 1, ?3, ?3)
                 ON CONFLICT(agent_id, model_id) DO UPDATE SET state='open', failure_streak=1,
                 opened_until_unix=excluded.opened_until_unix, updated_at_unix=excluded.updated_at_unix",
                params![agent_id, model_id, opened_until_unix],
            )
            .map_err(|source| StoreError::Sqlite {
                context: "force open circuit breaker",
                source,
            })?;
        Ok(())
    }

    pub fn insert_quota_snapshot(&self, input: &QuotaSnapshotInput) -> Result<QuotaSnapshotRecord> {
        let captured_at_unix = input.captured_at_unix.unwrap_or_else(now_unix);
        let captured_at_i64 = i64::try_from(captured_at_unix).map_err(|_| {
            StoreError::Config("captured_at_unix exceeds SQLite INTEGER range".to_string())
        })?;
        let reset_at_i64 = match input.reset_at_unix {
            Some(val) => Some(i64::try_from(val).map_err(|_| {
                StoreError::Config("reset_at_unix exceeds SQLite INTEGER range".to_string())
            })?),
            None => None,
        };
        let provider = input.provider.trim().to_lowercase();
        let scope = input.scope.trim();
        let status = input.status.as_deref().unwrap_or("ok");
        let metadata_json = input.metadata_json.as_deref().unwrap_or("{}");

        self.conn
            .execute(
                "INSERT INTO quota_snapshots(provider, scope, remaining_pct, used_pct, status, reset_at_unix, captured_at_unix, metadata_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    provider,
                    scope,
                    input.remaining_pct,
                    input.used_pct,
                    status,
                    reset_at_i64,
                    captured_at_i64,
                    metadata_json,
                ],
            )
            .map_err(|source| StoreError::Sqlite {
                context: "insert quota snapshot",
                source,
            })?;

        let id = self.conn.last_insert_rowid();

        Ok(QuotaSnapshotRecord {
            id,
            provider,
            scope: scope.to_string(),
            remaining_pct: input.remaining_pct,
            used_pct: input.used_pct,
            status: status.to_string(),
            reset_at_unix: input.reset_at_unix,
            captured_at_unix,
            metadata_json: metadata_json.to_string(),
        })
    }

    pub fn latest_quota_snapshots(
        &self,
        provider_filter: Option<&str>,
    ) -> Result<Vec<QuotaSnapshotRecord>> {
        let filter_owned = provider_filter.map(|p| p.trim().to_lowercase());
        let (sql, filter_param) = if let Some(ref p) = filter_owned {
            (
                "SELECT qs.id, qs.provider, qs.scope, qs.remaining_pct, qs.used_pct, qs.status, qs.reset_at_unix, qs.captured_at_unix, qs.metadata_json
                 FROM quota_snapshots qs
                 WHERE qs.provider = ?1 AND qs.id = (
                     SELECT qs2.id FROM quota_snapshots qs2
                     WHERE qs2.provider = qs.provider AND qs2.scope = qs.scope
                     ORDER BY qs2.captured_at_unix DESC, qs2.id DESC
                     LIMIT 1
                 )
                 ORDER BY qs.scope ASC",
                Some(p.as_str()),
            )
        } else {
            (
                "SELECT qs.id, qs.provider, qs.scope, qs.remaining_pct, qs.used_pct, qs.status, qs.reset_at_unix, qs.captured_at_unix, qs.metadata_json
                 FROM quota_snapshots qs
                 WHERE qs.id = (
                     SELECT qs2.id FROM quota_snapshots qs2
                     WHERE qs2.provider = qs.provider AND qs2.scope = qs.scope
                     ORDER BY qs2.captured_at_unix DESC, qs2.id DESC
                     LIMIT 1
                 )
                 ORDER BY qs.provider ASC, qs.scope ASC",
                None,
            )
        };

        let mut stmt = self
            .conn
            .prepare(sql)
            .map_err(|source| StoreError::Sqlite {
                context: "prepare latest quota snapshots query",
                source,
            })?;

        let map_row = |row: &rusqlite::Row<'_>| -> rusqlite::Result<QuotaSnapshotRecord> {
            let id: i64 = row.get(0)?;
            let provider: String = row.get(1)?;
            let scope: String = row.get(2)?;
            let remaining_pct: Option<f64> = row.get(3)?;
            let used_pct: Option<f64> = row.get(4)?;
            let status: String = row.get(5)?;
            let reset_at_i64: Option<i64> = row.get(6)?;
            let captured_at_i64: i64 = row.get(7)?;
            let metadata_json: String = row.get(8)?;

            let reset_at_unix = match reset_at_i64 {
                Some(v) => Some(u64::try_from(v).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        6,
                        rusqlite::types::Type::Integer,
                        Box::new(e),
                    )
                })?),
                None => None,
            };
            let captured_at_unix = u64::try_from(captured_at_i64).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    7,
                    rusqlite::types::Type::Integer,
                    Box::new(e),
                )
            })?;

            Ok(QuotaSnapshotRecord {
                id,
                provider,
                scope,
                remaining_pct,
                used_pct,
                status,
                reset_at_unix,
                captured_at_unix,
                metadata_json,
            })
        };

        let rows = if let Some(p) = filter_param {
            stmt.query_map(params![p], map_row)
        } else {
            stmt.query_map([], map_row)
        }
        .map_err(|source| StoreError::Sqlite {
            context: "query latest quota snapshots",
            source,
        })?;

        collect_rows(rows, "read latest quota snapshots")
    }

    #[allow(dead_code)]
    pub fn all_quota_snapshots(
        &self,
        provider_filter: Option<&str>,
    ) -> Result<Vec<QuotaSnapshotRecord>> {
        let (sql, filter_param) = if let Some(p) = provider_filter {
            (
                "SELECT id, provider, scope, remaining_pct, used_pct, status, reset_at_unix, captured_at_unix, metadata_json
                 FROM quota_snapshots
                 WHERE provider = ?1
                 ORDER BY captured_at_unix DESC, id DESC",
                Some(p),
            )
        } else {
            (
                "SELECT id, provider, scope, remaining_pct, used_pct, status, reset_at_unix, captured_at_unix, metadata_json
                 FROM quota_snapshots
                 ORDER BY captured_at_unix DESC, id DESC",
                None,
            )
        };

        let mut stmt = self
            .conn
            .prepare(sql)
            .map_err(|source| StoreError::Sqlite {
                context: "prepare all quota snapshots query",
                source,
            })?;

        let map_row = |row: &rusqlite::Row<'_>| -> rusqlite::Result<QuotaSnapshotRecord> {
            let id: i64 = row.get(0)?;
            let provider: String = row.get(1)?;
            let scope: String = row.get(2)?;
            let remaining_pct: Option<f64> = row.get(3)?;
            let used_pct: Option<f64> = row.get(4)?;
            let status: String = row.get(5)?;
            let reset_at_i64: Option<i64> = row.get(6)?;
            let captured_at_i64: i64 = row.get(7)?;
            let metadata_json: String = row.get(8)?;

            let reset_at_unix = match reset_at_i64 {
                Some(v) => Some(u64::try_from(v).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        6,
                        rusqlite::types::Type::Integer,
                        Box::new(e),
                    )
                })?),
                None => None,
            };
            let captured_at_unix = u64::try_from(captured_at_i64).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    7,
                    rusqlite::types::Type::Integer,
                    Box::new(e),
                )
            })?;

            Ok(QuotaSnapshotRecord {
                id,
                provider,
                scope,
                remaining_pct,
                used_pct,
                status,
                reset_at_unix,
                captured_at_unix,
                metadata_json,
            })
        };

        let rows = if let Some(p) = filter_param {
            stmt.query_map(params![p], map_row)
        } else {
            stmt.query_map([], map_row)
        }
        .map_err(|source| StoreError::Sqlite {
            context: "query all quota snapshots",
            source,
        })?;

        collect_rows(rows, "read all quota snapshots")
    }

    pub fn insert_empirical_record(&self, input: &EmpiricalRecordInput) -> Result<EmpiricalRecord> {
        let created_at_unix = input.created_at_unix.unwrap_or_else(now_unix);
        let created_at_i64 = i64::try_from(created_at_unix).map_err(|_| {
            StoreError::Config("created_at_unix exceeds SQLite INTEGER range".to_string())
        })?;

        self.conn.execute(
            "INSERT INTO empirical_history(
                correlation_id, user_id, repo, language_stack, task_type, risk_level,
                agent_id, provider_id, model_id, mode, timestamp_bucket,
                s_rec, c_law, q_tech, d_doc, e_cost, h_inv, penalties, score, created_at_unix
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)
             ON CONFLICT(correlation_id) DO UPDATE SET
                user_id=excluded.user_id, repo=excluded.repo, language_stack=excluded.language_stack,
                task_type=excluded.task_type, risk_level=excluded.risk_level, agent_id=excluded.agent_id,
                provider_id=excluded.provider_id, model_id=excluded.model_id, mode=excluded.mode,
                timestamp_bucket=excluded.timestamp_bucket, s_rec=excluded.s_rec, c_law=excluded.c_law,
                q_tech=excluded.q_tech, d_doc=excluded.d_doc, e_cost=excluded.e_cost, h_inv=excluded.h_inv,
                penalties=excluded.penalties, score=excluded.score, created_at_unix=excluded.created_at_unix",
            params![
                input.correlation_id,
                input.user_id,
                input.repo,
                input.language_stack,
                input.task_type,
                input.risk_level,
                input.agent_id,
                input.provider_id,
                input.model_id,
                input.mode,
                input.timestamp_bucket,
                input.s_rec,
                input.c_law,
                input.q_tech,
                input.d_doc,
                input.e_cost,
                input.h_inv,
                input.penalties,
                input.score,
                created_at_i64,
            ],
        ).map_err(|source| StoreError::Sqlite { context: "insert empirical record", source })?;

        let id = self.conn.last_insert_rowid();

        Ok(EmpiricalRecord {
            id,
            correlation_id: input.correlation_id.clone(),
            user_id: input.user_id.clone(),
            repo: input.repo.clone(),
            language_stack: input.language_stack.clone(),
            task_type: input.task_type.clone(),
            risk_level: input.risk_level.clone(),
            agent_id: input.agent_id.clone(),
            provider_id: input.provider_id.clone(),
            model_id: input.model_id.clone(),
            mode: input.mode.clone(),
            timestamp_bucket: input.timestamp_bucket.clone(),
            s_rec: input.s_rec,
            c_law: input.c_law,
            q_tech: input.q_tech,
            d_doc: input.d_doc,
            e_cost: input.e_cost,
            h_inv: input.h_inv,
            penalties: input.penalties,
            score: input.score,
            created_at_unix,
        })
    }

    pub fn list_empirical_history(
        &self,
        filter: &EmpiricalHistoryFilter,
    ) -> Result<Vec<EmpiricalRecord>> {
        let mut sql = "SELECT id, correlation_id, user_id, repo, language_stack, task_type, risk_level, \
                       agent_id, provider_id, model_id, mode, timestamp_bucket, \
                       s_rec, c_law, q_tech, d_doc, e_cost, h_inv, penalties, score, created_at_unix \
                       FROM empirical_history WHERE 1=1".to_string();
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(ref a) = filter.agent_id {
            sql.push_str(" AND agent_id = ?");
            params_vec.push(Box::new(a.clone()));
        }
        if let Some(ref m) = filter.model_id {
            sql.push_str(" AND model_id = ?");
            params_vec.push(Box::new(m.clone()));
        }
        if let Some(ref r) = filter.repo {
            sql.push_str(" AND repo = ?");
            params_vec.push(Box::new(r.clone()));
        }
        if let Some(ref t) = filter.task_type {
            sql.push_str(" AND task_type = ?");
            params_vec.push(Box::new(t.clone()));
        }
        if let Some(ref u) = filter.user_id {
            sql.push_str(" AND user_id = ?");
            params_vec.push(Box::new(u.clone()));
        }

        sql.push_str(" ORDER BY created_at_unix DESC, id DESC");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {limit}"));
        }

        let mut stmt = self
            .conn
            .prepare(&sql)
            .map_err(|source| StoreError::Sqlite {
                context: "prepare list empirical history",
                source,
            })?;

        let rusqlite_params: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|b| b.as_ref()).collect();

        let rows = stmt
            .query_map(rusqlite_params.as_slice(), |row| {
                let created_at_i64: i64 = row.get(20)?;
                let created_at_unix = u64::try_from(created_at_i64).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        20,
                        rusqlite::types::Type::Integer,
                        Box::new(e),
                    )
                })?;
                Ok(EmpiricalRecord {
                    id: row.get(0)?,
                    correlation_id: row.get(1)?,
                    user_id: row.get(2)?,
                    repo: row.get(3)?,
                    language_stack: row.get(4)?,
                    task_type: row.get(5)?,
                    risk_level: row.get(6)?,
                    agent_id: row.get(7)?,
                    provider_id: row.get(8)?,
                    model_id: row.get(9)?,
                    mode: row.get(10)?,
                    timestamp_bucket: row.get(11)?,
                    s_rec: row.get(12)?,
                    c_law: row.get(13)?,
                    q_tech: row.get(14)?,
                    d_doc: row.get(15)?,
                    e_cost: row.get(16)?,
                    h_inv: row.get(17)?,
                    penalties: row.get(18)?,
                    score: row.get(19)?,
                    created_at_unix,
                })
            })
            .map_err(|source| StoreError::Sqlite {
                context: "query list empirical history",
                source,
            })?;

        collect_rows(rows, "read empirical history")
    }

    /// Computes the aggregate score for an agent/model pair with temporal decay.
    ///
    /// More recent records have higher weights according to the exponential half-life decay formula:
    /// `weight_i = 0.5 ^ (delta_t / half_life)`, where `half_life` is 7.0 days.
    ///
    /// Note: `route_scores` is a fast, derived routing matrix cache,
    /// whereas `empirical_history` represents the complete and auditable historical record.
    pub fn aggregate_scores(
        &self,
        agent_id: &str,
        model_id: &str,
        repo: Option<&str>,
        task_type: Option<&str>,
    ) -> Result<AggregatedScore> {
        let filter = EmpiricalHistoryFilter {
            agent_id: Some(agent_id.to_string()),
            model_id: Some(model_id.to_string()),
            repo: repo.map(ToString::to_string),
            task_type: task_type.map(ToString::to_string),
            user_id: None,
            limit: None,
        };

        let records = self.list_empirical_history(&filter)?;
        let half_life_days = 7.0;
        let half_life_secs = half_life_days * 86400.0;

        if records.is_empty() {
            return Ok(AggregatedScore {
                agent_id: agent_id.to_string(),
                model_id: model_id.to_string(),
                repo: repo.map(ToString::to_string),
                task_type: task_type.map(ToString::to_string),
                sample_count: 0,
                mean_score: 0.0,
                decay_weighted_score: 0.0,
                mean_s_rec: 0.0,
                mean_c_law: 0.0,
                mean_q_tech: 0.0,
                mean_d_doc: 0.0,
                mean_e_cost: 0.0,
                mean_h_inv: 0.0,
                mean_penalties: 0.0,
                latest_timestamp_unix: None,
                decay_half_life_days: half_life_days,
            });
        }

        let now = now_unix();
        let count = records.len();
        let mut total_score = 0.0;
        let mut total_s_rec = 0.0;
        let mut total_c_law = 0.0;
        let mut total_q_tech = 0.0;
        let mut total_d_doc = 0.0;
        let mut total_e_cost = 0.0;
        let mut total_h_inv = 0.0;
        let mut total_penalties = 0.0;

        let mut weighted_score_sum = 0.0;
        let mut total_weight = 0.0;
        let mut latest_ts: Option<u64> = None;

        for rec in &records {
            total_score += rec.score;
            total_s_rec += rec.s_rec;
            total_c_law += rec.c_law;
            total_q_tech += rec.q_tech;
            total_d_doc += rec.d_doc;
            total_e_cost += rec.e_cost;
            total_h_inv += rec.h_inv;
            total_penalties += rec.penalties;

            let delta_secs = if now >= rec.created_at_unix {
                (now - rec.created_at_unix) as f64
            } else {
                0.0
            };

            let weight = 0.5_f64.powf(delta_secs / half_life_secs);
            weighted_score_sum += weight * rec.score;
            total_weight += weight;

            if latest_ts.map(|t| rec.created_at_unix > t).unwrap_or(true) {
                latest_ts = Some(rec.created_at_unix);
            }
        }

        let decay_weighted_score = if total_weight > 0.0 {
            weighted_score_sum / total_weight
        } else {
            total_score / count as f64
        };

        Ok(AggregatedScore {
            agent_id: agent_id.to_string(),
            model_id: model_id.to_string(),
            repo: repo.map(ToString::to_string),
            task_type: task_type.map(ToString::to_string),
            sample_count: count,
            mean_score: total_score / count as f64,
            decay_weighted_score,
            mean_s_rec: total_s_rec / count as f64,
            mean_c_law: total_c_law / count as f64,
            mean_q_tech: total_q_tech / count as f64,
            mean_d_doc: total_d_doc / count as f64,
            mean_e_cost: total_e_cost / count as f64,
            mean_h_inv: total_h_inv / count as f64,
            mean_penalties: total_penalties / count as f64,
            latest_timestamp_unix: latest_ts,
            decay_half_life_days: half_life_days,
        })
    }

    fn schema_version(&self) -> Result<i64> {
        self.conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
                [],
                |row| row.get(0),
            )
            .map_err(|source| StoreError::Sqlite {
                context: "read schema version",
                source,
            })
    }

    fn migrations_applied(&self) -> Result<Vec<i64>> {
        let mut stmt = self
            .conn
            .prepare("SELECT version FROM schema_migrations ORDER BY version")
            .map_err(|source| StoreError::Sqlite {
                context: "prepare migrations query",
                source,
            })?;
        let rows = stmt
            .query_map([], |row| row.get(0))
            .map_err(|source| StoreError::Sqlite {
                context: "query migrations",
                source,
            })?;
        collect_rows(rows, "read migrations")
    }

    fn tables_present(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
            .map_err(|source| StoreError::Sqlite { context: "prepare tables query", source })?;
        let rows = stmt
            .query_map([], |row| row.get(0))
            .map_err(|source| StoreError::Sqlite {
                context: "query tables",
                source,
            })?;
        collect_rows(rows, "read tables")
    }
}

fn collect_rows<T>(
    rows: rusqlite::MappedRows<'_, impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>>,
    context: &'static str,
) -> Result<Vec<T>> {
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|source| StoreError::Sqlite { context, source })
}

fn receipt_status_label(receipt: &ExecReceipt) -> Result<String> {
    let value =
        serde_json::to_value(&receipt.status).map_err(|source| StoreError::Serialization {
            context: "serialize receipt status",
            source,
        })?;
    value
        .as_str()
        .map(ToString::to_string)
        .ok_or_else(|| StoreError::Config("receipt status did not serialize as string".to_string()))
}

fn quote_sql_identifier(identifier: &str) -> Result<String> {
    if identifier.is_empty()
        || !identifier
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        return Err(StoreError::Config(format!(
            "invalid sqlite identifier: {identifier}"
        )));
    }
    Ok(format!("\"{identifier}\""))
}

const INITIAL_MIGRATION: &str = r#"
CREATE TABLE IF NOT EXISTS schema_migrations (
    version INTEGER PRIMARY KEY,
    applied_at_unix INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS agents (
    agent_id TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    adapter_status TEXT NOT NULL,
    metadata_json TEXT NOT NULL DEFAULT '{}',
    updated_at_unix INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS task_kinds (
    task_kind TEXT PRIMARY KEY,
    description TEXT NOT NULL DEFAULT '',
    metadata_json TEXT NOT NULL DEFAULT '{}',
    updated_at_unix INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS models (
    agent_id TEXT NOT NULL,
    model_id TEXT NOT NULL,
    task_kind TEXT NOT NULL DEFAULT 'general',
    gated INTEGER NOT NULL DEFAULT 0,
    active INTEGER NOT NULL DEFAULT 1,
    metadata_json TEXT NOT NULL DEFAULT '{}',
    updated_at_unix INTEGER NOT NULL,
    PRIMARY KEY (agent_id, model_id, task_kind),
    FOREIGN KEY (agent_id) REFERENCES agents(agent_id) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS receipts (
    correlation_id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    model_id TEXT NOT NULL,
    task_kind TEXT NOT NULL,
    status TEXT NOT NULL,
    duration_ms INTEGER NOT NULL DEFAULT 0,
    secrets_read INTEGER NOT NULL DEFAULT 0,
    receipt_json TEXT NOT NULL,
    created_at_unix INTEGER NOT NULL,
    receipt_hash TEXT NOT NULL DEFAULT ''
);
CREATE TABLE IF NOT EXISTS certifications (
    certificate_id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    model_id TEXT NOT NULL,
    task_kind TEXT NOT NULL,
    status TEXT NOT NULL,
    receipt_hash TEXT NOT NULL,
    secrets_read INTEGER NOT NULL DEFAULT 0,
    created_at_unix INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS route_scores (
    agent_id TEXT NOT NULL,
    model_id TEXT NOT NULL,
    task_kind TEXT NOT NULL,
    success_count INTEGER NOT NULL DEFAULT 0,
    failure_count INTEGER NOT NULL DEFAULT 0,
    total_latency_ms INTEGER NOT NULL DEFAULT 0,
    updated_at_unix INTEGER NOT NULL,
    PRIMARY KEY (agent_id, model_id, task_kind)
);
CREATE TABLE IF NOT EXISTS circuit_breakers (
    agent_id TEXT NOT NULL,
    model_id TEXT NOT NULL,
    state TEXT NOT NULL,
    failure_streak INTEGER NOT NULL DEFAULT 0,
    opened_until_unix INTEGER,
    updated_at_unix INTEGER NOT NULL,
    PRIMARY KEY (agent_id, model_id)
);
CREATE TABLE IF NOT EXISTS quota_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    provider TEXT NOT NULL,
    scope TEXT NOT NULL,
    remaining_pct REAL,
    used_pct REAL,
    status TEXT NOT NULL DEFAULT 'ok',
    reset_at_unix INTEGER,
    captured_at_unix INTEGER NOT NULL,
    metadata_json TEXT NOT NULL DEFAULT '{}'
);
CREATE INDEX IF NOT EXISTS idx_models_agent_model_task ON models(agent_id, model_id, task_kind);
CREATE INDEX IF NOT EXISTS idx_receipts_agent_model_task ON receipts(agent_id, model_id, task_kind);
CREATE INDEX IF NOT EXISTS idx_certifications_agent_model_task ON certifications(agent_id, model_id, task_kind);
CREATE INDEX IF NOT EXISTS idx_route_scores_agent_model_task ON route_scores(agent_id, model_id, task_kind);
CREATE INDEX IF NOT EXISTS idx_quota_snapshots_provider_scope ON quota_snapshots(provider, scope, captured_at_unix DESC);
"#;

const MIGRATION_V4: &str = r#"
CREATE TABLE IF NOT EXISTS delegate_receipts (
    correlation_id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    model_id TEXT NOT NULL,
    task_kind TEXT NOT NULL,
    status TEXT NOT NULL,
    verdict TEXT NOT NULL,
    evidence TEXT NOT NULL,
    reason TEXT,
    duration_ms INTEGER NOT NULL,
    secrets_read INTEGER NOT NULL DEFAULT 0,
    receipt_json TEXT NOT NULL,
    created_at_unix INTEGER NOT NULL,
    receipt_hash TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS idx_delegate_receipts_agent_model ON delegate_receipts(agent_id, model_id);
"#;

const MIGRATION_V5: &str = r#"
CREATE TABLE IF NOT EXISTS empirical_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    correlation_id TEXT NOT NULL UNIQUE,
    user_id TEXT NOT NULL,
    repo TEXT NOT NULL,
    language_stack TEXT NOT NULL,
    task_type TEXT NOT NULL,
    risk_level TEXT NOT NULL,
    agent_id TEXT NOT NULL,
    provider_id TEXT NOT NULL,
    model_id TEXT NOT NULL,
    mode TEXT NOT NULL,
    timestamp_bucket TEXT NOT NULL,
    s_rec REAL NOT NULL,
    c_law REAL NOT NULL,
    q_tech REAL NOT NULL,
    d_doc REAL NOT NULL,
    e_cost REAL NOT NULL,
    h_inv REAL NOT NULL,
    penalties REAL NOT NULL DEFAULT 0,
    score REAL NOT NULL,
    created_at_unix INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_empirical_history_agent_model_repo_task ON empirical_history(agent_id, model_id, repo, task_type);
CREATE INDEX IF NOT EXISTS idx_empirical_history_created_at ON empirical_history(created_at_unix);
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::receipt::{ExecReceipt, ExecStatus};

    fn test_receipt(
        correlation_id: &str,
        secrets_read: bool,
        status: ExecStatus,
        exit_code: Option<i32>,
        duration_ms: u128,
        timeout_seconds: u64,
    ) -> ExecReceipt {
        ExecReceipt {
            schema_version: 1,
            correlation_id: correlation_id.to_string(),
            agent: "test-agent".to_string(),
            model: "test-model".to_string(),
            command: vec!["runner".to_string()],
            status,
            policy_reason: "allowed".to_string(),
            started_at_unix: 1,
            duration_ms,
            timeout_seconds,
            exit_code,
            stdout_tail: "stdout".to_string(),
            stderr_tail: "stderr".to_string(),
            secrets_read,
            cleanup_attempted: false,
            cleanup_succeeded: false,
        }
    }

    #[test]
    fn migrates_temp_database_and_reports_status() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("state.sqlite");
        let store = open(Some(&path)).expect("open state");
        let status = store.status().expect("status");
        assert_eq!(status.schema_version, LATEST_SCHEMA_VERSION);
        assert!(!status.secrets_read);
        assert!(status.tables_present.contains(&"agents".to_string()));
        assert!(status.tables_present.contains(&"models".to_string()));
    }

    #[test]
    fn upserts_and_finds_arbitrary_agent_model() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("state.sqlite");
        let store = open(Some(&path)).expect("open state");
        store
            .upsert_agent(&AgentRecord {
                agent_id: "test-agent".to_string(),
                display_name: "Test Agent".to_string(),
                adapter_status: "available".to_string(),
                metadata_json: "{}".to_string(),
            })
            .expect("upsert agent");
        store
            .upsert_model(&ModelRecord {
                agent_id: "test-agent".to_string(),
                model_id: "test-model".to_string(),
                task_kind: "test-task".to_string(),
                gated: false,
                active: true,
                metadata_json: "{}".to_string(),
            })
            .expect("upsert model");
        let found = store
            .find_model("test-agent", "test-model", "test-task")
            .expect("find model")
            .expect("model exists");
        assert_eq!(found.agent_id, "test-agent");
        assert_eq!(found.model_id, "test-model");
        assert!(found.active);
    }

    #[test]
    fn receipt_store_inserts_and_verifies_hash() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("state.sqlite");
        let store = open(Some(&path)).expect("open state");
        let receipt = test_receipt("receipt-hash", false, ExecStatus::Succeeded, Some(0), 42, 9);

        let stored = store
            .insert_receipt(&receipt, "database")
            .expect("insert receipt");
        assert_eq!(stored.receipt_hash, receipt_sha256(&receipt).expect("hash"));
        assert_eq!(stored.task_kind, "database");

        let found = store
            .find_receipt("receipt-hash")
            .expect("find receipt")
            .expect("receipt exists");
        assert_eq!(found.receipt_hash, stored.receipt_hash);
        assert_eq!(found.receipt, receipt);
    }

    #[test]
    fn receipt_store_preserves_status_exit_duration_and_timeout() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("state.sqlite");
        let store = open(Some(&path)).expect("open state");
        let receipt = test_receipt(
            "receipt-fields",
            false,
            ExecStatus::TimedOut,
            None,
            1234,
            99,
        );

        store
            .insert_receipt(&receipt, "debugging")
            .expect("insert receipt");
        let found = store
            .find_receipt("receipt-fields")
            .expect("find receipt")
            .expect("receipt exists");

        assert_eq!(found.receipt.status, ExecStatus::TimedOut);
        assert_eq!(found.receipt.exit_code, None);
        assert_eq!(found.receipt.duration_ms, 1234);
        assert_eq!(found.receipt.timeout_seconds, 99);
    }

    #[test]
    fn receipt_store_rejects_duration_overflow() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("state.sqlite");
        let store = open(Some(&path)).expect("open state");
        let receipt = test_receipt(
            "receipt-overflow",
            false,
            ExecStatus::Succeeded,
            Some(0),
            u128::MAX,
            9,
        );

        let error = store
            .insert_receipt(&receipt, "database")
            .expect_err("overflow should fail");
        assert!(error
            .to_string()
            .contains("duration_ms exceeds SQLite INTEGER range"));
    }

    #[cfg(unix)]
    #[test]
    fn state_database_permissions_are_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("state.sqlite");
        let _store = open(Some(&path)).expect("open state");
        let mode = std::fs::metadata(path)
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[test]
    fn test_circuit_breaker_timeout_opens_cooldown() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("state.sqlite");
        let store = open(Some(&path)).expect("open state");

        store
            .record_breaker_outcome("agent-1", "model-1", BreakerOutcome::TimedOut)
            .expect("record timeout");

        let record = store
            .breaker_record("agent-1", "model-1")
            .expect("fetch record")
            .expect("record present");

        assert_eq!(record.state, "open");
        assert_eq!(record.failure_streak, 1);
        assert!(record.opened_until_unix.is_some());
        assert!(record.opened_until_unix.unwrap() >= now_unix() + 50);

        let allowed = store
            .breaker_allows_model("agent-1", "model-1")
            .expect("allows model check");
        assert!(!allowed);
    }

    #[test]
    fn test_circuit_breaker_success_resets_streak_and_closes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("state.sqlite");
        let store = open(Some(&path)).expect("open state");

        store
            .record_breaker_outcome("agent-1", "model-1", BreakerOutcome::TimedOut)
            .expect("record timeout");
        store
            .record_breaker_outcome("agent-1", "model-1", BreakerOutcome::Success)
            .expect("record success");

        let record = store
            .breaker_record("agent-1", "model-1")
            .expect("fetch record")
            .expect("record present");

        assert_eq!(record.state, "closed");
        assert_eq!(record.failure_streak, 0);
        assert!(record.opened_until_unix.is_none());

        let allowed = store
            .breaker_allows_model("agent-1", "model-1")
            .expect("allows model check");
        assert!(allowed);
    }

    #[test]
    fn test_circuit_breaker_auth401_applies_long_backoff() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("state.sqlite");
        let store = open(Some(&path)).expect("open state");

        store
            .record_breaker_outcome("agent-1", "model-1", BreakerOutcome::Auth401)
            .expect("record 401");

        let record = store
            .breaker_record("agent-1", "model-1")
            .expect("fetch record")
            .expect("record present");

        assert_eq!(record.state, "open");
        assert_eq!(record.failure_streak, 1);
        assert!(record.opened_until_unix.unwrap() >= now_unix() + 3500);
    }

    #[test]
    fn test_circuit_breaker_streak_exponential_backoff() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("state.sqlite");
        let store = open(Some(&path)).expect("open state");

        store
            .record_breaker_outcome("agent-1", "model-1", BreakerOutcome::TimedOut)
            .expect("first timeout");
        let r1 = store.breaker_record("agent-1", "model-1").unwrap().unwrap();
        assert_eq!(r1.failure_streak, 1);

        store
            .record_breaker_outcome("agent-1", "model-1", BreakerOutcome::TimedOut)
            .expect("second timeout");
        let r2 = store.breaker_record("agent-1", "model-1").unwrap().unwrap();
        assert_eq!(r2.failure_streak, 2);
        // Streak 2 backoff = 60 * 2^(2-1) = 120s
        assert!(r2.opened_until_unix.unwrap() >= now_unix() + 110);
    }

    #[test]
    fn test_quota_snapshots_insert_and_latest() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("state.sqlite");
        let store = open(Some(&path)).expect("open state");

        let input1 = QuotaSnapshotInput {
            provider: "agy".to_string(),
            scope: "gemini-weekly".to_string(),
            remaining_pct: Some(47.17),
            used_pct: Some(52.83),
            status: Some("ok".to_string()),
            reset_at_unix: None,
            captured_at_unix: Some(1000),
            metadata_json: None,
        };
        store.insert_quota_snapshot(&input1).expect("insert input1");

        let input2 = QuotaSnapshotInput {
            provider: "agy".to_string(),
            scope: "gemini-weekly".to_string(),
            remaining_pct: Some(40.0),
            used_pct: Some(60.0),
            status: Some("ok".to_string()),
            reset_at_unix: None,
            captured_at_unix: Some(2000),
            metadata_json: None,
        };
        store.insert_quota_snapshot(&input2).expect("insert input2");

        let input3 = QuotaSnapshotInput {
            provider: "claude-code".to_string(),
            scope: "session".to_string(),
            remaining_pct: Some(70.0),
            used_pct: Some(30.0),
            status: Some("ok".to_string()),
            reset_at_unix: Some(1788559999),
            captured_at_unix: Some(1500),
            metadata_json: Some("{\"promo\": true}".to_string()),
        };
        store.insert_quota_snapshot(&input3).expect("insert input3");

        let latest_all = store.latest_quota_snapshots(None).expect("latest all");
        assert_eq!(latest_all.len(), 2);
        let agy_snap = latest_all.iter().find(|s| s.provider == "agy").unwrap();
        assert_eq!(agy_snap.remaining_pct, Some(40.0));
        assert_eq!(agy_snap.captured_at_unix, 2000);

        let latest_agy = store
            .latest_quota_snapshots(Some("agy"))
            .expect("latest agy");
        assert_eq!(latest_agy.len(), 1);
        assert_eq!(latest_agy[0].remaining_pct, Some(40.0));

        let all_agy = store.all_quota_snapshots(Some("agy")).expect("all agy");
        assert_eq!(all_agy.len(), 2);
        assert_eq!(all_agy[0].captured_at_unix, 2000);
        assert_eq!(all_agy[1].captured_at_unix, 1000);
    }

    #[test]
    fn test_quota_migration_is_idempotent() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("state.sqlite");
        let store = open(Some(&path)).expect("open state");

        let status = store.status().expect("status");
        assert_eq!(status.schema_version, LATEST_SCHEMA_VERSION);
        assert!(status
            .tables_present
            .contains(&"quota_snapshots".to_string()));

        // Second migration execution on already migrated DB
        store.migrate().expect("second migration should succeed");
        let status2 = store.status().expect("status2");
        assert_eq!(status2.schema_version, LATEST_SCHEMA_VERSION);
    }

    #[test]
    fn test_migration_upgrade_from_v2_to_v3_preserves_data() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("v2_state.sqlite");

        // 1. Manually build a v2 database (before quota_snapshots existed)
        {
            let raw_conn = rusqlite::Connection::open(&path).expect("open raw sqlite");
            raw_conn
                .execute_batch(
                    r#"
                    CREATE TABLE schema_migrations (
                        version INTEGER PRIMARY KEY,
                        applied_at_unix INTEGER NOT NULL
                    );
                    CREATE TABLE agents (
                        agent_id TEXT PRIMARY KEY,
                        display_name TEXT NOT NULL,
                        adapter_status TEXT NOT NULL,
                        metadata_json TEXT NOT NULL DEFAULT '{}',
                        updated_at_unix INTEGER NOT NULL
                    );
                    CREATE TABLE task_kinds (
                        task_kind TEXT PRIMARY KEY,
                        description TEXT NOT NULL DEFAULT '',
                        metadata_json TEXT NOT NULL DEFAULT '{}',
                        updated_at_unix INTEGER NOT NULL
                    );
                    CREATE TABLE models (
                        agent_id TEXT NOT NULL,
                        model_id TEXT NOT NULL,
                        task_kind TEXT NOT NULL DEFAULT 'general',
                        gated INTEGER NOT NULL DEFAULT 0,
                        active INTEGER NOT NULL DEFAULT 1,
                        metadata_json TEXT NOT NULL DEFAULT '{}',
                        updated_at_unix INTEGER NOT NULL,
                        PRIMARY KEY (agent_id, model_id, task_kind),
                        FOREIGN KEY (agent_id) REFERENCES agents(agent_id) ON DELETE CASCADE
                    );
                    CREATE TABLE receipts (
                        correlation_id TEXT PRIMARY KEY,
                        agent_id TEXT NOT NULL,
                        model_id TEXT NOT NULL,
                        task_kind TEXT NOT NULL,
                        status TEXT NOT NULL,
                        duration_ms INTEGER NOT NULL DEFAULT 0,
                        secrets_read INTEGER NOT NULL DEFAULT 0,
                        receipt_json TEXT NOT NULL,
                        created_at_unix INTEGER NOT NULL,
                        receipt_hash TEXT NOT NULL DEFAULT ''
                    );
                    CREATE TABLE certifications (
                        certificate_id TEXT PRIMARY KEY,
                        agent_id TEXT NOT NULL,
                        model_id TEXT NOT NULL,
                        task_kind TEXT NOT NULL,
                        status TEXT NOT NULL,
                        receipt_hash TEXT NOT NULL,
                        secrets_read INTEGER NOT NULL DEFAULT 0,
                        created_at_unix INTEGER NOT NULL
                    );
                    CREATE TABLE route_scores (
                        agent_id TEXT NOT NULL,
                        model_id TEXT NOT NULL,
                        task_kind TEXT NOT NULL,
                        success_count INTEGER NOT NULL DEFAULT 0,
                        failure_count INTEGER NOT NULL DEFAULT 0,
                        total_latency_ms INTEGER NOT NULL DEFAULT 0,
                        updated_at_unix INTEGER NOT NULL,
                        PRIMARY KEY (agent_id, model_id, task_kind)
                    );
                    CREATE TABLE circuit_breakers (
                        agent_id TEXT NOT NULL,
                        model_id TEXT NOT NULL,
                        state TEXT NOT NULL,
                        failure_streak INTEGER NOT NULL DEFAULT 0,
                        opened_until_unix INTEGER,
                        updated_at_unix INTEGER NOT NULL,
                        PRIMARY KEY (agent_id, model_id)
                    );
                    INSERT INTO schema_migrations(version, applied_at_unix) VALUES (1, 1000);
                    INSERT INTO schema_migrations(version, applied_at_unix) VALUES (2, 2000);

                    INSERT INTO agents(agent_id, display_name, adapter_status, metadata_json, updated_at_unix)
                    VALUES ('pre-v3-agent', 'Pre V3 Agent', 'available', '{"pre": true}', 1000);
                    "#,
                )
                .expect("setup v2 schema and data");

            // Verify quota_snapshots does NOT exist prior to opening with state::open
            let mut stmt = raw_conn
                .prepare(
                    "SELECT name FROM sqlite_master WHERE type='table' AND name='quota_snapshots'",
                )
                .expect("prepare check");
            let mut rows = stmt.query([]).expect("query check");
            assert!(
                rows.next().expect("next").is_none(),
                "quota_snapshots must not exist in v2 db"
            );
        }

        // 2. Open with state::open (which runs migrate())
        let store = open(Some(&path)).expect("open state upgrades from v2 to v4");
        let status = store.status().expect("status");

        assert_eq!(status.schema_version, 5);
        assert_eq!(status.migrations_applied, vec![1, 2, 3, 4, 5]);
        assert!(status
            .tables_present
            .contains(&"quota_snapshots".to_string()));
        assert!(status
            .tables_present
            .contains(&"delegate_receipts".to_string()));
        assert!(status
            .tables_present
            .contains(&"empirical_history".to_string()));

        // 3. Verify pre-existing v2 data was preserved
        let agent = store
            .find_agent("pre-v3-agent")
            .expect("query agent")
            .expect("agent must exist");
        assert_eq!(agent.display_name, "Pre V3 Agent");
        assert_eq!(agent.metadata_json, "{\"pre\": true}");

        // 4. Verify new quota snapshot table is functional
        let snap = store
            .insert_quota_snapshot(&QuotaSnapshotInput {
                provider: "agy".to_string(),
                scope: "gemini-weekly".to_string(),
                remaining_pct: Some(55.5),
                used_pct: Some(44.5),
                status: Some("ok".to_string()),
                reset_at_unix: None,
                captured_at_unix: Some(3000),
                metadata_json: None,
            })
            .expect("insert snapshot into upgraded db");

        assert_eq!(snap.provider, "agy");
        assert_eq!(snap.remaining_pct, Some(55.5));

        let latest = store
            .latest_quota_snapshots(None)
            .expect("latest snapshots");
        assert_eq!(latest.len(), 1);
        assert_eq!(latest[0].scope, "gemini-weekly");
    }

    #[test]
    fn test_delegate_receipt_insert_and_find() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("state.sqlite");
        let store = open(Some(&path)).expect("open state");

        let receipt = DelegateReceipt {
            schema_version: 1,
            correlation_id: "del-corr-1".to_string(),
            agent: "agy".to_string(),
            model: "gemini-3.7-flash-high".to_string(),
            command: vec!["rtk".to_string(), "agy".to_string()],
            status: crate::receipt::DelegateStatus::Validated,
            reason: None,
            verdict: crate::receipt::DelegateVerdict::Util,
            evidence: "abcdef123456".to_string(),
            stdout_tail: "finished successfully".to_string(),
            stderr_tail: String::new(),
            started_at_unix: 1000,
            duration_ms: 120,
            timeout_seconds: 60,
            exit_code: Some(0),
            secrets_read: false,
        };

        let stored = store
            .insert_delegate_receipt(&receipt, "delegate")
            .expect("insert delegate receipt");
        assert_eq!(stored.correlation_id, "del-corr-1");
        assert_eq!(stored.status, "succeeded");
        assert_eq!(
            stored.receipt_hash,
            receipt_sha256(&stored.receipt).expect("hash stored legacy receipt")
        );

        let found = store
            .find_delegate_receipt("del-corr-1")
            .expect("find delegate receipt")
            .expect("delegate receipt must exist");
        assert_eq!(found.correlation_id, "del-corr-1");
        assert_eq!(found.status, crate::receipt::DelegateStatus::Validated);
        assert_eq!(found.verdict, crate::receipt::DelegateVerdict::Util);
        assert_eq!(found.evidence, "abcdef123456");

        // Check fallback in legacy receipts table
        let found_legacy = store
            .find_receipt("del-corr-1")
            .expect("find legacy receipt")
            .expect("legacy receipt exists");
        assert_eq!(found_legacy.correlation_id, "del-corr-1");
        assert_eq!(found_legacy.status, "succeeded");
        assert_eq!(
            found_legacy.receipt.status,
            crate::receipt::ExecStatus::Succeeded
        );
        assert_eq!(
            found_legacy.receipt_hash,
            receipt_sha256(&found_legacy.receipt).expect("hash legacy receipt")
        );

        // Check list_delegate_receipts
        let all = store
            .list_delegate_receipts()
            .expect("list delegate receipts");
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].correlation_id, "del-corr-1");
        assert_eq!(all[0].agent, "agy");
    }

    #[test]
    fn test_empirical_history_insert_list_and_aggregate() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("state.sqlite");
        let store = open(Some(&path)).expect("open state");

        let now = now_unix();
        let rec1 = EmpiricalRecordInput {
            correlation_id: "emp-corr-1".to_string(),
            user_id: "freddy".to_string(),
            repo: "agent-orchestrator".to_string(),
            language_stack: "rust".to_string(),
            task_type: "feature".to_string(),
            risk_level: "medio".to_string(),
            agent_id: "agy".to_string(),
            provider_id: "google".to_string(),
            model_id: "gemini-3.7-flash-high".to_string(),
            mode: "agentic".to_string(),
            timestamp_bucket: "2026-09-04".to_string(),
            s_rec: 1.0,
            c_law: 1.0,
            q_tech: 1.0,
            d_doc: 1.0,
            e_cost: 1.0,
            h_inv: 0.0,
            penalties: -1.0,
            score: 2.0,
            created_at_unix: Some(now.saturating_sub(86400 * 5)),
        };

        let rec2 = EmpiricalRecordInput {
            correlation_id: "emp-corr-2".to_string(),
            user_id: "freddy".to_string(),
            repo: "agent-orchestrator".to_string(),
            language_stack: "rust".to_string(),
            task_type: "feature".to_string(),
            risk_level: "medio".to_string(),
            agent_id: "agy".to_string(),
            provider_id: "google".to_string(),
            model_id: "gemini-3.7-flash-high".to_string(),
            mode: "agentic".to_string(),
            timestamp_bucket: "2026-09-04".to_string(),
            s_rec: 0.0,
            c_law: 1.0,
            q_tech: 0.0,
            d_doc: 1.0,
            e_cost: 1.0,
            h_inv: 0.0,
            penalties: 2.5,
            score: -2.0,
            created_at_unix: Some(now.saturating_sub(3600)),
        };

        let inserted1 = store.insert_empirical_record(&rec1).expect("insert rec1");
        assert_eq!(inserted1.correlation_id, "emp-corr-1");
        assert_eq!(inserted1.score, 2.0);

        let inserted2 = store.insert_empirical_record(&rec2).expect("insert rec2");
        assert_eq!(inserted2.correlation_id, "emp-corr-2");
        assert_eq!(inserted2.score, -2.0);

        // Filter search
        let list_all = store
            .list_empirical_history(&EmpiricalHistoryFilter {
                agent_id: Some("agy".to_string()),
                model_id: Some("gemini-3.7-flash-high".to_string()),
                repo: Some("agent-orchestrator".to_string()),
                task_type: Some("feature".to_string()),
                user_id: None,
                limit: None,
            })
            .expect("list history");
        assert_eq!(list_all.len(), 2);
        assert_eq!(list_all[0].correlation_id, "emp-corr-2"); // ordered by created_at desc
        assert_eq!(list_all[1].correlation_id, "emp-corr-1");

        // Aggregation
        let agg = store
            .aggregate_scores(
                "agy",
                "gemini-3.7-flash-high",
                Some("agent-orchestrator"),
                Some("feature"),
            )
            .expect("aggregate scores");
        assert_eq!(agg.sample_count, 2);
        assert_eq!(agg.mean_score, 0.0);
        assert_eq!(agg.mean_s_rec, 0.5);
        assert!(agg.decay_weighted_score < 0.0); // more recent rec2 is negative

        // Empty aggregation
        let empty_agg = store
            .aggregate_scores("non-existent-agent", "model", None, None)
            .expect("empty aggregate");
        assert_eq!(empty_agg.sample_count, 0);
        assert_eq!(empty_agg.mean_score, 0.0);
    }

    #[test]
    fn test_migration_v5_creates_empirical_history_and_preserves_data() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("v4_state.sqlite");

        // Setup manual v4 database
        {
            let raw_conn = rusqlite::Connection::open(&path).expect("open raw sqlite");
            raw_conn
                .execute_batch(
                    r#"
                    CREATE TABLE schema_migrations (
                        version INTEGER PRIMARY KEY,
                        applied_at_unix INTEGER NOT NULL
                    );
                    CREATE TABLE agents (
                        agent_id TEXT PRIMARY KEY,
                        display_name TEXT NOT NULL,
                        adapter_status TEXT NOT NULL,
                        metadata_json TEXT NOT NULL DEFAULT '{}',
                        updated_at_unix INTEGER NOT NULL
                    );
                    CREATE TABLE delegate_receipts (
                        correlation_id TEXT PRIMARY KEY,
                        agent_id TEXT NOT NULL,
                        model_id TEXT NOT NULL,
                        task_kind TEXT NOT NULL,
                        status TEXT NOT NULL,
                        verdict TEXT NOT NULL,
                        evidence TEXT NOT NULL,
                        reason TEXT,
                        duration_ms INTEGER NOT NULL,
                        secrets_read INTEGER NOT NULL DEFAULT 0,
                        receipt_json TEXT NOT NULL,
                        created_at_unix INTEGER NOT NULL,
                        receipt_hash TEXT NOT NULL DEFAULT ''
                    );
                    INSERT INTO schema_migrations(version, applied_at_unix) VALUES (1, 1000);
                    INSERT INTO schema_migrations(version, applied_at_unix) VALUES (2, 2000);
                    INSERT INTO schema_migrations(version, applied_at_unix) VALUES (3, 3000);
                    INSERT INTO schema_migrations(version, applied_at_unix) VALUES (4, 4000);
                    INSERT INTO agents(agent_id, display_name, adapter_status, metadata_json, updated_at_unix)
                    VALUES ('test-v4-agent', 'Test V4 Agent', 'available', '{}', 4000);
                    "#,
                )
                .expect("setup v4 database");
        }

        // Open with state::open which runs migrations up to v5
        let store = open(Some(&path)).expect("upgrade to v5");
        let status = store.status().expect("status");
        assert_eq!(status.schema_version, 5);
        assert!(status
            .tables_present
            .contains(&"empirical_history".to_string()));
        assert!(status
            .tables_present
            .contains(&"delegate_receipts".to_string()));

        let agent = store
            .find_agent("test-v4-agent")
            .expect("find agent")
            .expect("exists");
        assert_eq!(agent.display_name, "Test V4 Agent");
    }

    #[test]
    fn test_ingest_from_delegate_receipts_is_idempotent() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("state.sqlite");
        let store = open(Some(&path)).expect("open state");

        let receipt1 = DelegateReceipt {
            schema_version: 1,
            correlation_id: "ingest-corr-1".to_string(),
            agent: "agy".to_string(),
            model: "gemini-3.7-flash-high".to_string(),
            command: vec!["rtk".to_string(), "agy".to_string()],
            status: crate::receipt::DelegateStatus::Validated,
            reason: None,
            verdict: crate::receipt::DelegateVerdict::Util,
            evidence: "123456789abc".to_string(),
            stdout_tail: "ok".to_string(),
            stderr_tail: String::new(),
            started_at_unix: 1788523200,
            duration_ms: 50,
            timeout_seconds: 60,
            exit_code: Some(0),
            secrets_read: false,
        };

        let receipt2 = DelegateReceipt {
            schema_version: 1,
            correlation_id: "ingest-corr-2".to_string(),
            agent: "claude-code".to_string(),
            model: "claude-3-7-sonnet".to_string(),
            command: vec!["rtk".to_string(), "claude".to_string()],
            status: crate::receipt::DelegateStatus::Failed,
            reason: Some("not_executed".to_string()),
            verdict: crate::receipt::DelegateVerdict::NonUtil,
            evidence: "none".to_string(),
            stdout_tail: String::new(),
            stderr_tail: "err".to_string(),
            started_at_unix: 1788523300,
            duration_ms: 10,
            timeout_seconds: 60,
            exit_code: Some(1),
            secrets_read: false,
        };

        store
            .insert_delegate_receipt(&receipt1, "delegate")
            .expect("insert r1");
        store
            .insert_delegate_receipt(&receipt2, "delegate")
            .expect("insert r2");

        // First ingestion
        let report1 = crate::score::ingest_from_delegate_receipts(&store).expect("ingest 1");
        assert_eq!(report1.total_receipts_scanned, 2);
        assert_eq!(report1.records_ingested, 2);

        let history1 = store
            .list_empirical_history(&EmpiricalHistoryFilter::default())
            .expect("list history 1");
        assert_eq!(history1.len(), 2);

        // Second ingestion - should be completely idempotent (upsert)
        let report2 = crate::score::ingest_from_delegate_receipts(&store).expect("ingest 2");
        assert_eq!(report2.total_receipts_scanned, 2);
        assert_eq!(report2.records_ingested, 2);

        let history2 = store
            .list_empirical_history(&EmpiricalHistoryFilter::default())
            .expect("list history 2");
        assert_eq!(history2.len(), 2); // Count remains 2
    }
}
