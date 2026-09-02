use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::path::{Path, PathBuf};
use thiserror::Error;

pub const STATE_DB_ENV: &str = "ORQ_STATE_DB";
const LATEST_SCHEMA_VERSION: i64 = 1;

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
                "INSERT OR IGNORE INTO schema_migrations(version, applied_at_unix) VALUES (?1, strftime('%s','now'))",
                params![LATEST_SCHEMA_VERSION],
            )
            .map_err(|source| StoreError::Sqlite { context: "record migration", source })?;
        Ok(())
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
    created_at_unix INTEGER NOT NULL
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

    #[test]
    fn migrates_temp_database_and_reports_status() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("state.sqlite");
        let store = open(Some(&path)).expect("open state");
        let status = store.status().expect("status");
        assert_eq!(status.schema_version, 1);
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
}
