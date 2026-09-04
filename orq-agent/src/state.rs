use crate::receipt::{receipt_sha256, ExecReceipt};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

pub const STATE_DB_ENV: &str = "ORQ_STATE_DB";
const LATEST_SCHEMA_VERSION: i64 = 2;

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
                "INSERT OR IGNORE INTO schema_migrations(version, applied_at_unix) VALUES (?1, strftime('%s','now'))",
                params![LATEST_SCHEMA_VERSION],
            )
            .map_err(|source| StoreError::Sqlite { context: "record migration", source })?;
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
CREATE INDEX IF NOT EXISTS idx_models_agent_model_task ON models(agent_id, model_id, task_kind);
CREATE INDEX IF NOT EXISTS idx_receipts_agent_model_task ON receipts(agent_id, model_id, task_kind);
CREATE INDEX IF NOT EXISTS idx_certifications_agent_model_task ON certifications(agent_id, model_id, task_kind);
CREATE INDEX IF NOT EXISTS idx_route_scores_agent_model_task ON route_scores(agent_id, model_id, task_kind);
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
        assert_eq!(status.secrets_read, false);
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
}
