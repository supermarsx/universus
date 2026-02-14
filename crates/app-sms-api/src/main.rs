use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use rusqlite::{params, Connection, Error as SqliteError, ErrorCode, OptionalExtension};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

const SERVICE_NAME: &str = "sms";
const DEFAULT_PORT: u16 = 3003;
const HISTORY_DEFAULT_LIMIT: usize = 50;
const HISTORY_MAX_LIMIT: usize = 200;
const DEFAULT_HISTORY_DB_PATH: &str = "sms-history.sqlite3";
const DEFAULT_CHANNEL_FAILURE_THRESHOLD: u64 = 3;
const DEFAULT_CHANNEL_COOLDOWN_MS: u128 = 30_000;

const SUPPORTED_CHANNELS: [&str; 7] = [
    "sms_twilio",
    "sms_http",
    "whatsapp_twilio",
    "whatsapp_baileys",
    "telegram",
    "discord",
    "custom_http",
];

#[derive(Serialize)]
struct ServiceStatus {
    status: &'static str,
    service: &'static str,
}

#[derive(Serialize)]
struct BasicErrorResponse {
    success: bool,
    error: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SendRequest {
    contact: String,
    message: String,
    channels: Option<Vec<String>>,
    metadata: Option<serde_json::Value>,
    idempotency_key: Option<String>,
}

#[derive(Serialize)]
struct SendResponse {
    success: bool,
    channel: String,
    destination: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    idempotent: Option<bool>,
}

#[derive(Clone, Debug)]
struct HistoryEntry {
    id: u64,
    request_id: String,
    idempotency_key: Option<String>,
    contact: String,
    destination: String,
    channel: String,
    status: String,
    error: Option<String>,
    metadata: Option<serde_json::Value>,
    created_at_ms: u128,
}

#[derive(Serialize)]
struct HistoryEntryPayload {
    id: u64,
    request_id: String,
    idempotency_key: Option<String>,
    contact: String,
    destination: String,
    channel: String,
    status: String,
    error: Option<String>,
    metadata: Option<serde_json::Value>,
    created_at: String,
}

#[derive(Serialize)]
struct HistoryResponse {
    success: bool,
    entries: Vec<HistoryEntryPayload>,
}

#[derive(Serialize)]
struct MetricsSnapshot {
    requests: u64,
    successes: u64,
    failures: u64,
    #[serde(rename = "perChannelSuccess")]
    per_channel_success: HashMap<String, u64>,
    #[serde(rename = "perChannelFailure")]
    per_channel_failure: HashMap<String, u64>,
    #[serde(rename = "avgResponseMs")]
    avg_response_ms: f64,
}

#[derive(Clone, Serialize)]
struct HistoryStatsItem {
    channel: String,
    status: String,
    count: u64,
}

#[derive(Serialize)]
struct MetricsResponse {
    success: bool,
    metrics: MetricsSnapshot,
    history: Vec<HistoryStatsItem>,
}

#[derive(Deserialize)]
struct HistoryQuery {
    limit: Option<usize>,
}

#[derive(Default)]
struct ChannelCircuitState {
    consecutive_failures: u64,
    open_until_ms: Option<u128>,
}

#[derive(Clone)]
struct HistoryStore {
    db_path: String,
}

impl HistoryStore {
    fn from_env() -> Result<Self, String> {
        let db_path = std::env::var("SMS_HISTORY_DB_PATH")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| DEFAULT_HISTORY_DB_PATH.to_string());

        let store = Self { db_path };
        store.init_schema()?;
        Ok(store)
    }

    fn connect(&self) -> Result<Connection, String> {
        Connection::open(&self.db_path)
            .map_err(|err| format!("Failed to open SMS history DB: {err}"))
    }

    fn init_schema(&self) -> Result<(), String> {
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
        .map_err(|err| format!("Failed to initialize SMS history schema: {err}"))?;
        Ok(())
    }

