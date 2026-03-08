#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ---------------------------------------------------------------------------
// Payment Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaymentRequest {
    pub user_id: String,
    pub amount_cents: i64,
    pub currency: String,
    pub description: String,
    pub product_id: String,
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaymentResponse {
    pub payment_id: String,
    pub provider: String,
    pub status: PaymentStatus,
    pub redirect_url: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaymentStatus {
    Pending,
    Completed,
    Failed,
    Refunded,
    Cancelled,
}

impl Display for PaymentStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            Self::Pending => "Pending",
            Self::Completed => "Completed",
            Self::Failed => "Failed",
            Self::Refunded => "Refunded",
            Self::Cancelled => "Cancelled",
        };
        write!(f, "{label}")
    }
}

impl FromStr for PaymentStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(Self::Pending),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "refunded" => Ok(Self::Refunded),
            "cancelled" => Ok(Self::Cancelled),
            other => Err(format!("unknown payment status: {other}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefundResponse {
    pub refund_id: String,
    pub payment_id: String,
    pub amount_cents: i64,
    pub status: PaymentStatus,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transaction {
    pub id: String,
    pub user_id: String,
    pub payment_id: String,
    pub amount_cents: i64,
    pub currency: String,
    pub product_id: String,
    pub product_name: String,
    pub status: PaymentStatus,
    pub provider: String,
    pub created_at: String,
}

// ---------------------------------------------------------------------------
// Payment Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaymentError {
    InvalidAmount,
    InvalidCurrency,
    PaymentNotFound,
    ProviderUnavailable,
    AlreadyRefunded,
    PartialRefundExceedsAmount,
    ProcessingFailed(String),
}

impl Display for PaymentError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidAmount => write!(f, "invalid payment amount"),
            Self::InvalidCurrency => write!(f, "unsupported currency"),
            Self::PaymentNotFound => write!(f, "payment not found"),
            Self::ProviderUnavailable => write!(f, "payment provider unavailable"),
            Self::AlreadyRefunded => write!(f, "payment has already been refunded"),
            Self::PartialRefundExceedsAmount => {
                write!(f, "partial refund amount exceeds payment amount")
            }
            Self::ProcessingFailed(msg) => write!(f, "payment processing failed: {msg}"),
        }
    }
}

impl Error for PaymentError {}

// ---------------------------------------------------------------------------
// Payment Provider Trait
// ---------------------------------------------------------------------------

pub trait PaymentProvider: Send + Sync {
    fn name(&self) -> &str;
    fn create_payment(&self, request: &PaymentRequest) -> Result<PaymentResponse, PaymentError>;
    fn verify_payment(&self, payment_id: &str) -> Result<PaymentStatus, PaymentError>;
    fn refund_payment(
        &self,
        payment_id: &str,
        amount: Option<i64>,
    ) -> Result<RefundResponse, PaymentError>;
    fn list_transactions(
        &self,
        user_id: &str,
        limit: usize,
    ) -> Result<Vec<Transaction>, PaymentError>;
}

// ---------------------------------------------------------------------------
// Payment Validation
// ---------------------------------------------------------------------------

pub const SUPPORTED_CURRENCIES: &[&str] = &["USD", "EUR", "GBP"];

