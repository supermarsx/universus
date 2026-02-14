use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, Error as SqliteError, ErrorCode, OptionalExtension};

use crate::models::{HistoryRecord, HistoryRecordInput, HistoryStatsItem};

pub const DEFAULT_HISTORY_DB_PATH: &str = "sms-history.sqlite3";

#[derive(Debug)]
pub enum HistoryStoreError {
    Open(String),
    InitSchema(String),
    Query(String),
}

impl Display for HistoryStoreError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Open(message) => write!(f, "{message}"),
            Self::InitSchema(message) => write!(f, "{message}"),
            Self::Query(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for HistoryStoreError {}

#[derive(Debug)]
pub enum InsertHistoryError {
    DuplicateIdempotency,
    Store(HistoryStoreError),
}

impl Display for InsertHistoryError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateIdempotency => {
                write!(f, "duplicate idempotency key for successful history entry")
            }
            Self::Store(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for InsertHistoryError {}

#[derive(Clone, Debug)]
pub struct HistoryStore {
    db_path: PathBuf,
}

impl HistoryStore {
    pub fn from_env() -> Result<Self, HistoryStoreError> {
        let db_path = std::env::var("SMS_HISTORY_DB_PATH")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| DEFAULT_HISTORY_DB_PATH.to_string());

        Self::new(db_path)
    }

    pub fn new<P: Into<PathBuf>>(db_path: P) -> Result<Self, HistoryStoreError> {
        let store = Self {
            db_path: db_path.into(),
        };
        store.init_schema()?;
        Ok(store)
    }

    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    fn connect(&self) -> Result<Connection, HistoryStoreError> {
        Connection::open(&self.db_path)
            .map_err(|err| HistoryStoreError::Open(format!("Failed to open SMS history DB: {err}")))
    }

    fn init_schema(&self) -> Result<(), HistoryStoreError> {
        let conn = self.connect()?;
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS sms_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                request_id TEXT NOT NULL,
                idempotency_key TEXT,
                contact TEXT NOT NULL,
                destination TEXT NOT NULL,
                channel TEXT NOT NULL,
                status TEXT NOT NULL,
                error TEXT,
                metadata TEXT,
                created_at_ms INTEGER NOT NULL
            );
            CREATE UNIQUE INDEX IF NOT EXISTS idx_sms_history_success_idempotency
                ON sms_history(idempotency_key)
                WHERE idempotency_key IS NOT NULL AND status = 'success';
            ",
        )
        .map_err(|err| {
            HistoryStoreError::InitSchema(format!("Failed to initialize SMS history schema: {err}"))
        })?;
        Ok(())
    }

    fn row_to_history_entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<HistoryRecord> {
        let metadata_raw: Option<String> = row.get("metadata")?;
        let created_at_ms_raw: i64 = row.get("created_at_ms")?;

        Ok(HistoryRecord {
            id: row.get::<_, i64>("id")?.max(0) as u64,
            request_id: row.get("request_id")?,
            idempotency_key: row.get("idempotency_key")?,
            contact: row.get("contact")?,
            destination: row.get("destination")?,
            channel: row.get("channel")?,
            status: row.get("status")?,
            error: row.get("error")?,
            metadata: metadata_raw.and_then(|value| serde_json::from_str(&value).ok()),
            created_at_ms: created_at_ms_raw.max(0) as u128,
        })
    }

    pub fn find_success_by_idempotency(
        &self,
        key: &str,
    ) -> Result<Option<HistoryRecord>, HistoryStoreError> {
        let conn = self.connect()?;
        conn.query_row(
            "
            SELECT id, request_id, idempotency_key, contact, destination, channel, status, error, metadata, created_at_ms
            FROM sms_history
            WHERE idempotency_key = ?1 AND status = 'success'
            ORDER BY id DESC
            LIMIT 1
            ",
            params![key],
            Self::row_to_history_entry,
        )
        .optional()
        .map_err(|err| {
            HistoryStoreError::Query(format!("Failed to query idempotency history: {err}"))
        })
    }

    pub fn count_recent_for_contact(
        &self,
        contact: &str,
        window_seconds: u64,
        now_ms: u128,
    ) -> Result<usize, HistoryStoreError> {
        let lower_bound = now_ms.saturating_sub((window_seconds as u128) * 1000);
        let conn = self.connect()?;

        let count: i64 = conn
            .query_row(
                "
                SELECT COUNT(*)
                FROM sms_history
                WHERE contact = ?1 AND created_at_ms >= ?2
                ",
                params![contact, lower_bound as i64],
                |row| row.get(0),
            )
            .map_err(|err| {
                HistoryStoreError::Query(format!("Failed to query recent history: {err}"))
            })?;

        Ok(count.max(0) as usize)
    }

    pub fn insert_history(&self, entry: &HistoryRecordInput) -> Result<u64, InsertHistoryError> {
        let conn = self.connect().map_err(InsertHistoryError::Store)?;
        let metadata_json = entry.metadata.as_ref().map(|value| value.to_string());

        let result = conn.execute(
            "
            INSERT INTO sms_history (
                request_id, idempotency_key, contact, destination, channel, status, error, metadata, created_at_ms
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ",
            params![
                &entry.request_id,
                &entry.idempotency_key,
                &entry.contact,
                &entry.destination,
                &entry.channel,
                &entry.status,
                &entry.error,
                &metadata_json,
                entry.created_at_ms as i64
            ],
        );

        match result {
            Ok(_) => Ok(conn.last_insert_rowid().max(0) as u64),
            Err(SqliteError::SqliteFailure(code, _))
                if code.code == ErrorCode::ConstraintViolation =>
            {
                Err(InsertHistoryError::DuplicateIdempotency)
            }
            Err(err) => Err(InsertHistoryError::Store(HistoryStoreError::Query(
                format!("Failed to insert history entry: {err}"),
            ))),
        }
    }

    pub fn load_recent_history(
        &self,
        limit: usize,
    ) -> Result<Vec<HistoryRecord>, HistoryStoreError> {
        let conn = self.connect()?;
        let mut statement = conn
            .prepare(
                "
                SELECT id, request_id, idempotency_key, contact, destination, channel, status, error, metadata, created_at_ms
                FROM sms_history
                ORDER BY id DESC
                LIMIT ?1
                ",
            )
            .map_err(|err| HistoryStoreError::Query(format!("Failed to prepare history query: {err}")))?;

        let rows = statement
            .query_map(params![limit as i64], Self::row_to_history_entry)
            .map_err(|err| {
                HistoryStoreError::Query(format!("Failed to read history rows: {err}"))
            })?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| HistoryStoreError::Query(format!("Failed to parse history rows: {err}")))
    }

    pub fn history_stats(&self) -> Result<Vec<HistoryStatsItem>, HistoryStoreError> {
        let conn = self.connect()?;
        let mut statement = conn
            .prepare(
                "
                SELECT channel, status, COUNT(*) as count
                FROM sms_history
                GROUP BY channel, status
                ORDER BY channel ASC, status ASC
                ",
            )
            .map_err(|err| {
                HistoryStoreError::Query(format!("Failed to prepare history stats query: {err}"))
            })?;

        let rows = statement
            .query_map([], |row| {
                Ok(HistoryStatsItem {
                    channel: row.get("channel")?,
                    status: row.get("status")?,
                    count: row.get::<_, i64>("count")?.max(0) as u64,
                })
            })
            .map_err(|err| {
                HistoryStoreError::Query(format!("Failed to read history stats rows: {err}"))
            })?;

        rows.collect::<Result<Vec<_>, _>>().map_err(|err| {
            HistoryStoreError::Query(format!("Failed to parse history stats rows: {err}"))
        })
    }
}