    fn row_to_history_entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<HistoryEntry> {
        let metadata_raw: Option<String> = row.get("metadata")?;
        let created_at_ms_raw: i64 = row.get("created_at_ms")?;
        Ok(HistoryEntry {
            id: row.get::<_, i64>("id")? as u64,
            request_id: row.get("request_id")?,
            idempotency_key: row.get("idempotency_key")?,
            contact: row.get("contact")?,
            destination: row.get("destination")?,
            channel: row.get("channel")?,
            status: row.get("status")?,
            error: row.get("error")?,
            metadata: metadata_raw
                .and_then(|value| serde_json::from_str::<serde_json::Value>(&value).ok()),
            created_at_ms: created_at_ms_raw.max(0) as u128,
        })
    }

    fn find_success_by_idempotency(&self, key: &str) -> Result<Option<HistoryEntry>, String> {
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
        .map_err(|err| format!("Failed to query idempotency history: {err}"))
    }

    fn count_recent_for_contact(
        &self,
        contact: &str,
        window_seconds: u64,
    ) -> Result<usize, String> {
        let now = now_unix_epoch_ms();
        let lower_bound = now.saturating_sub((window_seconds as u128) * 1000);
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
            .map_err(|err| format!("Failed to query recent history: {err}"))?;
        Ok(count.max(0) as usize)
    }

    fn insert_history(&self, entry: &HistoryEntry) -> Result<u64, InsertHistoryError> {
        let conn = self.connect().map_err(InsertHistoryError::Other)?;
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
            Err(err) => Err(InsertHistoryError::Other(format!(
                "Failed to insert history entry: {err}"
            ))),
        }
    }

    fn load_recent_history(&self, limit: usize) -> Result<Vec<HistoryEntry>, String> {
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
            .map_err(|err| format!("Failed to prepare history query: {err}"))?;

        let rows = statement
            .query_map(params![limit as i64], Self::row_to_history_entry)
            .map_err(|err| format!("Failed to read history rows: {err}"))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| format!("Failed to parse history rows: {err}"))
    }

    fn history_stats(&self) -> Result<Vec<HistoryStatsItem>, String> {
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
            .map_err(|err| format!("Failed to prepare history stats query: {err}"))?;

        let rows = statement
            .query_map([], |row| {
                Ok(HistoryStatsItem {
                    channel: row.get("channel")?,
                    status: row.get("status")?,
                    count: row.get::<_, i64>("count")?.max(0) as u64,
                })
            })
            .map_err(|err| format!("Failed to read history stats rows: {err}"))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| format!("Failed to parse history stats rows: {err}"))
    }
}

enum InsertHistoryError {
    DuplicateIdempotency,
    Other(String),
}

struct SmsApiState {
    started_at: Instant,
    request_count: u64,
    success_count: u64,
    failure_count: u64,
    per_channel_success: HashMap<String, u64>,
    per_channel_failure: HashMap<String, u64>,
    response_times_ms: Vec<u128>,
    next_request_sequence: u64,
    history_store: HistoryStore,
    channel_circuits: HashMap<String, ChannelCircuitState>,
}

impl SmsApiState {
    fn new() -> Result<Self, String> {
        let history_store = HistoryStore::from_env()?;
        Ok(Self {
            started_at: Instant::now(),
            request_count: 0,
            success_count: 0,
            failure_count: 0,
            per_channel_success: HashMap::new(),
            per_channel_failure: HashMap::new(),
            response_times_ms: Vec::new(),
            next_request_sequence: 0,
            history_store,
            channel_circuits: HashMap::new(),
        })
    }

    fn next_request_id(&mut self) -> String {
        self.next_request_sequence += 1;
        format!("sms-req-{:016}", self.next_request_sequence)
    }

    fn record_request(&mut self) {
        self.request_count += 1;
    }

    fn record_success(&mut self, channel: &str, duration_ms: u128) {
        self.success_count += 1;
        *self
            .per_channel_success
            .entry(channel.to_string())
            .or_insert(0) += 1;
        self.response_times_ms.push(duration_ms);
        if self.response_times_ms.len() > 1000 {
            self.response_times_ms.remove(0);
        }
        self.reset_channel_circuit(channel);
    }