pub fn validate_payment_request(req: &PaymentRequest) -> Result<(), PaymentError> {
    if req.amount_cents <= 0 {
        return Err(PaymentError::InvalidAmount);
    }
    if req.user_id.trim().is_empty() {
        return Err(PaymentError::ProcessingFailed(
            "user_id must not be empty".to_owned(),
        ));
    }
    if req.product_id.trim().is_empty() {
        return Err(PaymentError::ProcessingFailed(
            "product_id must not be empty".to_owned(),
        ));
    }
    if !SUPPORTED_CURRENCIES.contains(&req.currency.as_str()) {
        return Err(PaymentError::InvalidCurrency);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Product Catalog
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Product {
    pub id: String,
    pub name: String,
    pub description: String,
    pub price_cents: i64,
    pub currency: String,
    pub dark_matter_amount: i64,
    pub is_active: bool,
    pub bonus_percent: Option<i32>,
}

pub fn default_product_catalog() -> Vec<Product> {
    vec![
        Product {
            id: "dm-small".to_owned(),
            name: "Small Dark Matter Pack".to_owned(),
            description: "500 units of Dark Matter".to_owned(),
            price_cents: 299,
            currency: "USD".to_owned(),
            dark_matter_amount: 500,
            is_active: true,
            bonus_percent: None,
        },
        Product {
            id: "dm-medium".to_owned(),
            name: "Medium Dark Matter Pack".to_owned(),
            description: "2500 units of Dark Matter".to_owned(),
            price_cents: 999,
            currency: "USD".to_owned(),
            dark_matter_amount: 2500,
            is_active: true,
            bonus_percent: Some(10),
        },
        Product {
            id: "dm-large".to_owned(),
            name: "Large Dark Matter Pack".to_owned(),
            description: "6500 units of Dark Matter".to_owned(),
            price_cents: 1999,
            currency: "USD".to_owned(),
            dark_matter_amount: 6500,
            is_active: true,
            bonus_percent: Some(30),
        },
        Product {
            id: "dm-premium".to_owned(),
            name: "Premium Dark Matter Pack".to_owned(),
            description: "15000 units of Dark Matter".to_owned(),
            price_cents: 3999,
            currency: "USD".to_owned(),
            dark_matter_amount: 15000,
            is_active: true,
            bonus_percent: Some(50),
        },
    ]
}

pub fn find_product(id: &str) -> Option<Product> {
    default_product_catalog().into_iter().find(|p| p.id == id)
}

pub fn active_products() -> Vec<Product> {
    default_product_catalog()
        .into_iter()
        .filter(|p| p.is_active)
        .collect()
}

// ---------------------------------------------------------------------------
// Webhook Types & Parsing
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebhookEvent {
    pub event_id: String,
    pub event_type: WebhookEventType,
    pub payment_id: String,
    pub data: Value,
    pub timestamp: i64,
    pub signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WebhookEventType {
    PaymentCompleted,
    PaymentFailed,
    RefundProcessed,
    DisputeOpened,
}

impl Display for WebhookEventType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            Self::PaymentCompleted => "PaymentCompleted",
            Self::PaymentFailed => "PaymentFailed",
            Self::RefundProcessed => "RefundProcessed",
            Self::DisputeOpened => "DisputeOpened",
        };
        write!(f, "{label}")
    }
}

impl FromStr for WebhookEventType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "PaymentCompleted" => Ok(Self::PaymentCompleted),
            "PaymentFailed" => Ok(Self::PaymentFailed),
            "RefundProcessed" => Ok(Self::RefundProcessed),
            "DisputeOpened" => Ok(Self::DisputeOpened),
            other => Err(format!("unknown webhook event type: {other}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebhookParseError {
    EmptyPayload,
    InvalidJson(String),
    MissingField(&'static str),
}

impl Display for WebhookParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyPayload => write!(f, "webhook payload is empty"),
            Self::InvalidJson(err) => write!(f, "webhook payload is not valid JSON: {err}"),
            Self::MissingField(field) => {
                write!(f, "webhook payload is missing required field '{field}'")
            }
        }
    }
}

impl Error for WebhookParseError {}

pub fn parse_webhook_event(payload: &str) -> Result<WebhookEvent, WebhookParseError> {
    if payload.trim().is_empty() {
        return Err(WebhookParseError::EmptyPayload);
    }

    let root: Value =
        serde_json::from_str(payload).map_err(|e| WebhookParseError::InvalidJson(e.to_string()))?;

    let obj = root.as_object().ok_or(WebhookParseError::InvalidJson(
        "expected a JSON object".to_owned(),
    ))?;

    let event_id = require_string(obj, "event_id")?;
    let event_type_str = require_string(obj, "event_type")?;
    let payment_id = require_string(obj, "payment_id")?;
    let signature = require_string(obj, "signature")?;

    let event_type = WebhookEventType::from_str(&event_type_str)
        .map_err(|e| WebhookParseError::InvalidJson(e))?;

    let data = obj
        .get("data")
        .cloned()
        .ok_or(WebhookParseError::MissingField("data"))?;

    let timestamp = obj
        .get("timestamp")
        .and_then(|v| v.as_i64())
        .ok_or(WebhookParseError::MissingField("timestamp"))?;

    Ok(WebhookEvent {
        event_id,
        event_type,
        payment_id,
        data,
        timestamp,
        signature,
    })
}

