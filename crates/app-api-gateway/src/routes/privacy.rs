use std::{net::SocketAddr, sync::Arc};

use app_privacy_worker::{ExportEncryptor, PrivacyKeyring};
use axum::body::Body;
use axum::extract::{
    rejection::{JsonRejection, QueryRejection},
    ConnectInfo, Path, Query,
};
use axum::http::{
    header::{AUTHORIZATION, CACHE_CONTROL, CONTENT_DISPOSITION, CONTENT_TYPE, PRAGMA},
    HeaderMap, HeaderValue, Request, StatusCode,
};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Extension, Json, Router};
use platform_db::{
    privacy_communication_category_is_essential, privacy_evidence_digest,
    CommunicationPreferenceRow, CommunicationPreferenceUpdate, ConsentStatus, ConsentUpdate,
    Database, PrivacyConsentRow, PrivacyError, PrivacyExportAvailability,
    PrivacyRequestCreateInput, PrivacyRequestDetail, PrivacyRequestEventRow, PrivacyRequestStatus,
    PrivacyRequestSummary, PrivacyRequestType, PRIVACY_COMMUNICATION_CATEGORIES,
    PRIVACY_COMMUNICATION_CHANNELS,
};
use serde::{Deserialize, Serialize};

use crate::auth_guard::AuthUser;
use crate::response::success;

const DEFAULT_POLICY_VERSION: &str = "privacy-v1";
const DEVELOPMENT_EVIDENCE_PEPPER: [u8; 32] = [0x44; 32];
const RESTRICTION_CONFIRMATION: &str = "RESTRICT MY ACCOUNT";
const ERASURE_CONFIRMATION: &str = "ERASE MY ACCOUNT";
const CORRECTION_CONFIRMATION: &str = "APPLY MY CORRECTIONS";
const CANCELLATION_CONFIRMATION: &str = "CANCEL REQUEST";
const DELIVERY_TOKEN_HEADER: &str = "x-privacy-delivery-token";
const MAX_DELIVERY_RESPONSE_BYTES: u64 = 64 * 1024 * 1024;

struct SecretBytes(Vec<u8>);

impl Drop for SecretBytes {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}

#[derive(Clone)]
struct PrivacyRouteConfig {
    policy_version: String,
    evidence_pepper: Option<Arc<SecretBytes>>,
    correction_encryptor: Option<ExportEncryptor>,
    delivery_bridge: Option<PrivacyDeliveryBridge>,
    available: bool,
}

#[derive(Clone)]
struct PrivacyDeliveryBridge {
    base_url: String,
    client: reqwest::Client,
}

impl PrivacyRouteConfig {
    fn from_env() -> Self {
        Self::from_lookup(&|name| std::env::var(name).ok())
    }

    fn from_lookup(lookup: &dyn Fn(&str) -> Option<String>) -> Self {
        let environment = ["UNIVERSUS_ENV", "APP_ENV", "ENVIRONMENT", "RUST_ENV"]
            .into_iter()
            .find_map(lookup)
            .unwrap_or_else(|| "development".to_string());
        let production_like = matches!(
            environment.trim().to_ascii_lowercase().as_str(),
            "production" | "prod" | "staging" | "stage"
        );
        let policy_version = lookup("PRIVACY_POLICY_VERSION")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| DEFAULT_POLICY_VERSION.to_string());
        let policy_valid = valid_short_identifier(&policy_version);
        let configured_pepper = lookup("PRIVACY_REQUEST_IP_PEPPER")
            .map(|value| value.trim().as_bytes().to_vec())
            .filter(|value| !value.is_empty());
        let configured_pepper_valid = configured_pepper
            .as_ref()
            .is_none_or(|pepper| (32..=1024).contains(&pepper.len()));
        let evidence_pepper = match configured_pepper {
            Some(pepper) if configured_pepper_valid => Some(Arc::new(SecretBytes(pepper))),
            Some(_) => None,
            None if production_like => None,
            None => Some(Arc::new(SecretBytes(DEVELOPMENT_EVIDENCE_PEPPER.to_vec()))),
        };
        let correction_encryptor = PrivacyKeyring::from_lookup(lookup)
            .and_then(|keyring| ExportEncryptor::from_keyring(keyring, 4096))
            .ok();
        let delivery_bridge = lookup("PRIVACY_WORKER_INTERNAL_URL")
            .map(|value| value.trim().trim_end_matches('/').to_string())
            .filter(|value| valid_internal_url(value))
            .and_then(|base_url| {
                reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(30))
                    .redirect(reqwest::redirect::Policy::none())
                    .build()
                    .ok()
                    .map(|client| PrivacyDeliveryBridge { base_url, client })
            });
        let available = policy_valid
            && configured_pepper_valid
            && (!production_like || evidence_pepper.is_some());
        Self {
            policy_version,
            evidence_pepper,
            correction_encryptor,
            delivery_bridge,
            available,
        }
    }

    fn digest(&self, kind: &str, value: &str) -> Result<[u8; 32], PrivacyError> {
        let pepper = self.evidence_pepper.as_ref().ok_or(PrivacyError::Database(
            "privacy evidence pepper is unavailable".to_string(),
        ))?;
        let mut evidence = Vec::with_capacity(kind.len() + value.len() + 1);
        evidence.extend_from_slice(kind.as_bytes());
        evidence.push(0);
        evidence.extend_from_slice(value.as_bytes());
        let digest = privacy_evidence_digest(&pepper.0, &evidence);
        evidence.fill(0);
        digest
    }
}