    fn record_failure(&mut self) {
        self.failure_count += 1;
    }

    fn record_channel_failure_attempt(&mut self, channel: &str) {
        *self
            .per_channel_failure
            .entry(channel.to_string())
            .or_insert(0) += 1;

        let threshold = channel_failure_threshold();
        if threshold == 0 {
            return;
        }

        let now = now_unix_epoch_ms();
        let state = self
            .channel_circuits
            .entry(channel.to_string())
            .or_default();
        state.consecutive_failures = state.consecutive_failures.saturating_add(1);

        if state.consecutive_failures >= threshold {
            state.consecutive_failures = 0;
            state.open_until_ms = Some(now.saturating_add(channel_cooldown_ms()));
        }
    }

    fn reset_channel_circuit(&mut self, channel: &str) {
        if let Some(state) = self.channel_circuits.get_mut(channel) {
            state.consecutive_failures = 0;
            state.open_until_ms = None;
        }
    }

    fn is_channel_open(&mut self, channel: &str, now_ms: u128) -> bool {
        let Some(state) = self.channel_circuits.get_mut(channel) else {
            return false;
        };

        let Some(open_until_ms) = state.open_until_ms else {
            return false;
        };

        if open_until_ms > now_ms {
            return true;
        }

        state.open_until_ms = None;
        state.consecutive_failures = 0;
        false
    }

    fn find_history_by_idempotency(&self, key: &str) -> Result<Option<HistoryEntry>, String> {
        self.history_store.find_success_by_idempotency(key)
    }

    fn count_recent_for_contact(
        &self,
        contact: &str,
        window_seconds: u64,
    ) -> Result<usize, String> {
        self.history_store
            .count_recent_for_contact(contact, window_seconds)
    }

    fn insert_history(&self, entry: &HistoryEntry) -> Result<u64, InsertHistoryError> {
        self.history_store.insert_history(entry)
    }

    fn load_recent_history(&self, limit: usize) -> Result<Vec<HistoryEntry>, String> {
        self.history_store.load_recent_history(limit)
    }

    fn history_stats(&self) -> Result<Vec<HistoryStatsItem>, String> {
        self.history_store.history_stats()
    }

    fn metrics_snapshot(&self) -> MetricsSnapshot {
        let avg = if self.response_times_ms.is_empty() {
            0.0
        } else {
            self.response_times_ms.iter().sum::<u128>() as f64 / self.response_times_ms.len() as f64
        };

        MetricsSnapshot {
            requests: self.request_count,
            successes: self.success_count,
            failures: self.failure_count,
            per_channel_success: self.per_channel_success.clone(),
            per_channel_failure: self.per_channel_failure.clone(),
            avg_response_ms: avg,
        }
    }
}

fn now_unix_epoch_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

fn created_at_iso_ms(unix_ms: u128) -> String {
    let seconds = (unix_ms / 1000) as i64;
    let nanos = ((unix_ms % 1000) * 1_000_000) as u32;
    chrono::DateTime::from_timestamp(seconds, nanos)
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_else(|| "1970-01-01T00:00:00+00:00".to_string())
}