fn require_string(
    obj: &serde_json::Map<String, Value>,
    field: &'static str,
) -> Result<String, WebhookParseError> {
    obj.get(field)
        .and_then(|v| v.as_str())
        .map(|s| s.to_owned())
        .ok_or(WebhookParseError::MissingField(field))
}

/// Simple HMAC-style webhook signature verification.
///
/// Computes a basic signature from `payload` and `secret` by summing their
/// byte values and compares it against the provided `signature` hex string.
/// This is intentionally a toy implementation suitable for local/mock usage.
pub fn verify_webhook_signature(payload: &str, signature: &str, secret: &str) -> bool {
    let computed = compute_signature(payload, secret);
    constant_time_eq(computed.as_bytes(), signature.as_bytes())
}

fn compute_signature(payload: &str, secret: &str) -> String {
    // Simple keyed hash: XOR-fold payload bytes with secret bytes, accumulate.
    let secret_bytes = secret.as_bytes();
    if secret_bytes.is_empty() {
        return String::new();
    }
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325; // FNV offset basis
    for (i, &b) in payload.as_bytes().iter().enumerate() {
        let key_byte = secret_bytes[i % secret_bytes.len()];
        hash ^= (b ^ key_byte) as u64;
        hash = hash.wrapping_mul(0x0100_0000_01b3); // FNV prime
    }
    format!("{hash:016x}")
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

// ---------------------------------------------------------------------------
// Logging / Mock Payment Provider
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct StoredPayment {
    request: PaymentRequest,
    status: PaymentStatus,
    payment_id: String,
    created_at: String,
}

#[derive(Debug)]
pub struct LoggingPaymentProvider {
    name: String,
    sequence: AtomicU64,
    payments: Mutex<HashMap<String, StoredPayment>>,
}

impl LoggingPaymentProvider {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            sequence: AtomicU64::new(0),
            payments: Mutex::new(HashMap::new()),
        }
    }

    pub fn payment_count(&self) -> u64 {
        self.sequence.load(Ordering::Relaxed)
    }

    fn next_id(&self, prefix: &str) -> String {
        let seq = self.sequence.fetch_add(1, Ordering::Relaxed) + 1;
        format!("{prefix}-{seq}")
    }

    fn now_iso() -> String {
        // Fixed timestamp for deterministic mock behaviour.
        "2026-01-01T00:00:00Z".to_owned()
    }
}

impl Default for LoggingPaymentProvider {
    fn default() -> Self {
        Self::new("logging")
    }
}

impl PaymentProvider for LoggingPaymentProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn create_payment(&self, request: &PaymentRequest) -> Result<PaymentResponse, PaymentError> {
        validate_payment_request(request)?;

        let payment_id = self.next_id("pay");
        let created_at = Self::now_iso();

        println!(
            "payment-create provider={} payment_id={} user={} amount={} {}",
            self.name, payment_id, request.user_id, request.amount_cents, request.currency
        );

        let stored = StoredPayment {
            request: request.clone(),
            status: PaymentStatus::Pending,
            payment_id: payment_id.clone(),
            created_at: created_at.clone(),
        };

        self.payments
            .lock()
            .expect("lock poisoned")
            .insert(payment_id.clone(), stored);