#[derive(Clone, Copy)]
struct PrivacyOwner {
    universe_id: i64,
    user_id: i32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RequestListQuery {
    limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreatePrivacyRequest {
    request_type: String,
    idempotency_key: String,
    confirmation: Option<String>,
    changes: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CancelPrivacyRequest {
    expected_version: i64,
    confirmation: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct UpdateConsentRequest {
    status: String,
    policy_version: String,
    expected_version: i64,
    confirmed: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct UpdateCommunicationRequest {
    enabled: bool,
    expected_version: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PrivacyExportPayload {
    ready: bool,
    expired: bool,
    expires_at_unix: i64,
    plaintext_size: i64,
    delivery_available: bool,
    delivery_status: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PrivacyRequestPayload {
    id: i32,
    request_type: &'static str,
    status: &'static str,
    requested_at_unix: i64,
    cooling_off_until_unix: Option<i64>,
    completed_at_unix: Option<i64>,
    cancelled_at_unix: Option<i64>,
    legal_hold_active: bool,
    retention_until_unix: i64,
    version: i64,
    cancellation_allowed: bool,
    export: Option<PrivacyExportPayload>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PrivacyRequestEventPayload {
    id: i64,
    event_type: String,
    from_status: Option<&'static str>,
    to_status: &'static str,
    actor_type: String,
    reason_code: Option<String>,
    request_version: i64,
    created_at_unix: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PrivacyRequestDetailPayload {
    request: PrivacyRequestPayload,
    timeline: Vec<PrivacyRequestEventPayload>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConsentPayload {
    purpose: String,
    channel: String,
    status: &'static str,
    lawful_basis: String,
    policy_version: String,
    collected_at_unix: i64,
    expires_at_unix: Option<i64>,
    version: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConsentCollectionPayload {
    current_policy_version: String,
    consents: Vec<ConsentPayload>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CommunicationPayload {
    channel: String,
    category: String,
    enabled: bool,
    explicitly_configured: bool,
    effective_allowed: bool,
    essential: bool,
    marketing_consent_current: bool,
    suppressed_by_restriction: bool,
    updated_at_unix: Option<i64>,
    version: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PrivacyApiError {
    success: bool,
    code: &'static str,
    error: &'static str,
}

pub fn router() -> Router {
    router_with_config(PrivacyRouteConfig::from_env())
}

fn router_with_config(config: PrivacyRouteConfig) -> Router {
    Router::new()
        .route("/api/privacy/requests", get(list_requests_handler))
        .route("/api/privacy/requests", post(create_request_handler))
        .route(
            "/api/privacy/requests/:request_id",
            get(request_detail_handler),
        )
        .route(
            "/api/privacy/requests/:request_id/cancel",
            post(cancel_request_handler),
        )
        .route(
            "/api/privacy/requests/:request_id/delivery",
            post(issue_export_delivery_handler),
        )
        .route(
            "/api/privacy/requests/:request_id/download",
            post(download_export_handler),
        )
        .route("/api/privacy/consents", get(list_consents_handler))
        .route(
            "/api/privacy/consents/:channel",
            put(update_consent_handler),
        )
        .route(
            "/api/privacy/communications",
            get(list_communications_handler),
        )
        .route(
            "/api/privacy/communications/:channel/:category",
            put(update_communication_handler),
        )
        .layer(Extension(config))
        .layer(middleware::from_fn(privacy_no_store))
}

async fn privacy_no_store(request: Request<Body>, next: Next<Body>) -> Response {
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        CACHE_CONTROL,
        HeaderValue::from_static("no-store, max-age=0"),
    );
    response
        .headers_mut()
        .insert(PRAGMA, HeaderValue::from_static("no-cache"));
    response
}

async fn list_requests_handler(
    Extension(config): Extension<PrivacyRouteConfig>,
    Extension(database): Extension<Option<Database>>,
    AuthUser(user): AuthUser,
    query: Result<Query<RequestListQuery>, QueryRejection>,
) -> Response {
    let Query(query) = match query {
        Ok(query) => query,
        Err(_) => {
            return api_error(
                StatusCode::BAD_REQUEST,
                "privacy_invalid_query",
                "Invalid privacy query",
            )
        }
    };
    let (database, owner) = match prerequisites(&config, database, &user) {
        Ok(value) => value,
        Err(error) => return error.response(),
    };
    let limit = query.limit.unwrap_or(50);
    match database
        .list_privacy_requests_for_owner(owner.universe_id, owner.user_id, limit)
        .await
    {
        Ok(requests) => success(
            requests
                .into_iter()
                .map(|request| request_payload(request, config.delivery_bridge.is_some()))
                .collect::<Vec<_>>(),
        ),
        Err(error) => repository_error(error),
    }
}

async fn create_request_handler(
    Extension(config): Extension<PrivacyRouteConfig>,
    Extension(database): Extension<Option<Database>>,
    AuthUser(user): AuthUser,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    payload: Result<Json<CreatePrivacyRequest>, JsonRejection>,
) -> Response {
    let Json(payload) = match payload {
        Ok(payload) => payload,
        Err(_) => return invalid_payload(),
    };
    let (database, owner) = match prerequisites(&config, database, &user) {
        Ok(value) => value,
        Err(error) => return error.response(),
    };
    let request_type = match parse_request_type(&payload.request_type) {
        Some(request_type) => request_type,
        None => return invalid_payload(),
    };
    if !valid_idempotency_key(&payload.idempotency_key)
        || !confirmation_matches(request_type, payload.confirmation.as_deref())
    {
        return invalid_payload();
    }
    let encrypted_payload = if request_type == PrivacyRequestType::Correction {
        let Some(changes) = payload.changes.as_ref() else {
            return invalid_payload();
        };
        let Some(encryptor) = config.correction_encryptor.as_ref() else {
            return api_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "privacy_correction_unavailable",
                "Correction encryption is unavailable",
            );
        };
        match encryptor.prepare_correction_payload(owner.universe_id, owner.user_id, changes) {
            Ok(payload) => Some(payload),
            Err(_) => return invalid_payload(),
        }
    } else {
        if payload.changes.is_some() {
            return invalid_payload();
        }
        None
    };
    let requester_ip_digest = canonical_request_ip(connect_info.map(|peer| peer.0), &headers)
        .map(|ip| config.digest("request-ip", &ip))
        .transpose();
    let requester_ip_digest = match requester_ip_digest {
        Ok(digest) => digest,
        Err(error) => return repository_error(error),
    };
    match database
        .create_privacy_request(PrivacyRequestCreateInput {
            universe_id: owner.universe_id,
            user_id: owner.user_id,
            request_type,
            idempotency_key: payload.idempotency_key.trim().to_string(),
            request_source: "user_self_service".to_string(),
            requester_ip_digest,
            encrypted_payload,
            erasure_cooling_off_seconds: None,
        })
        .await
    {
        Ok(request) => success(request_payload(
            PrivacyRequestSummary {
                request,
                export: None,
            },
            config.delivery_bridge.is_some(),
        )),
        Err(error) => repository_error(error),
    }
}

async fn request_detail_handler(
    Extension(config): Extension<PrivacyRouteConfig>,
    Extension(database): Extension<Option<Database>>,
    AuthUser(user): AuthUser,
    Path(request_id): Path<String>,
) -> Response {
    let request_id = match positive_i32(&request_id) {
        Some(request_id) => request_id,
        None => return invalid_payload(),
    };
    let (database, owner) = match prerequisites(&config, database, &user) {
        Ok(value) => value,
        Err(error) => return error.response(),
    };
    match database
        .privacy_request_detail_for_owner(owner.universe_id, owner.user_id, request_id)
        .await
    {
        Ok(Some(detail)) => success(detail_payload(detail, config.delivery_bridge.is_some())),
        Ok(None) => not_found(),
        Err(error) => repository_error(error),
    }
}

async fn cancel_request_handler(
    Extension(config): Extension<PrivacyRouteConfig>,
    Extension(database): Extension<Option<Database>>,
    AuthUser(user): AuthUser,
    Path(request_id): Path<String>,
    payload: Result<Json<CancelPrivacyRequest>, JsonRejection>,
) -> Response {
    let request_id = match positive_i32(&request_id) {
        Some(request_id) => request_id,
        None => return invalid_payload(),
    };
    let Json(payload) = match payload {
        Ok(payload) => payload,
        Err(_) => return invalid_payload(),
    };
    if payload.expected_version <= 0 || payload.confirmation != CANCELLATION_CONFIRMATION {
        return invalid_payload();
    }
    let (database, owner) = match prerequisites(&config, database, &user) {
        Ok(value) => value,
        Err(error) => return error.response(),
    };
    match database
        .cancel_privacy_request_if_version(
            owner.universe_id,
            owner.user_id,
            request_id,
            payload.expected_version,
            "user_cancelled",
        )
        .await
    {
        Ok(request) => success(request_payload(
            PrivacyRequestSummary {
                request,
                export: None,
            },
            config.delivery_bridge.is_some(),
        )),
        Err(error) => repository_error(error),
    }
}

async fn issue_export_delivery_handler(
    Extension(config): Extension<PrivacyRouteConfig>,
    Extension(database): Extension<Option<Database>>,
    AuthUser(user): AuthUser,
    Path(request_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let request_id = match positive_i32(&request_id) {
        Some(request_id) => request_id,
        None => return invalid_payload(),
    };
    if let Err(error) = prerequisites(&config, database, &user) {
        return error.response();
    }
    forward_delivery_request(&config, request_id, "delivery", &headers, None).await
}

async fn download_export_handler(
    Extension(config): Extension<PrivacyRouteConfig>,
    Extension(database): Extension<Option<Database>>,
    AuthUser(user): AuthUser,
    Path(request_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let request_id = match positive_i32(&request_id) {
        Some(request_id) => request_id,
        None => return invalid_payload(),
    };
    if let Err(error) = prerequisites(&config, database, &user) {
        return error.response();
    }
    let Some(delivery_token) = headers.get(DELIVERY_TOKEN_HEADER) else {
        return api_error(
            StatusCode::BAD_REQUEST,
            "privacy_delivery_token_required",
            "Privacy delivery token is required",
        );
    };
    forward_delivery_request(
        &config,
        request_id,
        "download",
        &headers,
        Some(delivery_token),
    )
    .await
}

async fn forward_delivery_request(
    config: &PrivacyRouteConfig,
    request_id: i32,
    action: &'static str,
    headers: &HeaderMap,
    delivery_token: Option<&HeaderValue>,
) -> Response {
    let Some(bridge) = config.delivery_bridge.as_ref() else {
        return api_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "privacy_delivery_unavailable",
            "Privacy export delivery is unavailable",
        );
    };
    let Some(authorization) = headers.get(AUTHORIZATION) else {
        return api_error(
            StatusCode::UNAUTHORIZED,
            "privacy_identity_unavailable",
            "Authenticated privacy identity is unavailable",
        );
    };
    let mut request = bridge.client.post(format!(
        "{}/api/privacy/exports/{request_id}/{action}",
        bridge.base_url
    ));
    request = request.header(reqwest::header::AUTHORIZATION, authorization.as_bytes());
    if let Some(token) = delivery_token {
        request = request.header(DELIVERY_TOKEN_HEADER, token.as_bytes());
    }
    let upstream = match request.send().await {
        Ok(response) => response,
        Err(_) => {
            return api_error(
                StatusCode::BAD_GATEWAY,
                "privacy_delivery_unavailable",
                "Privacy export delivery is unavailable",
            )
        }
    };
    if upstream
        .content_length()
        .is_some_and(|length| length > MAX_DELIVERY_RESPONSE_BYTES)
    {
        return api_error(
            StatusCode::BAD_GATEWAY,
            "privacy_delivery_invalid_response",
            "Privacy export delivery returned an invalid response",
        );
    }
    let status = upstream.status();
    let response_headers = [CONTENT_TYPE, CONTENT_DISPOSITION, CACHE_CONTROL, PRAGMA]
        .into_iter()
        .filter_map(|name| {
            upstream
                .headers()
                .get(name.as_str())
                .cloned()
                .map(|value| (name, value))
        })
        .collect::<Vec<_>>();
    let bytes = match upstream.bytes().await {
        Ok(bytes) if bytes.len() as u64 <= MAX_DELIVERY_RESPONSE_BYTES => bytes,
        _ => {
            return api_error(
                StatusCode::BAD_GATEWAY,
                "privacy_delivery_invalid_response",
                "Privacy export delivery returned an invalid response",
            )
        }
    };
    let mut response = (status, bytes).into_response();
    for (name, value) in response_headers {
        response.headers_mut().insert(name, value);
    }
    response
}

async fn list_consents_handler(
    Extension(config): Extension<PrivacyRouteConfig>,
    Extension(database): Extension<Option<Database>>,
    AuthUser(user): AuthUser,
) -> Response {
    let (database, owner) = match prerequisites(&config, database, &user) {
        Ok(value) => value,
        Err(error) => return error.response(),
    };
    match database
        .list_privacy_consents_for_owner(owner.universe_id, owner.user_id)
        .await
    {
        Ok(consents) => success(ConsentCollectionPayload {
            current_policy_version: config.policy_version.clone(),
            consents: consents.into_iter().map(consent_payload).collect(),
        }),
        Err(error) => repository_error(error),
    }
}

async fn update_consent_handler(
    Extension(config): Extension<PrivacyRouteConfig>,
    Extension(database): Extension<Option<Database>>,
    AuthUser(user): AuthUser,
    Path(channel): Path<String>,
    payload: Result<Json<UpdateConsentRequest>, JsonRejection>,
) -> Response {
    let Json(payload) = match payload {
        Ok(payload) => payload,
        Err(_) => return invalid_payload(),
    };
    if !valid_consent_channel(&channel)
        || payload.expected_version < 0
        || payload.policy_version != config.policy_version
    {
        return invalid_payload();
    }
    let status = match parse_consent_status(&payload.status) {
        Some(status) => status,
        None => return invalid_payload(),
    };
    if status == ConsentStatus::Granted && !payload.confirmed {
        return invalid_payload();
    }
    let (database, owner) = match prerequisites(&config, database, &user) {
        Ok(value) => value,
        Err(error) => return error.response(),
    };
    let current = match database
        .list_privacy_consents_for_owner(owner.universe_id, owner.user_id)
        .await
    {
        Ok(consents) => consents
            .into_iter()
            .find(|consent| consent.purpose == "marketing" && consent.channel == channel),
        Err(error) => return repository_error(error),
    };
    let current_version = current.as_ref().map_or(0, |consent| consent.version);
    if current_version != payload.expected_version {
        return version_conflict();
    }
    if current.as_ref().is_some_and(|consent| {
        consent.status == status
            && consent.policy_version == config.policy_version
            && consent.lawful_basis == "consent"
    }) {
        return success(consent_payload(current.expect("checked current consent")));
    }
    let proof_digest = if status == ConsentStatus::Granted {
        let proof = format!(
            "{}:{}:marketing:{}:{}",
            owner.universe_id, owner.user_id, channel, config.policy_version
        );
        match config.digest("explicit-consent", &proof) {
            Ok(digest) => Some(digest),
            Err(error) => return repository_error(error),
        }
    } else {
        None
    };
    if let Err(error) = database
        .set_privacy_consent_if_version(
            ConsentUpdate {
                universe_id: owner.universe_id,
                user_id: owner.user_id,
                purpose: "marketing".to_string(),
                channel: channel.clone(),
                status,
                lawful_basis: "consent".to_string(),
                policy_version: config.policy_version.clone(),
                proof_digest,
                expires_at_unix: None,
                changed_by_user_id: owner.user_id,
                actor_type: "user".to_string(),
            },
            payload.expected_version,
        )
        .await
    {
        return repository_error(error);
    }
    match database
        .list_privacy_consents_for_owner(owner.universe_id, owner.user_id)
        .await
    {
        Ok(consents) => consents
            .into_iter()
            .find(|consent| consent.purpose == "marketing" && consent.channel == channel)
            .map(consent_payload)
            .map(success)
            .unwrap_or_else(repository_unavailable),
        Err(error) => repository_error(error),
    }
}

async fn list_communications_handler(
    Extension(config): Extension<PrivacyRouteConfig>,
    Extension(database): Extension<Option<Database>>,
    AuthUser(user): AuthUser,
) -> Response {
    let (database, owner) = match prerequisites(&config, database, &user) {
        Ok(value) => value,
        Err(error) => return error.response(),
    };
    match database
        .communication_preferences_for_owner(owner.universe_id, owner.user_id)
        .await
    {
        Ok(preferences) => success(
            preferences
                .into_iter()
                .map(communication_payload)
                .collect::<Vec<_>>(),
        ),
        Err(error) => repository_error(error),
    }
}

async fn update_communication_handler(
    Extension(config): Extension<PrivacyRouteConfig>,
    Extension(database): Extension<Option<Database>>,
    AuthUser(user): AuthUser,
    Path((channel, category)): Path<(String, String)>,
    payload: Result<Json<UpdateCommunicationRequest>, JsonRejection>,
) -> Response {
    let Json(payload) = match payload {
        Ok(payload) => payload,
        Err(_) => return invalid_payload(),
    };
    if !PRIVACY_COMMUNICATION_CHANNELS.contains(&channel.as_str())
        || !PRIVACY_COMMUNICATION_CATEGORIES.contains(&category.as_str())
        || payload.expected_version < 0
    {
        return invalid_payload();
    }
    if privacy_communication_category_is_essential(&category) {
        return api_error(
            StatusCode::BAD_REQUEST,
            "privacy_essential_communication_read_only",
            "Essential account communications cannot be changed",
        );
    }
    let (database, owner) = match prerequisites(&config, database, &user) {
        Ok(value) => value,
        Err(error) => return error.response(),
    };
    let current = match database
        .communication_preferences_for_owner(owner.universe_id, owner.user_id)
        .await
    {
        Ok(preferences) => preferences
            .into_iter()
            .find(|preference| preference.channel == channel && preference.category == category),
        Err(error) => return repository_error(error),
    };
    let Some(current) = current else {
        return repository_unavailable();
    };
    if current.version != payload.expected_version {
        return version_conflict();
    }
    if current.enabled == payload.enabled {
        return success(communication_payload(current));
    }
    if let Err(error) = database
        .set_communication_preference_if_version(
            CommunicationPreferenceUpdate {
                universe_id: owner.universe_id,
                user_id: owner.user_id,
                channel: channel.clone(),
                category: category.clone(),
                enabled: payload.enabled,
                changed_by_user_id: owner.user_id,
                actor_type: "user".to_string(),
            },
            payload.expected_version,
        )
        .await
    {
        return repository_error(error);
    }
    match database
        .communication_preferences_for_owner(owner.universe_id, owner.user_id)
        .await
    {
        Ok(preferences) => preferences
            .into_iter()
            .find(|preference| preference.channel == channel && preference.category == category)
            .map(communication_payload)
            .map(success)
            .unwrap_or_else(repository_unavailable),
        Err(error) => repository_error(error),
    }
}

fn prerequisites(
    config: &PrivacyRouteConfig,
    database: Option<Database>,
    user: &platform_auth::AuthUser,
) -> Result<(Database, PrivacyOwner), PrivacyPrerequisiteError> {
    if !config.available {
        return Err(PrivacyPrerequisiteError::Unavailable);
    }
    let database = database.ok_or(PrivacyPrerequisiteError::Unavailable)?;
    let user_id = user
        .user_id
        .parse::<i32>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or(PrivacyPrerequisiteError::Identity)?;
    let universe_id = user
        .universe_id
        .filter(|value| *value > 0)
        .ok_or(PrivacyPrerequisiteError::Identity)?;
    Ok((
        database,
        PrivacyOwner {
            universe_id,
            user_id,
        },
    ))
}

#[derive(Clone, Copy)]
enum PrivacyPrerequisiteError {
    Unavailable,
    Identity,
}

impl PrivacyPrerequisiteError {
    fn response(self) -> Response {
        match self {
            Self::Unavailable => repository_unavailable(),
            Self::Identity => identity_unavailable(),
        }
    }
}

fn request_payload(
    summary: PrivacyRequestSummary,
    delivery_available: bool,
) -> PrivacyRequestPayload {
    let cancellation_allowed = !summary.request.legal_hold_active
        && matches!(
            summary.request.status,
            PrivacyRequestStatus::Pending
                | PrivacyRequestStatus::CoolingOff
                | PrivacyRequestStatus::InReview
                | PrivacyRequestStatus::Approved
                | PrivacyRequestStatus::Queued
                | PrivacyRequestStatus::Processing
                | PrivacyRequestStatus::Failed
        );
    PrivacyRequestPayload {
        id: summary.request.id,
        request_type: summary.request.request_type.as_str(),
        status: summary.request.status.as_str(),
        requested_at_unix: summary.request.requested_at_unix,
        cooling_off_until_unix: summary.request.cooling_off_until_unix,
        completed_at_unix: summary.request.completed_at_unix,
        cancelled_at_unix: summary.request.cancelled_at_unix,
        legal_hold_active: summary.request.legal_hold_active,
        retention_until_unix: summary.request.retention_until_unix,
        version: summary.request.version,
        cancellation_allowed,
        export: summary
            .export
            .map(|export| export_payload(export, delivery_available)),
    }
}

fn export_payload(
    export: PrivacyExportAvailability,
    delivery_available: bool,
) -> PrivacyExportPayload {
    PrivacyExportPayload {
        ready: export.ready,
        expired: export.expired,
        expires_at_unix: export.expires_at_unix,
        plaintext_size: export.plaintext_size,
        delivery_available: delivery_available && export.ready && !export.expired,
        delivery_status: if export.ready {
            if delivery_available && !export.expired {
                "ready"
            } else {
                "prepared_delivery_not_connected"
            }
        } else if export.expired {
            "expired"
        } else {
            "preparing"
        },
    }
}

fn detail_payload(
    detail: PrivacyRequestDetail,
    delivery_available: bool,
) -> PrivacyRequestDetailPayload {
    PrivacyRequestDetailPayload {
        request: request_payload(
            PrivacyRequestSummary {
                request: detail.request,
                export: detail.export,
            },
            delivery_available,
        ),
        timeline: detail.timeline.into_iter().map(event_payload).collect(),
    }
}

fn event_payload(event: PrivacyRequestEventRow) -> PrivacyRequestEventPayload {
    PrivacyRequestEventPayload {
        id: event.id,
        event_type: event.event_type,
        from_status: event.from_status.map(PrivacyRequestStatus::as_str),
        to_status: event.to_status.as_str(),
        actor_type: event.actor_type,
        reason_code: event.reason_code,
        request_version: event.request_version,
        created_at_unix: event.created_at_unix,
    }
}

fn consent_payload(consent: PrivacyConsentRow) -> ConsentPayload {
    ConsentPayload {
        purpose: consent.purpose,
        channel: consent.channel,
        status: consent.status.as_str(),
        lawful_basis: consent.lawful_basis,
        policy_version: consent.policy_version,
        collected_at_unix: consent.collected_at_unix,
        expires_at_unix: consent.expires_at_unix,
        version: consent.version,
    }
}

fn communication_payload(preference: CommunicationPreferenceRow) -> CommunicationPayload {
    CommunicationPayload {
        channel: preference.channel,
        category: preference.category,
        enabled: preference.enabled,
        explicitly_configured: preference.explicitly_configured,
        effective_allowed: preference.effective_allowed,
        essential: preference.essential,
        marketing_consent_current: preference.marketing_consent_current,
        suppressed_by_restriction: preference.suppressed_by_restriction,
        updated_at_unix: preference.updated_at_unix,
        version: preference.version,
    }
}

fn parse_request_type(value: &str) -> Option<PrivacyRequestType> {
    match value.trim() {
        "export" => Some(PrivacyRequestType::Export),
        "correction" => Some(PrivacyRequestType::Correction),
        "restriction" => Some(PrivacyRequestType::Restriction),
        "erasure" => Some(PrivacyRequestType::Erasure),
        _ => None,
    }
}

fn parse_consent_status(value: &str) -> Option<ConsentStatus> {
    match value.trim() {
        "granted" => Some(ConsentStatus::Granted),
        "denied" => Some(ConsentStatus::Denied),
        "withdrawn" => Some(ConsentStatus::Withdrawn),
        _ => None,
    }
}

fn confirmation_matches(request_type: PrivacyRequestType, confirmation: Option<&str>) -> bool {
    match request_type {
        PrivacyRequestType::Restriction => confirmation == Some(RESTRICTION_CONFIRMATION),
        PrivacyRequestType::Erasure => confirmation == Some(ERASURE_CONFIRMATION),
        PrivacyRequestType::Correction => confirmation == Some(CORRECTION_CONFIRMATION),
        PrivacyRequestType::Export => confirmation.is_none(),
    }
}

fn valid_internal_url(value: &str) -> bool {
    reqwest::Url::parse(value).is_ok_and(|url| {
        matches!(url.scheme(), "http" | "https")
            && url.host_str().is_some()
            && url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none()
            && url.path() == "/"
    })
}

fn valid_idempotency_key(value: &str) -> bool {
    let value = value.trim();
    (8..=128).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
}

fn valid_short_identifier(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty()
        && value.len() <= 100
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
}

fn valid_consent_channel(channel: &str) -> bool {
    channel == "all" || PRIVACY_COMMUNICATION_CHANNELS.contains(&channel)
}

fn positive_i32(value: &str) -> Option<i32> {
    value.parse::<i32>().ok().filter(|value| *value > 0)
}

fn canonical_request_ip(peer: Option<SocketAddr>, _headers: &HeaderMap) -> Option<String> {
    // Forwarded headers are intentionally ignored until deployment supplies
    // an explicit trusted-proxy boundary. Direct clients can spoof them.
    peer.map(|address| address.ip().to_string())
}

fn repository_error(error: PrivacyError) -> Response {
    match error {
        PrivacyError::InvalidInput(_) => invalid_payload(),
        PrivacyError::NotFound | PrivacyError::Forbidden | PrivacyError::DeliveryDenied => {
            not_found()
        }
        PrivacyError::Conflict(
            "privacy request version changed"
            | "consent version changed"
            | "communication preference version changed",
        ) => version_conflict(),
        PrivacyError::Conflict("an active request of this type already exists") => api_error(
            StatusCode::CONFLICT,
            "privacy_request_active",
            "An active privacy request of this type already exists",
        ),
        PrivacyError::Conflict(_) => api_error(
            StatusCode::CONFLICT,
            "privacy_conflict",
            "Privacy request conflicts with current state",
        ),
        PrivacyError::CoolingOff => api_error(
            StatusCode::CONFLICT,
            "privacy_cooling_off",
            "Privacy request is still cooling off",
        ),
        PrivacyError::LegalHold => api_error(
            StatusCode::CONFLICT,
            "privacy_legal_hold",
            "Privacy request is under legal hold",
        ),
        PrivacyError::LeaseLost => api_error(
            StatusCode::CONFLICT,
            "privacy_state_changed",
            "Privacy state changed; refresh and retry",
        ),
        PrivacyError::Database(_) => repository_unavailable(),
    }
}

fn invalid_payload() -> Response {
    api_error(
        StatusCode::BAD_REQUEST,
        "privacy_invalid_request",
        "Invalid privacy request",
    )
}

fn identity_unavailable() -> Response {
    api_error(
        StatusCode::UNAUTHORIZED,
        "privacy_identity_unavailable",
        "Authenticated privacy identity is unavailable",
    )
}

fn not_found() -> Response {
    api_error(
        StatusCode::NOT_FOUND,
        "privacy_not_found",
        "Privacy record not found",
    )
}

fn version_conflict() -> Response {
    api_error(
        StatusCode::CONFLICT,
        "privacy_version_conflict",
        "Privacy settings changed; refresh and retry",
    )
}

fn repository_unavailable() -> Response {
    api_error(
        StatusCode::SERVICE_UNAVAILABLE,
        "privacy_repository_unavailable",
        "Privacy service is unavailable",
    )
}

fn api_error(status: StatusCode, code: &'static str, error: &'static str) -> Response {
    (
        status,
        Json(PrivacyApiError {
            success: false,
            code,
            error,
        }),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn production_config_requires_a_strong_explicit_pepper() {
        let production = HashMap::from([("UNIVERSUS_ENV", "production")]);
        let config =
            PrivacyRouteConfig::from_lookup(&|name| production.get(name).map(ToString::to_string));
        assert!(!config.available);

        let configured = HashMap::from([
            ("UNIVERSUS_ENV", "production"),
            (
                "PRIVACY_REQUEST_IP_PEPPER",
                "a-production-only-pepper-with-at-least-32-bytes",
            ),
            ("PRIVACY_POLICY_VERSION", "privacy-v2"),
        ]);
        let config =
            PrivacyRouteConfig::from_lookup(&|name| configured.get(name).map(ToString::to_string));
        assert!(config.available);
        assert_eq!(config.policy_version, "privacy-v2");
    }

    #[test]
    fn development_uses_an_explicit_test_fixture_without_weakening_production() {
        let config = PrivacyRouteConfig::from_lookup(&|_| None);
        assert!(config.available);
        assert_eq!(
            config.digest("request-ip", "192.0.2.10").unwrap(),
            config.digest("request-ip", "192.0.2.10").unwrap()
        );
        assert_ne!(
            config.digest("request-ip", "192.0.2.10").unwrap(),
            config.digest("request-ip", "192.0.2.11").unwrap()
        );
    }

    #[test]
    fn request_contract_rejects_actor_fields_and_requires_high_friction_phrases() {
        assert!(
            serde_json::from_value::<CreatePrivacyRequest>(serde_json::json!({
                "requestType": "export",
                "idempotencyKey": "export-12345678",
                "universeId": 9
            }))
            .is_err()
        );
        assert!(!confirmation_matches(
            PrivacyRequestType::Restriction,
            Some("yes")
        ));
        assert!(confirmation_matches(
            PrivacyRequestType::Restriction,
            Some(RESTRICTION_CONFIRMATION)
        ));
        assert!(confirmation_matches(
            PrivacyRequestType::Erasure,
            Some(ERASURE_CONFIRMATION)
        ));
    }

    #[test]
    fn peer_ip_is_canonical_and_spoofed_forwarded_headers_are_ignored() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "198.51.100.99".parse().unwrap());
        assert_eq!(
            canonical_request_ip(
                Some("[2001:0db8::1]:443".parse::<SocketAddr>().unwrap()),
                &headers
            ),
            Some("2001:db8::1".to_string())
        );
        assert_eq!(canonical_request_ip(None, &headers), None);
    }

    #[test]
    fn repository_errors_have_stable_non_leaking_statuses() {
        assert_eq!(
            repository_error(PrivacyError::NotFound).status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            repository_error(PrivacyError::Forbidden).status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            repository_error(PrivacyError::Conflict("detail")).status(),
            StatusCode::CONFLICT
        );
        assert_eq!(
            repository_error(PrivacyError::Database("raw database detail".to_string())).status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
    }

    #[test]
    fn essential_communication_categories_are_read_only() {
        assert!(privacy_communication_category_is_essential("security"));
        assert!(privacy_communication_category_is_essential("transactional"));
        assert!(!privacy_communication_category_is_essential(
            "product_updates"
        ));
    }
}