fn service_api_key() -> Option<String> {
    std::env::var("SMS_SERVICE_API_KEY")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn unauthorized_response() -> (StatusCode, Json<BasicErrorResponse>) {
    (
        StatusCode::UNAUTHORIZED,
        Json(BasicErrorResponse {
            success: false,
            error: "Unauthorized".to_string(),
        }),
    )
}

fn is_authorized(headers: &HeaderMap) -> bool {
    let Some(required_key) = service_api_key() else {
        return true;
    };

    headers
        .get("x-api-key")
        .and_then(|value| value.to_str().ok())
        .map(|value| value == required_key)
        .unwrap_or(false)
}

fn canonicalize_channel(value: &str) -> Result<String, String> {
    let normalized = value.trim().to_lowercase();
    let mapped = match normalized.as_str() {
        "sms" => "sms_twilio",
        "whatsapp" => "whatsapp_twilio",
        "custom" => "custom_http",
        _ => normalized.as_str(),
    };

    if SUPPORTED_CHANNELS.contains(&mapped) {
        Ok(mapped.to_string())
    } else {
        Err(format!("Unsupported SMS verification channel: {value}"))
    }
}

fn build_default_sequence() -> Result<Vec<String>, String> {
    let primary = std::env::var("SMS_DEFAULT_CHANNEL").unwrap_or_else(|_| "sms_twilio".to_string());
    let fallback = std::env::var("SMS_FALLBACK_CHANNELS")
        .unwrap_or_default()
        .split(',')
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>();

    let mut sequence = vec![primary];
    sequence.extend(fallback);

    build_sequence(sequence)
}

fn build_sequence(channels: Vec<String>) -> Result<Vec<String>, String> {
    let mut sequence = Vec::new();

    for channel in channels {
        let canonical = canonicalize_channel(&channel)?;
        if !sequence.contains(&canonical) {
            sequence.push(canonical);
        }
    }

    if sequence.is_empty() {
        let default = canonicalize_channel("sms_twilio")?;
        sequence.push(default);
    }

    Ok(sequence)
}

fn normalize_phone_number(phone: &str) -> Result<String, String> {
    let sanitized: String = phone
        .chars()
        .filter(|ch| ch.is_ascii_digit() || *ch == '+')
        .collect();
    if sanitized.is_empty() {
        return Err("Invalid phone number".to_string());
    }

    if sanitized.starts_with('+') {
        return Ok(sanitized);
    }

    let default_country = std::env::var("SMS_DEFAULT_COUNTRY_CODE")
        .ok()
        .map(|value| value.trim().trim_start_matches('+').to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            "SMS_DEFAULT_COUNTRY_CODE is required when sending to bare phone numbers".to_string()
        })?;

    Ok(format!("+{default_country}{sanitized}"))
}

fn normalize_destination(channel: &str, contact: &str) -> Result<String, String> {
    let contact = contact.trim();
    if contact.is_empty() {
        return Err("Contact value is required".to_string());
    }

    let phone_based = matches!(
        channel,
        "sms_twilio" | "sms_http" | "whatsapp_twilio" | "whatsapp_baileys"
    );

    if phone_based {
        normalize_phone_number(contact)
    } else {
        Ok(contact.to_string())
    }
}

fn rate_limit_window_seconds() -> u64 {
    std::env::var("SMS_RATE_LIMIT_WINDOW_SECONDS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(300)
}

fn rate_limit_max_per_contact() -> u64 {
    std::env::var("SMS_RATE_LIMIT_MAX_PER_CONTACT")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(5)
}

fn channel_failure_threshold() -> u64 {
    std::env::var("SMS_CHANNEL_FAILURE_THRESHOLD")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(DEFAULT_CHANNEL_FAILURE_THRESHOLD)
}

fn channel_cooldown_ms() -> u128 {
    std::env::var("SMS_CHANNEL_COOLDOWN_MS")
        .ok()
        .and_then(|value| value.parse::<u128>().ok())
        .unwrap_or(DEFAULT_CHANNEL_COOLDOWN_MS)
}

async fn health() -> Json<ServiceStatus> {
    Json(ServiceStatus {
        status: "ok",
        service: SERVICE_NAME,
    })
}

fn db_error_response(error: String) -> axum::response::Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(BasicErrorResponse {
            success: false,
            error,
        }),
    )
        .into_response()
}