        Ok(PaymentResponse {
            payment_id,
            provider: self.name.clone(),
            status: PaymentStatus::Pending,
            redirect_url: None,
            created_at,
        })
    }

    fn verify_payment(&self, payment_id: &str) -> Result<PaymentStatus, PaymentError> {
        let store = self.payments.lock().expect("lock poisoned");
        let payment = store.get(payment_id).ok_or(PaymentError::PaymentNotFound)?;
        Ok(payment.status.clone())
    }

    fn refund_payment(
        &self,
        payment_id: &str,
        amount: Option<i64>,
    ) -> Result<RefundResponse, PaymentError> {
        let mut store = self.payments.lock().expect("lock poisoned");
        let payment = store
            .get_mut(payment_id)
            .ok_or(PaymentError::PaymentNotFound)?;

        if payment.status == PaymentStatus::Refunded {
            return Err(PaymentError::AlreadyRefunded);
        }

        let refund_amount = amount.unwrap_or(payment.request.amount_cents);
        if refund_amount <= 0 {
            return Err(PaymentError::InvalidAmount);
        }
        if refund_amount > payment.request.amount_cents {
            return Err(PaymentError::PartialRefundExceedsAmount);
        }

        payment.status = PaymentStatus::Refunded;

        let refund_id = format!("ref-{payment_id}");
        let created_at = Self::now_iso();

        println!(
            "payment-refund provider={} payment_id={} refund_id={} amount={}",
            self.name, payment_id, refund_id, refund_amount
        );

        Ok(RefundResponse {
            refund_id,
            payment_id: payment_id.to_owned(),
            amount_cents: refund_amount,
            status: PaymentStatus::Refunded,
            created_at,
        })
    }

    fn list_transactions(
        &self,
        user_id: &str,
        limit: usize,
    ) -> Result<Vec<Transaction>, PaymentError> {
        let store = self.payments.lock().expect("lock poisoned");
        let mut txns: Vec<Transaction> = store
            .values()
            .filter(|p| p.request.user_id == user_id)
            .map(|p| {
                let product_name = find_product(&p.request.product_id)
                    .map(|prod| prod.name)
                    .unwrap_or_else(|| "Unknown Product".to_owned());
                Transaction {
                    id: format!("txn-{}", p.payment_id),
                    user_id: p.request.user_id.clone(),
                    payment_id: p.payment_id.clone(),
                    amount_cents: p.request.amount_cents,
                    currency: p.request.currency.clone(),
                    product_id: p.request.product_id.clone(),
                    product_name,
                    status: p.status.clone(),
                    provider: self.name.clone(),
                    created_at: p.created_at.clone(),
                }
            })
            .collect();

        // Sort by payment_id for deterministic output.
        txns.sort_by(|a, b| a.payment_id.cmp(&b.payment_id));
        txns.truncate(limit);
        Ok(txns)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_request() -> PaymentRequest {
        PaymentRequest {
            user_id: "user-42".to_owned(),
            amount_cents: 999,
            currency: "USD".to_owned(),
            description: "Medium DM Pack".to_owned(),
            product_id: "dm-medium".to_owned(),
            metadata: None,
        }
    }

    // -- PaymentStatus --

    #[test]
    fn payment_status_display_roundtrip() {
        let statuses = vec![
            PaymentStatus::Pending,
            PaymentStatus::Completed,
            PaymentStatus::Failed,
            PaymentStatus::Refunded,
            PaymentStatus::Cancelled,
        ];
        for status in statuses {
            let text = status.to_string();
            let parsed: PaymentStatus = text.parse().expect("should parse");
            assert_eq!(parsed, status);
        }
    }

    #[test]
    fn payment_status_from_str_rejects_unknown() {
        let result = "Expired".parse::<PaymentStatus>();
        assert!(result.is_err());
    }

    // -- Validation --

    #[test]
    fn validate_payment_request_accepts_valid() {
        let req = fixture_request();
        assert!(validate_payment_request(&req).is_ok());
    }

    #[test]
    fn validate_payment_request_rejects_zero_amount() {
        let mut req = fixture_request();
        req.amount_cents = 0;
        assert_eq!(
            validate_payment_request(&req),
            Err(PaymentError::InvalidAmount)
        );
    }

    #[test]
    fn validate_payment_request_rejects_negative_amount() {
        let mut req = fixture_request();
        req.amount_cents = -100;
        assert_eq!(
            validate_payment_request(&req),
            Err(PaymentError::InvalidAmount)
        );
    }

    #[test]
    fn validate_payment_request_rejects_unsupported_currency() {
        let mut req = fixture_request();
        req.currency = "JPY".to_owned();
        assert_eq!(
            validate_payment_request(&req),
            Err(PaymentError::InvalidCurrency)
        );
    }

    #[test]
    fn validate_payment_request_rejects_empty_user_id() {
        let mut req = fixture_request();
        req.user_id = "  ".to_owned();
        assert!(matches!(
            validate_payment_request(&req),
            Err(PaymentError::ProcessingFailed(_))
        ));
    }

    #[test]
    fn validate_payment_request_rejects_empty_product_id() {
        let mut req = fixture_request();
        req.product_id = "".to_owned();
        assert!(matches!(
            validate_payment_request(&req),
            Err(PaymentError::ProcessingFailed(_))
        ));
    }

    // -- Product Catalog --

    #[test]
    fn default_catalog_has_four_products() {
        let catalog = default_product_catalog();
        assert_eq!(catalog.len(), 4);
    }

    #[test]
    fn find_product_returns_correct_product() {
        let product = find_product("dm-large").expect("should find dm-large");
        assert_eq!(product.dark_matter_amount, 6500);
        assert_eq!(product.price_cents, 1999);
        assert_eq!(product.bonus_percent, Some(30));
    }

    #[test]
    fn find_product_returns_none_for_unknown() {
        assert!(find_product("dm-ultra").is_none());
    }

    #[test]
    fn active_products_returns_all_defaults() {
        let active = active_products();
        assert_eq!(active.len(), 4);
        assert!(active.iter().all(|p| p.is_active));
    }

    // -- LoggingPaymentProvider --

    #[test]
    fn logging_provider_creates_payment_with_pending_status() {
        let provider = LoggingPaymentProvider::new("test-pay");
        let req = fixture_request();

        let resp = provider.create_payment(&req).expect("should succeed");
        assert_eq!(resp.provider, "test-pay");
        assert_eq!(resp.status, PaymentStatus::Pending);
        assert_eq!(resp.payment_id, "pay-1");
        assert_eq!(provider.payment_count(), 1);
    }

    #[test]
    fn logging_provider_verify_returns_stored_status() {
        let provider = LoggingPaymentProvider::new("test-pay");
        let req = fixture_request();

        let resp = provider.create_payment(&req).expect("create");
        let status = provider.verify_payment(&resp.payment_id).expect("verify");
        assert_eq!(status, PaymentStatus::Pending);
    }

    #[test]
    fn logging_provider_verify_unknown_payment_returns_not_found() {
        let provider = LoggingPaymentProvider::default();
        let result = provider.verify_payment("pay-999");
        assert_eq!(result, Err(PaymentError::PaymentNotFound));
    }

    #[test]
    fn logging_provider_refund_full_amount() {
        let provider = LoggingPaymentProvider::new("test-pay");
        let req = fixture_request();
        let resp = provider.create_payment(&req).expect("create");

        let refund = provider
            .refund_payment(&resp.payment_id, None)
            .expect("refund");
        assert_eq!(refund.amount_cents, 999);
        assert_eq!(refund.status, PaymentStatus::Refunded);

        // Verify status changed.
        let status = provider.verify_payment(&resp.payment_id).expect("verify");
        assert_eq!(status, PaymentStatus::Refunded);
    }

    #[test]
    fn logging_provider_refund_partial_amount() {
        let provider = LoggingPaymentProvider::new("test-pay");
        let req = fixture_request();
        let resp = provider.create_payment(&req).expect("create");

        let refund = provider
            .refund_payment(&resp.payment_id, Some(500))
            .expect("partial refund");
        assert_eq!(refund.amount_cents, 500);
    }

    #[test]
    fn logging_provider_refund_rejects_double_refund() {
        let provider = LoggingPaymentProvider::new("test-pay");
        let req = fixture_request();
        let resp = provider.create_payment(&req).expect("create");

        provider
            .refund_payment(&resp.payment_id, None)
            .expect("first refund");
        let err = provider
            .refund_payment(&resp.payment_id, None)
            .expect_err("second refund should fail");
        assert_eq!(err, PaymentError::AlreadyRefunded);
    }

    #[test]
    fn logging_provider_refund_rejects_excessive_partial() {
        let provider = LoggingPaymentProvider::new("test-pay");
        let req = fixture_request();
        let resp = provider.create_payment(&req).expect("create");

        let err = provider
            .refund_payment(&resp.payment_id, Some(9999))
            .expect_err("excessive refund");
        assert_eq!(err, PaymentError::PartialRefundExceedsAmount);
    }

    #[test]
    fn logging_provider_list_transactions_filters_by_user() {
        let provider = LoggingPaymentProvider::new("test-pay");

        let mut req1 = fixture_request();
        req1.user_id = "alice".to_owned();
        let mut req2 = fixture_request();
        req2.user_id = "bob".to_owned();
        let mut req3 = fixture_request();
        req3.user_id = "alice".to_owned();

        provider.create_payment(&req1).expect("create 1");
        provider.create_payment(&req2).expect("create 2");
        provider.create_payment(&req3).expect("create 3");

        let alice_txns = provider.list_transactions("alice", 10).expect("list");
        assert_eq!(alice_txns.len(), 2);
        assert!(alice_txns.iter().all(|t| t.user_id == "alice"));

        let bob_txns = provider.list_transactions("bob", 10).expect("list");
        assert_eq!(bob_txns.len(), 1);
    }

    #[test]
    fn logging_provider_list_transactions_respects_limit() {
        let provider = LoggingPaymentProvider::new("test-pay");

        for _ in 0..5 {
            provider.create_payment(&fixture_request()).expect("create");
        }

        let txns = provider.list_transactions("user-42", 3).expect("list");
        assert_eq!(txns.len(), 3);
    }

    // -- Webhook Parsing --

    #[test]
    fn parse_webhook_event_parses_valid_payload() {
        let payload = r#"{
            "event_id": "evt-1",
            "event_type": "PaymentCompleted",
            "payment_id": "pay-100",
            "data": {"amount": 999},
            "timestamp": 1700000000,
            "signature": "abc123"
        }"#;

        let event = parse_webhook_event(payload).expect("should parse");
        assert_eq!(event.event_id, "evt-1");
        assert_eq!(event.event_type, WebhookEventType::PaymentCompleted);
        assert_eq!(event.payment_id, "pay-100");
        assert_eq!(event.timestamp, 1700000000);
        assert_eq!(event.signature, "abc123");
    }

    #[test]
    fn parse_webhook_event_rejects_empty_payload() {
        let err = parse_webhook_event("  ").expect_err("should fail");
        assert_eq!(err, WebhookParseError::EmptyPayload);
    }

    #[test]
    fn parse_webhook_event_rejects_invalid_json() {
        let err = parse_webhook_event("{not valid}").expect_err("should fail");
        assert!(matches!(err, WebhookParseError::InvalidJson(_)));
    }

    #[test]
    fn parse_webhook_event_rejects_missing_field() {
        let payload = r#"{
            "event_id": "evt-1",
            "event_type": "PaymentCompleted",
            "data": {},
            "timestamp": 1700000000,
            "signature": "abc123"
        }"#;

        let err = parse_webhook_event(payload).expect_err("should fail");
        assert_eq!(err, WebhookParseError::MissingField("payment_id"));
    }

    // -- Webhook Signature --

    #[test]
    fn verify_webhook_signature_accepts_valid() {
        let payload = "test-payload";
        let secret = "my-secret";
        let sig = compute_signature(payload, secret);
        assert!(verify_webhook_signature(payload, &sig, secret));
    }

    #[test]
    fn verify_webhook_signature_rejects_wrong_signature() {
        assert!(!verify_webhook_signature("payload", "wrong-sig", "secret"));
    }

    #[test]
    fn verify_webhook_signature_rejects_tampered_payload() {
        let secret = "my-secret";
        let sig = compute_signature("original", secret);
        assert!(!verify_webhook_signature("tampered", &sig, secret));
    }

    // -- Error Display --

    #[test]
    fn payment_error_display() {
        assert_eq!(
            PaymentError::InvalidAmount.to_string(),
            "invalid payment amount"
        );
        assert_eq!(
            PaymentError::PaymentNotFound.to_string(),
            "payment not found"
        );
        assert_eq!(
            PaymentError::ProcessingFailed("timeout".to_owned()).to_string(),
            "payment processing failed: timeout"
        );
    }

    #[test]
    fn webhook_parse_error_display() {
        assert_eq!(
            WebhookParseError::EmptyPayload.to_string(),
            "webhook payload is empty"
        );
        assert_eq!(
            WebhookParseError::MissingField("event_id").to_string(),
            "webhook payload is missing required field 'event_id'"
        );
    }

    // -- Serialization round-trip --

    #[test]
    fn payment_request_serialization_roundtrip() {
        let req = fixture_request();
        let json = serde_json::to_string(&req).expect("serialize");
        let deserialized: PaymentRequest = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized, req);
    }

    #[test]
    fn product_serialization_roundtrip() {
        let product = find_product("dm-premium").expect("find");
        let json = serde_json::to_string(&product).expect("serialize");
        let deserialized: Product = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized, product);
    }
}
