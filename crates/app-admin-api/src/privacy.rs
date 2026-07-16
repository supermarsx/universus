use std::str::FromStr;

use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderValue, Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Extension, Json, Router,
};
use platform_db::{
    Database, PrivacyAdminDecision, PrivacyAdminDecisionInput, PrivacyAdminRequestDetail,
    PrivacyAdminRequestFilter, PrivacyError, PrivacyExecutionEventRow, PrivacyRequestEventRow,
    PrivacyRequestRow, PrivacyRequestStatus, PrivacyRequestType, PrivacyRetentionAudit,
};
use serde::{Deserialize, Serialize};

#[derive(Clone)]
struct PrivacyAdminState {
    database: Database,
}

#[derive(Clone)]
struct PrivacyAdminPrincipal {
    user_id: i32,
    universe_id: i64,
}

pub fn router(database: Database) -> Router {
    let state = PrivacyAdminState { database };
    Router::new()
        .route("/api/admin/privacy/requests", get(list_requests))
        .route(
            "/api/admin/privacy/requests/:request_id",
            get(request_detail),
        )
        .route(
            "/api/admin/privacy/requests/:request_id/decisions",
            post(record_decision),
        )
        .route("/api/admin/privacy/retention/run", post(run_retention))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_live_tenant_admin,
        ))
        .with_state(state)
}