async fn send_sms(
    State(state): State<Arc<Mutex<SmsApiState>>>,
    headers: HeaderMap,
    Json(payload): Json<SendRequest>,
) -> impl IntoResponse {
    if !is_authorized(&headers) {
        return unauthorized_response().into_response();
    }

    if payload.contact.trim().is_empty() || payload.message.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(BasicErrorResponse {
                success: false,
                error: "contact and message are required".to_string(),
            }),
        )
            .into_response();
    }

    let idempotency_header = headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let idempotency_key = payload
        .idempotency_key
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or(idempotency_header);

    if idempotency_key
        .as_ref()
        .map(|value| value.len() > 128)
        .unwrap_or(false)
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(BasicErrorResponse {
                success: false,
                error: "Idempotency key too long".to_string(),
            }),
        )
            .into_response();
    }

    let mut state = state.lock().await;
    let contact = payload.contact.clone();

    if let Some(key) = idempotency_key.as_deref() {
        let existing = match state.find_history_by_idempotency(key) {
            Ok(value) => value,
            Err(error) => return db_error_response(error),
        };

        if let Some(existing) = existing {
            return (
                StatusCode::OK,
                Json(SendResponse {
                    success: true,
                    channel: existing.channel,
                    destination: existing.destination,
                    idempotent: Some(true),
                }),
            )
                .into_response();
        }
    }

    let max_per_contact = rate_limit_max_per_contact();
    if max_per_contact > 0 {
        let recent = match state.count_recent_for_contact(&contact, rate_limit_window_seconds()) {
            Ok(value) => value,
            Err(error) => return db_error_response(error),
        };

        if recent >= max_per_contact as usize {
            return (
                StatusCode::TOO_MANY_REQUESTS,
                Json(BasicErrorResponse {
                    success: false,
                    error: "Rate limit exceeded for this contact".to_string(),
                }),
            )
                .into_response();
        }
    }

    state.record_request();
    let request_id = state.next_request_id();
    let started_at_ms = now_unix_epoch_ms();

    let channels_result = if let Some(channels) = payload.channels.clone() {
        if channels.is_empty() {
            build_default_sequence()
        } else {
            build_sequence(channels)
        }
    } else {
        build_default_sequence()
    };

    let channels = match channels_result {
        Ok(channels) => channels,
        Err(error) => {
            state.record_failure();
            state.record_channel_failure_attempt("unknown");
            let insert_result = state.insert_history(&HistoryEntry {
                id: 0,
                request_id,
                idempotency_key,
                contact,
                destination: "unknown".to_string(),
                channel: "unknown".to_string(),
                status: "failed".to_string(),
                error: Some(error.clone()),
                metadata: payload.metadata,
                created_at_ms: now_unix_epoch_ms(),
            });
            if let Err(InsertHistoryError::Other(err)) = insert_result {
                return db_error_response(err);
            }

            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(BasicErrorResponse {
                    success: false,
                    error,
                }),
            )
                .into_response();
        }
    };

    let now_ms = now_unix_epoch_ms();
    let available_channels = channels
        .into_iter()
        .filter(|channel| !state.is_channel_open(channel, now_ms))
        .collect::<Vec<_>>();

    if available_channels.is_empty() {
        state.record_failure();
        state.record_channel_failure_attempt("unknown");
        let error = "All configured channels are in cooldown".to_string();
        let insert_result = state.insert_history(&HistoryEntry {
            id: 0,
            request_id,
            idempotency_key,
            contact: contact.clone(),
            destination: contact,
            channel: "unknown".to_string(),
            status: "failed".to_string(),
            error: Some(error.clone()),
            metadata: payload.metadata,
            created_at_ms: now_unix_epoch_ms(),
        });
        if let Err(InsertHistoryError::Other(err)) = insert_result {
            return db_error_response(err);
        }

        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(BasicErrorResponse {
                success: false,
                error,
            }),
        )
            .into_response();
    }

    let mut last_channel = "unknown".to_string();
    let mut last_error = None::<String>;

    for channel in available_channels {
        last_channel = channel.clone();
        match normalize_destination(&channel, &contact) {
            Ok(destination) => {
                let duration_ms = now_unix_epoch_ms().saturating_sub(started_at_ms);
                state.record_success(&channel, duration_ms);

                let entry = HistoryEntry {
                    id: 0,
                    request_id: request_id.clone(),
                    idempotency_key: idempotency_key.clone(),
                    contact: contact.clone(),
                    destination: destination.clone(),
                    channel: channel.clone(),
                    status: "success".to_string(),
                    error: None,
                    metadata: payload.metadata.clone(),
                    created_at_ms: now_unix_epoch_ms(),
                };

                match state.insert_history(&entry) {
                    Ok(_) => {}
                    Err(InsertHistoryError::DuplicateIdempotency) => {
                        if let Some(key) = idempotency_key.as_deref() {
                            let existing = match state.find_history_by_idempotency(key) {
                                Ok(value) => value,
                                Err(error) => return db_error_response(error),
                            };
                            if let Some(existing) = existing {
                                return (
                                    StatusCode::OK,
                                    Json(SendResponse {
                                        success: true,
                                        channel: existing.channel,
                                        destination: existing.destination,
                                        idempotent: Some(true),
                                    }),
                                )
                                    .into_response();
                            }
                        }
                        return db_error_response(
                            "Idempotency conflict but no prior record found".to_string(),
                        );
                    }
                    Err(InsertHistoryError::Other(err)) => return db_error_response(err),
                }

                return (
                    StatusCode::OK,
                    Json(SendResponse {
                        success: true,
                        channel,
                        destination,
                        idempotent: None,
                    }),
                )
                    .into_response();
            }
            Err(error) => {
                state.record_channel_failure_attempt(&channel);
                last_error = Some(error);
            }
        }
    }

    state.record_failure();
    let error = last_error.unwrap_or_else(|| "No channel could process this request".to_string());
    let insert_result = state.insert_history(&HistoryEntry {
        id: 0,
        request_id,
        idempotency_key,
        contact: contact.clone(),
        destination: contact,
        channel: last_channel,
        status: "failed".to_string(),
        error: Some(error.clone()),
        metadata: payload.metadata,
        created_at_ms: now_unix_epoch_ms(),
    });
    if let Err(InsertHistoryError::Other(err)) = insert_result {
        return db_error_response(err);
    }

    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(BasicErrorResponse {
            success: false,
            error,
        }),
    )
        .into_response()
}

