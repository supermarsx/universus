use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
pub struct HistoryRecord {
    pub id: u64,
    pub request_id: String,
    pub idempotency_key: Option<String>,
    pub contact: String,
    pub destination: String,
    pub channel: String,
    pub status: String,
    pub error: Option<String>,
    pub metadata: Option<Value>,
    pub created_at_ms: u128,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HistoryRecordInput {
    pub request_id: String,
    pub idempotency_key: Option<String>,
    pub contact: String,
    pub destination: String,
    pub channel: String,
    pub status: String,
    pub error: Option<String>,
    pub metadata: Option<Value>,
    pub created_at_ms: u128,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HistoryStatsItem {
    pub channel: String,
    pub status: String,
    pub count: u64,
}