async fn require_live_tenant_admin(
    State(state): State<PrivacyAdminState>,
    mut request: Request<Body>,
    next: Next<Body>,
) -> Response {
    let Some(authorization) = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
    else {
        return error(StatusCode::UNAUTHORIZED, "privacy_admin_unauthorized");
    };
    let Some(token) = platform_auth::extract_bearer_token(authorization) else {
        return error(StatusCode::UNAUTHORIZED, "privacy_admin_unauthorized");
    };
    let claims = match platform_auth::validate_token(&platform_auth::AuthConfig::from_env(), token)
    {
        Ok(claims) => claims,
        Err(_) => return error(StatusCode::UNAUTHORIZED, "privacy_admin_unauthorized"),
    };
    let role = platform_auth::UserRole::from_str(&claims.role);
    if !claims.is_access_token()
        || !matches!(
            role,
            Ok(platform_auth::UserRole::Admin | platform_auth::UserRole::SuperAdmin)
        )
    {
        return error(StatusCode::FORBIDDEN, "privacy_admin_forbidden");
    }
    let Some(session_id) = claims.sid.as_deref() else {
        return error(StatusCode::UNAUTHORIZED, "privacy_admin_session_required");
    };
    let Some(universe_id) = claims.universe_id.filter(|value| *value > 0) else {
        return error(StatusCode::UNAUTHORIZED, "privacy_admin_tenant_required");
    };
    let Ok(user_id) = claims.sub.parse::<i32>() else {
        return error(StatusCode::UNAUTHORIZED, "privacy_admin_unauthorized");
    };
    match state
        .database
        .validate_privacy_admin_session(&claims.sub, session_id, claims.auth_epoch, universe_id)
        .await
    {
        Ok(true) => {}
        Ok(false) => return error(StatusCode::UNAUTHORIZED, "privacy_admin_session_invalid"),
        Err(_) => {
            return error(
                StatusCode::SERVICE_UNAVAILABLE,
                "privacy_admin_authentication_unavailable",
            )
        }
    }
    request.extensions_mut().insert(PrivacyAdminPrincipal {
        user_id,
        universe_id,
    });
    next.run(request).await
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ListQuery {
    request_type: Option<String>,
    status: Option<String>,
    user_id: Option<i32>,
    before_request_id: Option<i32>,
    limit: Option<i64>,
}

async fn list_requests(
    State(state): State<PrivacyAdminState>,
    Extension(admin): Extension<PrivacyAdminPrincipal>,
    Query(query): Query<ListQuery>,
) -> Response {
    let request_type = match query.request_type.as_deref().map(parse_request_type) {
        Some(None) => return error(StatusCode::BAD_REQUEST, "privacy_filter_invalid"),
        Some(Some(value)) => Some(value),
        None => None,
    };
    let status = match query.status.as_deref().map(parse_request_status) {
        Some(None) => return error(StatusCode::BAD_REQUEST, "privacy_filter_invalid"),
        Some(Some(value)) => Some(value),
        None => None,
    };
    match state
        .database
        .list_privacy_requests_for_admin(PrivacyAdminRequestFilter {
            universe_id: admin.universe_id,
            request_type,
            status,
            user_id: query.user_id,
            before_request_id: query.before_request_id,
            limit: query.limit.unwrap_or(50),
        })
        .await
    {
        Ok(requests) => success(&serde_json::json!({
            "requests": requests.iter().map(request_payload).collect::<Vec<_>>()
        })),
        Err(error_value) => repository_error(error_value),
    }
}

async fn request_detail(
    State(state): State<PrivacyAdminState>,
    Extension(admin): Extension<PrivacyAdminPrincipal>,
    Path(request_id): Path<i32>,
) -> Response {
    match state
        .database
        .privacy_request_detail_for_admin(admin.universe_id, request_id)
        .await
    {
        Ok(detail) => success(&detail_payload(detail)),
        Err(error_value) => repository_error(error_value),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DecisionBody {
    decision: String,
    reason_code: String,
    expected_version: i64,
}

async fn record_decision(
    State(state): State<PrivacyAdminState>,
    Extension(admin): Extension<PrivacyAdminPrincipal>,
    Path(request_id): Path<i32>,
    Json(body): Json<DecisionBody>,
) -> Response {
    let Some(decision) = parse_admin_decision(&body.decision) else {
        return error(StatusCode::BAD_REQUEST, "privacy_decision_invalid");
    };
    match state
        .database
        .record_privacy_admin_decision_if_version(
            PrivacyAdminDecisionInput {
                universe_id: admin.universe_id,
                request_id,
                admin_user_id: admin.user_id,
                decision,
                reason_code: body.reason_code,
            },
            body.expected_version,
        )
        .await
    {
        Ok(request) => success(&request_payload(&request)),
        Err(error_value) => repository_error(error_value),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RetentionBody {
    outbox_retention_days: i32,
}

async fn run_retention(
    State(state): State<PrivacyAdminState>,
    Extension(admin): Extension<PrivacyAdminPrincipal>,
    Json(body): Json<RetentionBody>,
) -> Response {
    let result = match state
        .database
        .run_privacy_retention(
            body.outbox_retention_days,
            PrivacyRetentionAudit {
                universe_id: Some(admin.universe_id),
                admin_user_id: Some(admin.user_id),
                communication_evidence_redacted: 0,
                communication_events_deleted: 0,
            },
        )
        .await
    {
        Ok((result, _)) => result,
        Err(error_value) => return repository_error(error_value),
    };
    success(&serde_json::json!({
        "artifactsPurged": result.artifacts_purged,
        "requestPayloadsRedacted": result.request_payloads_redacted,
        "outboxRowsDeleted": result.outbox_rows_deleted
    }))
}

fn request_payload(request: &PrivacyRequestRow) -> serde_json::Value {
    serde_json::json!({
        "id": request.id,
        "userId": request.user_id,
        "requestType": request.request_type.as_str(),
        "status": request.status.as_str(),
        "requestedAtUnix": request.requested_at_unix,
        "coolingOffUntilUnix": request.cooling_off_until_unix,
        "completedAtUnix": request.completed_at_unix,
        "cancelledAtUnix": request.cancelled_at_unix,
        "legalHoldActive": request.legal_hold_active,
        "retentionUntilUnix": request.retention_until_unix,
        "version": request.version
    })
}

fn detail_payload(detail: PrivacyAdminRequestDetail) -> serde_json::Value {
    serde_json::json!({
        "request": request_payload(&detail.request),
        "timeline": detail.timeline.iter().map(timeline_payload).collect::<Vec<_>>(),
        "decisions": detail.decisions.iter().map(|decision| serde_json::json!({
            "id": decision.id,
            "adminUserId": decision.admin_user_id,
            "decision": decision.decision,
            "reasonCode": decision.reason_code,
            "decidedAtUnix": decision.decided_at_unix
        })).collect::<Vec<_>>(),
        "executions": detail.executions.iter().map(execution_payload).collect::<Vec<_>>()
    })
}

fn timeline_payload(event: &PrivacyRequestEventRow) -> serde_json::Value {
    serde_json::json!({
        "id": event.id,
        "eventType": event.event_type,
        "fromStatus": event.from_status.map(PrivacyRequestStatus::as_str),
        "toStatus": event.to_status.as_str(),
        "actorType": event.actor_type,
        "reasonCode": event.reason_code,
        "requestVersion": event.request_version,
        "createdAtUnix": event.created_at_unix
    })
}

fn execution_payload(event: &PrivacyExecutionEventRow) -> serde_json::Value {
    serde_json::json!({
        "id": event.id,
        "action": event.action,
        "actorType": event.actor_type,
        "actorUserId": event.actor_user_id,
        "reasonCode": event.reason_code,
        "fieldNames": event.field_names,
        "createdAtUnix": event.created_at_unix
    })
}

fn parse_request_type(value: &str) -> Option<PrivacyRequestType> {
    match value {
        "export" => Some(PrivacyRequestType::Export),
        "correction" => Some(PrivacyRequestType::Correction),
        "restriction" => Some(PrivacyRequestType::Restriction),
        "erasure" => Some(PrivacyRequestType::Erasure),
        _ => None,
    }
}

fn parse_request_status(value: &str) -> Option<PrivacyRequestStatus> {
    match value {
        "pending" => Some(PrivacyRequestStatus::Pending),
        "cooling_off" => Some(PrivacyRequestStatus::CoolingOff),
        "in_review" => Some(PrivacyRequestStatus::InReview),
        "approved" => Some(PrivacyRequestStatus::Approved),
        "queued" => Some(PrivacyRequestStatus::Queued),
        "processing" => Some(PrivacyRequestStatus::Processing),
        "completed" => Some(PrivacyRequestStatus::Completed),
        "cancelled" => Some(PrivacyRequestStatus::Cancelled),
        "rejected" => Some(PrivacyRequestStatus::Rejected),
        "failed" => Some(PrivacyRequestStatus::Failed),
        "blocked_legal_hold" => Some(PrivacyRequestStatus::BlockedLegalHold),
        _ => None,
    }
}

fn parse_admin_decision(value: &str) -> Option<PrivacyAdminDecision> {
    match value {
        "approve" => Some(PrivacyAdminDecision::Approve),
        "reject" => Some(PrivacyAdminDecision::Reject),
        "apply_legal_hold" => Some(PrivacyAdminDecision::ApplyLegalHold),
        "release_legal_hold" => Some(PrivacyAdminDecision::ReleaseLegalHold),
        _ => None,
    }
}

fn repository_error(error_value: PrivacyError) -> Response {
    match error_value {
        PrivacyError::InvalidInput(_) => error(StatusCode::BAD_REQUEST, "privacy_input_invalid"),
        PrivacyError::NotFound => error(StatusCode::NOT_FOUND, "privacy_request_not_found"),
        PrivacyError::Forbidden => error(StatusCode::FORBIDDEN, "privacy_admin_forbidden"),
        PrivacyError::Conflict(_) => error(StatusCode::CONFLICT, "privacy_request_conflict"),
        PrivacyError::CoolingOff => error(StatusCode::CONFLICT, "privacy_cooling_off_active"),
        PrivacyError::LegalHold => error(StatusCode::CONFLICT, "privacy_legal_hold_active"),
        PrivacyError::LeaseLost | PrivacyError::DeliveryDenied => {
            error(StatusCode::CONFLICT, "privacy_request_conflict")
        }
        PrivacyError::Database(_) => error(
            StatusCode::SERVICE_UNAVAILABLE,
            "privacy_repository_unavailable",
        ),
    }
}

#[derive(Serialize)]
struct Envelope<T> {
    success: bool,
    data: T,
}

fn success<T: Serialize>(payload: &T) -> Response {
    json_response(
        StatusCode::OK,
        &Envelope {
            success: true,
            data: payload,
        },
    )
}

fn error(status: StatusCode, code: &'static str) -> Response {
    json_response(status, &serde_json::json!({"success": false, "code": code}))
}

fn json_response<T: Serialize>(status: StatusCode, payload: &T) -> Response {
    let body = serde_json::to_vec(payload).unwrap_or_else(|_| b"{\"success\":false}".to_vec());
    let mut response = (status, body).into_response();
    let headers: &mut HeaderMap = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, max-age=0"),
    );
    headers.insert(header::PRAGMA, HeaderValue::from_static("no-cache"));
    response
}