async fn metrics(
    State(state): State<Arc<Mutex<SmsApiState>>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if !is_authorized(&headers) {
        return unauthorized_response().into_response();
    }

    let state = state.lock().await;
    let _ = state.started_at.elapsed();
    let history = match state.history_stats() {
        Ok(value) => value,
        Err(error) => return db_error_response(error),
    };

    (
        StatusCode::OK,
        Json(MetricsResponse {
            success: true,
            metrics: state.metrics_snapshot(),
            history,
        }),
    )
        .into_response()
}

async fn history(
    State(state): State<Arc<Mutex<SmsApiState>>>,
    headers: HeaderMap,
    Query(query): Query<HistoryQuery>,
) -> impl IntoResponse {
    if !is_authorized(&headers) {
        return unauthorized_response().into_response();
    }

    let state = state.lock().await;
    let limit = query
        .limit
        .unwrap_or(HISTORY_DEFAULT_LIMIT)
        .min(HISTORY_MAX_LIMIT)
        .max(1);

    let entries_raw = match state.load_recent_history(limit) {
        Ok(entries) => entries,
        Err(error) => return db_error_response(error),
    };

    let entries = entries_raw
        .into_iter()
        .map(|entry| HistoryEntryPayload {
            id: entry.id,
            request_id: entry.request_id,
            idempotency_key: entry.idempotency_key,
            contact: entry.contact,
            destination: entry.destination,
            channel: entry.channel,
            status: entry.status,
            error: entry.error,
            metadata: entry.metadata,
            created_at: created_at_iso_ms(entry.created_at_ms),
        })
        .collect::<Vec<_>>();

    (
        StatusCode::OK,
        Json(HistoryResponse {
            success: true,
            entries,
        }),
    )
        .into_response()
}