#[cfg(test)]
mod tests {
    use tempfile::NamedTempFile;

    use super::*;

    fn sample_record(
        idempotency_key: Option<&str>,
        status: &str,
        created_at_ms: u128,
    ) -> HistoryRecordInput {
        HistoryRecordInput {
            request_id: format!("req-{created_at_ms}"),
            idempotency_key: idempotency_key.map(ToOwned::to_owned),
            contact: "+12065550123".to_string(),
            destination: "+12065550123".to_string(),
            channel: "sms_twilio".to_string(),
            status: status.to_string(),
            error: None,
            metadata: Some(serde_json::json!({ "attempt": created_at_ms })),
            created_at_ms,
        }
    }

    #[test]
    fn insert_and_find_success_by_idempotency() {
        let db_file = NamedTempFile::new().expect("temp db file");
        let store = HistoryStore::new(db_file.path()).expect("create store");

        let inserted_id = store
            .insert_history(&sample_record(Some("idem-1"), "success", 1000))
            .expect("insert history");
        assert!(inserted_id > 0);

        let found = store
            .find_success_by_idempotency("idem-1")
            .expect("query history")
            .expect("history exists");

        assert_eq!(found.idempotency_key.as_deref(), Some("idem-1"));
        assert_eq!(found.status, "success");
        assert_eq!(found.metadata, Some(serde_json::json!({ "attempt": 1000 })));
    }

    #[test]
    fn duplicate_success_idempotency_is_rejected() {
        let db_file = NamedTempFile::new().expect("temp db file");
        let store = HistoryStore::new(db_file.path()).expect("create store");

        store
            .insert_history(&sample_record(Some("idem-dup"), "success", 1000))
            .expect("first insert should pass");

        let duplicate = store.insert_history(&sample_record(Some("idem-dup"), "success", 1001));
        assert!(matches!(
            duplicate,
            Err(InsertHistoryError::DuplicateIdempotency)
        ));
    }

    #[test]
    fn find_only_returns_success_and_recent_history_orders_desc() {
        let db_file = NamedTempFile::new().expect("temp db file");
        let store = HistoryStore::new(db_file.path()).expect("create store");

        store
            .insert_history(&sample_record(Some("idem-mixed"), "failed", 1000))
            .expect("insert failed status");
        store
            .insert_history(&sample_record(Some("idem-mixed"), "success", 2000))
            .expect("insert success status");

        let found = store
            .find_success_by_idempotency("idem-mixed")
            .expect("find success")
            .expect("success exists");
        assert_eq!(found.status, "success");
        assert_eq!(found.created_at_ms, 2000);

        let recent = store.load_recent_history(2).expect("recent history query");
        assert_eq!(recent.len(), 2);
        assert!(recent[0].id > recent[1].id);
    }
}
