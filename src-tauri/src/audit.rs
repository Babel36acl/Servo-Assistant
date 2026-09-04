use rusqlite::{params, Connection};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuditStoreError {
    #[error("SQLite 审计库错误：{0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("无法创建审计目录：{0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEntry {
    pub timestamp_ms: u64,
    pub action: String,
    pub status: String,
    pub detail: String,
}

#[derive(Clone)]
pub struct AuditStore {
    path: PathBuf,
}

impl AuditStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, AuditStoreError> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let store = Self { path };
        let connection = store.connection()?;
        connection.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=FULL;
             CREATE TABLE IF NOT EXISTS audit_entries (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 timestamp_ms INTEGER NOT NULL,
                 action TEXT NOT NULL,
                 status TEXT NOT NULL,
                 detail TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_audit_timestamp
                 ON audit_entries(timestamp_ms DESC);
             CREATE TABLE IF NOT EXISTS parameter_snapshots (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 timestamp_ms INTEGER NOT NULL,
                 device_id TEXT NOT NULL,
                 profile_version TEXT NOT NULL,
                 label TEXT NOT NULL,
                 snapshot_json TEXT NOT NULL
             );",
        )?;
        Ok(store)
    }

    pub fn append(
        &self,
        action: &str,
        status: &str,
        detail: &str,
    ) -> Result<AuditEntry, AuditStoreError> {
        let entry = AuditEntry {
            timestamp_ms: now_ms(),
            action: action.to_owned(),
            status: status.to_owned(),
            detail: detail.to_owned(),
        };
        self.connection()?.execute(
            "INSERT INTO audit_entries(timestamp_ms, action, status, detail)
             VALUES (?1, ?2, ?3, ?4)",
            params![entry.timestamp_ms, entry.action, entry.status, entry.detail],
        )?;
        Ok(entry)
    }

    pub fn list(&self, limit: usize) -> Result<Vec<AuditEntry>, AuditStoreError> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT timestamp_ms, action, status, detail
             FROM audit_entries ORDER BY id DESC LIMIT ?1",
        )?;
        let entries = statement
            .query_map([limit.clamp(1, 5000) as i64], |row| {
                Ok(AuditEntry {
                    timestamp_ms: row.get(0)?,
                    action: row.get(1)?,
                    status: row.get(2)?,
                    detail: row.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(entries.into_iter().rev().collect())
    }

    pub fn save_snapshot(
        &self,
        timestamp_ms: u64,
        device_id: &str,
        profile_version: &str,
        label: &str,
        snapshot_json: &str,
    ) -> Result<(), AuditStoreError> {
        self.connection()?.execute(
            "INSERT INTO parameter_snapshots(
                 timestamp_ms, device_id, profile_version, label, snapshot_json
             ) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                timestamp_ms,
                device_id,
                profile_version,
                label,
                snapshot_json
            ],
        )?;
        Ok(())
    }

    fn connection(&self) -> Result<Connection, rusqlite::Error> {
        Connection::open(&self.path)
    }
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_and_snapshot_are_persistent() {
        let path = std::env::temp_dir().join(format!(
            "servo-assistant-audit-{}-{}.db",
            std::process::id(),
            now_ms()
        ));
        let store = AuditStore::open(&path).unwrap();
        store.append("test", "success", "persistent").unwrap();
        store
            .save_snapshot(now_ms(), "example-servo", "1.0", "test", "{}")
            .unwrap();
        drop(store);

        let reopened = AuditStore::open(&path).unwrap();
        let entries = reopened.list(10).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].detail, "persistent");
        drop(reopened);
        let _ = std::fs::remove_file(path);
    }
}