fn listen_port(default_port: u16) -> u16 {
    std::env::var("PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(default_port)
}

fn build_router(state: Arc<Mutex<SmsApiState>>) -> Router {
    Router::new()
        .route("/api/send", post(send_sms))
        .route("/metrics", get(metrics))
        .route("/history", get(history))
        .route("/health", get(health))
        .with_state(state)
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let state = SmsApiState::new().expect("sms api state should initialize");
    let app = build_router(Arc::new(Mutex::new(state)));

    let addr = SocketAddr::from(([0, 0, 0, 0], listen_port(DEFAULT_PORT)));
    tracing::info!(service = SERVICE_NAME, %addr, "startup");

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .expect("server failed");
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Method, Request};
    use hyper::body::to_bytes;
    use serde_json::{json, Value};
    use serial_test::serial;
    use tempfile::NamedTempFile;
    use tower::ServiceExt;

    fn unique_db_path() -> String {
        let file = NamedTempFile::new().expect("temp file should create");
        let path = file.path().to_path_buf();
        drop(file);
        let _ = std::fs::remove_file(&path);
        path.to_string_lossy().to_string()
    }

    fn reset_test_env() {
        std::env::remove_var("SMS_SERVICE_API_KEY");
        std::env::remove_var("SMS_DEFAULT_COUNTRY_CODE");
        std::env::remove_var("SMS_CHANNEL_FAILURE_THRESHOLD");
        std::env::remove_var("SMS_CHANNEL_COOLDOWN_MS");
        std::env::remove_var("SMS_RATE_LIMIT_MAX_PER_CONTACT");
        std::env::remove_var("SMS_RATE_LIMIT_WINDOW_SECONDS");
        std::env::remove_var("SMS_DEFAULT_CHANNEL");
        std::env::remove_var("SMS_FALLBACK_CHANNELS");
        std::env::remove_var("SMS_HISTORY_DB_PATH");
    }

    fn test_app(db_path: &str) -> Router {
        std::env::set_var("SMS_HISTORY_DB_PATH", db_path);
        build_router(Arc::new(Mutex::new(
            SmsApiState::new().expect("state should initialize"),
        )))
    }

    async fn request(app: &Router, method: Method, uri: &str, body: Value) -> (StatusCode, Value) {
        let request = Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .expect("request should build");

        let response = app
            .clone()
            .oneshot(request)
            .await
            .expect("request should execute");
        let status = response.status();
        let bytes = to_bytes(response.into_body())
            .await
            .expect("body should read");
        let payload = serde_json::from_slice::<Value>(&bytes).expect("json should parse");
        (status, payload)
    }

    async fn get(app: &Router, uri: &str) -> (StatusCode, Value) {
        let request = Request::builder()
            .method(Method::GET)
            .uri(uri)
            .body(Body::empty())
            .expect("request should build");

        let response = app
            .clone()
            .oneshot(request)
            .await
            .expect("request should execute");
        let status = response.status();
        let bytes = to_bytes(response.into_body())
            .await
            .expect("body should read");
        let payload = serde_json::from_slice::<Value>(&bytes).expect("json should parse");
        (status, payload)
    }

    fn sample_payload(contact: &str, idempotency_key: Option<&str>) -> Value {
        json!({
            "contact": contact,
            "message": "Your Universus code is 123456",
            "channels": ["sms", "telegram"],
            "metadata": { "userId": 42 },
            "idempotencyKey": idempotency_key
        })
    }

    #[tokio::test]
    #[serial]
    async fn send_accepts_legacy_payload_shape() {
        reset_test_env();
        let db = unique_db_path();
        std::env::set_var("SMS_DEFAULT_COUNTRY_CODE", "1");
        let app = test_app(&db);

        let (status, payload) = request(
            &app,
            Method::POST,
            "/api/send",
            sample_payload("+12065550123", None),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(payload["success"], true);
        assert_eq!(payload["channel"], "sms_twilio");
        assert_eq!(payload["destination"], "+12065550123");
    }

    #[tokio::test]
    #[serial]
    async fn send_replays_idempotent_success() {
        reset_test_env();
        let db = unique_db_path();
        std::env::set_var("SMS_DEFAULT_COUNTRY_CODE", "1");
        let app = test_app(&db);

        let _ = request(
            &app,
            Method::POST,
            "/api/send",
            sample_payload("+12065550123", Some("idem-1")),
        )
        .await;

        let (status, replay) = request(
            &app,
            Method::POST,
            "/api/send",
            sample_payload("+12065550123", Some("idem-1")),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(replay["success"], true);
        assert_eq!(replay["idempotent"], true);
    }

    #[tokio::test]
    #[serial]
    async fn idempotency_persists_across_app_restarts() {
        reset_test_env();
        let db = unique_db_path();
        std::env::set_var("SMS_DEFAULT_COUNTRY_CODE", "1");

        let app_a = test_app(&db);
        let (first_status, _) = request(
            &app_a,
            Method::POST,
            "/api/send",
            sample_payload("+12065550123", Some("idem-persist-1")),
        )
        .await;
        assert_eq!(first_status, StatusCode::OK);

        let app_b = test_app(&db);
        let (second_status, replay) = request(
            &app_b,
            Method::POST,
            "/api/send",
            sample_payload("+12065550123", Some("idem-persist-1")),
        )
        .await;
        assert_eq!(second_status, StatusCode::OK);
        assert_eq!(replay["idempotent"], true);

        let (history_status, history_payload) = get(&app_b, "/history?limit=10").await;
        assert_eq!(history_status, StatusCode::OK);
        assert_eq!(history_payload["entries"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    #[serial]
    async fn circuit_opens_after_threshold_and_blocks_channel() {
        reset_test_env();
        let db = unique_db_path();
        std::env::set_var("SMS_CHANNEL_FAILURE_THRESHOLD", "1");
        std::env::set_var("SMS_CHANNEL_COOLDOWN_MS", "60000");
        let app = test_app(&db);

        let first_payload = json!({
            "contact": "2065550123",
            "message": "Code",
            "channels": ["sms"]
        });
        let (first_status, _) = request(&app, Method::POST, "/api/send", first_payload).await;
        assert_eq!(first_status, StatusCode::INTERNAL_SERVER_ERROR);

        let second_payload = json!({
            "contact": "2065550123",
            "message": "Code",
            "channels": ["sms"]
        });
        let (second_status, second_response) =
            request(&app, Method::POST, "/api/send", second_payload).await;
        assert_eq!(second_status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            second_response["error"],
            "All configured channels are in cooldown"
        );
    }

    #[tokio::test]
    #[serial]
    async fn metrics_and_history_match_legacy_shape() {
        reset_test_env();
        let db = unique_db_path();
        std::env::set_var("SMS_DEFAULT_COUNTRY_CODE", "1");
        let app = test_app(&db);

        let _ = request(
            &app,
            Method::POST,
            "/api/send",
            sample_payload("+12065550123", None),
        )
        .await;

        let (metrics_status, metrics_payload) = get(&app, "/metrics").await;
        assert_eq!(metrics_status, StatusCode::OK);
        assert_eq!(metrics_payload["success"], true);
        assert_eq!(metrics_payload["metrics"]["requests"], 1);
        assert!(metrics_payload["history"].is_array());

        let (history_status, history_payload) = get(&app, "/history?limit=1").await;
        assert_eq!(history_status, StatusCode::OK);
        assert_eq!(history_payload["success"], true);
        assert_eq!(history_payload["entries"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    #[serial]
    async fn health_reports_legacy_service_name() {
        reset_test_env();
        let db = unique_db_path();
        let app = test_app(&db);

        let (status, payload) = get(&app, "/health").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(payload["status"], "ok");
        assert_eq!(payload["service"], "sms");
    }

    #[tokio::test]
    #[serial]
    async fn send_requires_api_key_when_configured() {
        reset_test_env();
        let db = unique_db_path();
        std::env::set_var("SMS_SERVICE_API_KEY", "secret");
        std::env::set_var("SMS_DEFAULT_COUNTRY_CODE", "1");
        let app = test_app(&db);

        let (status, payload) = request(
            &app,
            Method::POST,
            "/api/send",
            sample_payload("+12065550123", None),
        )
        .await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(payload["success"], false);
        assert_eq!(payload["error"], "Unauthorized");
    }
}
