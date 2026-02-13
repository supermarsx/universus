use std::net::SocketAddr;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::{Path, State};
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

const SERVICE_NAME: &str = "app-admin-api";
const DEFAULT_PORT: u16 = 3001;

#[derive(Clone)]
struct AppState {
    settings: Arc<Mutex<SettingsPayload>>,
    incidents: Arc<Mutex<Vec<IncidentPayload>>>,
    admin_ops: Arc<Mutex<AdminOpsState>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            settings: Arc::new(Mutex::new(SettingsPayload::default())),
            incidents: Arc::new(Mutex::new(vec![IncidentPayload {
                id: "inc-1".to_string(),
                title: "Background job lag".to_string(),
                severity: "minor".to_string(),
                state: "open".to_string(),
                created_at: "2026-02-13T00:00:00Z".to_string(),
            }])),
            admin_ops: Arc::new(Mutex::new(AdminOpsState::default())),
        }
    }
}

#[derive(Serialize)]
struct Envelope<T> {
    status: &'static str,
    data: T,
}

#[derive(Serialize)]
struct ServiceStatus {
    status: &'static str,
    service: &'static str,
}

#[derive(Serialize)]
struct DashboardPayload {
    active_users: u64,
    alerts_open: u64,
    requests_per_minute: u64,
    maintenance_active: bool,
    blocked_users: usize,
    adjusted_users: usize,
}

#[derive(Serialize)]
struct UsersPayload {
    total: u64,
    admins: u64,
    suspended: u64,
}

#[derive(Serialize)]
struct MonitoringPayload {
    uptime_percent: f32,
    p95_latency_ms: u64,
    error_rate_percent: f32,
}

#[derive(Serialize, Deserialize, Clone)]
struct SettingsPayload {
    maintenance_mode: bool,
    notification_channel: String,
    retention_days: u16,
}

impl Default for SettingsPayload {
    fn default() -> Self {
        Self {
            maintenance_mode: false,
            notification_channel: "slack://ops-alerts".to_string(),
            retention_days: 90,
        }
    }
}

#[derive(Serialize, Deserialize)]
struct SettingsUpdateRequest {
    maintenance_mode: bool,
    notification_channel: String,
    retention_days: u16,
}

#[derive(Serialize)]
struct AnalyticsPayload {
    dau: u64,
    wau: u64,
    conversion_rate_percent: f32,
}

#[derive(Serialize)]
struct AuditRecord {
    id: String,
    actor: String,
    action: String,
    at: String,
}

#[derive(Serialize)]
struct AuditPayload {
    records: Vec<AuditRecord>,
}

#[derive(Serialize, Deserialize, Clone)]
struct IncidentPayload {
    id: String,
    title: String,
    severity: String,
    state: String,
    created_at: String,
}

#[derive(Serialize)]
struct AdminStatusPayload {
    service_state: &'static str,
    incidents_open: usize,
    incidents: Vec<IncidentPayload>,
    admin_state: AdminOpsSnapshot,
}

#[derive(Serialize)]
struct EventsPayload {
    events: Vec<EventRecord>,
}

#[derive(Serialize)]
struct EventRecord {
    id: String,
    category: String,
    summary: String,
    at: String,
}

#[derive(Deserialize)]
struct CreateIncidentRequest {
    title: String,
    severity: String,
}

#[derive(Deserialize)]
struct UpdateIncidentRequest {
    state: String,
}

#[derive(Clone)]
struct AdminOpsState {
    blocked_users: BTreeSet<String>,
    maintenance: MaintenancePayload,
    resource_adjustments: BTreeMap<String, BTreeMap<String, i64>>,
}

impl Default for AdminOpsState {
    fn default() -> Self {
        Self {
            blocked_users: BTreeSet::new(),
            maintenance: MaintenancePayload::default(),
            resource_adjustments: BTreeMap::new(),
        }
    }
}

#[derive(Serialize, Clone, Default)]
struct MaintenancePayload {
    active: bool,
    started_at_epoch_seconds: Option<u64>,
    stopped_at_epoch_seconds: Option<u64>,
}

#[derive(Serialize)]
struct UserBlockPayload {
    user_id: String,
    blocked: bool,
}

#[derive(Deserialize)]
struct ResourceAdjustmentRequest {
    resource: String,
    delta: i64,
}

#[derive(Serialize)]
struct ResourceAdjustmentPayload {
    user_id: String,
    resource: String,
    delta: i64,
    total: i64,
}

#[derive(Serialize)]
struct AdminOpsSnapshot {
    blocked_users: Vec<String>,
    maintenance: MaintenancePayload,
    resource_adjustments: BTreeMap<String, BTreeMap<String, i64>>,
}

fn app_router() -> Router {
    let state = AppState::default();

    let admin_router = Router::new()
        .route("/dashboard", get(admin_dashboard))
        .route("/users", get(admin_users))
        .route("/monitoring", get(admin_monitoring))
        .route("/settings", get(get_settings).post(post_settings).patch(patch_settings))
        .route("/analytics", get(admin_analytics))
        .route("/audit", get(admin_audit))
        .route("/status", get(admin_status))
        .route("/users/:id/block", post(block_user))
        .route("/users/:id/unblock", post(unblock_user))
        .route("/users/:id/resources", post(adjust_user_resources))
        .route("/maintenance/start", post(start_maintenance))
        .route("/maintenance/stop", post(stop_maintenance))
        .route("/maintenance", get(get_maintenance))
        .route("/events", get(admin_events))
        .route("/status/incidents", post(create_incident))
        .route("/status/incidents/:id", patch(update_incident));

    Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
        .route("/status", get(status))
        .nest("/api/admin", admin_router)
        .with_state(state)
}

async fn health() -> Json<ServiceStatus> {
    Json(ServiceStatus {
        status: "ok",
        service: SERVICE_NAME,
    })
}

async fn ready() -> Json<ServiceStatus> {
    Json(ServiceStatus {
        status: "ok",
        service: SERVICE_NAME,
    })
}

async fn status() -> Json<ServiceStatus> {
    Json(ServiceStatus {
        status: "ok",
        service: SERVICE_NAME,
    })
}

async fn admin_dashboard(State(state): State<AppState>) -> Json<Envelope<DashboardPayload>> {
    let admin_ops = snapshot_admin_ops(&state);

    Json(Envelope {
        status: "ok",
        data: DashboardPayload {
            active_users: 1284,
            alerts_open: 3,
            requests_per_minute: 2370,
            maintenance_active: admin_ops.maintenance.active,
            blocked_users: admin_ops.blocked_users.len(),
            adjusted_users: admin_ops.resource_adjustments.len(),
        },
    })
}

async fn admin_users() -> Json<Envelope<UsersPayload>> {
    Json(Envelope {
        status: "ok",
        data: UsersPayload {
            total: 8320,
            admins: 12,
            suspended: 5,
        },
    })
}

async fn admin_monitoring() -> Json<Envelope<MonitoringPayload>> {
    Json(Envelope {
        status: "ok",
        data: MonitoringPayload {
            uptime_percent: 99.97,
            p95_latency_ms: 180,
            error_rate_percent: 0.02,
        },
    })
}

async fn get_settings(State(state): State<AppState>) -> Json<Envelope<SettingsPayload>> {
    let settings = state.settings.lock().expect("settings lock poisoned").clone();
    Json(Envelope {
        status: "ok",
        data: settings,
    })
}

async fn post_settings(
    State(state): State<AppState>,
    Json(payload): Json<SettingsUpdateRequest>,
) -> Json<Envelope<SettingsPayload>> {
    let updated = SettingsPayload {
        maintenance_mode: payload.maintenance_mode,
        notification_channel: payload.notification_channel,
        retention_days: payload.retention_days,
    };

    *state.settings.lock().expect("settings lock poisoned") = updated.clone();

    Json(Envelope {
        status: "ok",
        data: updated,
    })
}

async fn patch_settings(
    State(state): State<AppState>,
    Json(payload): Json<SettingsUpdateRequest>,
) -> Json<Envelope<SettingsPayload>> {
    let updated = SettingsPayload {
        maintenance_mode: payload.maintenance_mode,
        notification_channel: payload.notification_channel,
        retention_days: payload.retention_days,
    };

    *state.settings.lock().expect("settings lock poisoned") = updated.clone();

    Json(Envelope {
        status: "ok",
        data: updated,
    })
}

async fn admin_analytics() -> Json<Envelope<AnalyticsPayload>> {
    Json(Envelope {
        status: "ok",
        data: AnalyticsPayload {
            dau: 2510,
            wau: 11320,
            conversion_rate_percent: 3.6,
        },
    })
}

async fn admin_audit() -> Json<Envelope<AuditPayload>> {
    Json(Envelope {
        status: "ok",
        data: AuditPayload {
            records: vec![
                AuditRecord {
                    id: "aud-1001".to_string(),
                    actor: "admin@universus".to_string(),
                    action: "updated.settings".to_string(),
                    at: "2026-02-13T02:12:01Z".to_string(),
                },
                AuditRecord {
                    id: "aud-1002".to_string(),
                    actor: "system".to_string(),
                    action: "incident.created".to_string(),
                    at: "2026-02-13T03:30:22Z".to_string(),
                },
            ],
        },
    })
}

async fn admin_status(State(state): State<AppState>) -> Json<Envelope<AdminStatusPayload>> {
    let incidents = state
        .incidents
        .lock()
        .expect("incidents lock poisoned")
        .clone();
    let admin_state = snapshot_admin_ops(&state);

    let incidents_open = incidents.iter().filter(|incident| incident.state == "open").count();

    Json(Envelope {
        status: "ok",
        data: AdminStatusPayload {
            service_state: "operational",
            incidents_open,
            incidents,
            admin_state,
        },
    })
}

async fn admin_events() -> Json<Envelope<EventsPayload>> {
    Json(Envelope {
        status: "ok",
        data: EventsPayload {
            events: vec![
                EventRecord {
                    id: "evt-9001".to_string(),
                    category: "deployment".to_string(),
                    summary: "admin-api deployed".to_string(),
                    at: "2026-02-13T01:00:00Z".to_string(),
                },
                EventRecord {
                    id: "evt-9002".to_string(),
                    category: "security".to_string(),
                    summary: "token rotation completed".to_string(),
                    at: "2026-02-13T04:10:00Z".to_string(),
                },
            ],
        },
    })
}

async fn create_incident(
    State(state): State<AppState>,
    Json(payload): Json<CreateIncidentRequest>,
) -> Json<Envelope<IncidentPayload>> {
    let mut incidents = state.incidents.lock().expect("incidents lock poisoned");
    let id = format!("inc-{}", incidents.len() + 1);
    let incident = IncidentPayload {
        id,
        title: payload.title,
        severity: payload.severity,
        state: "open".to_string(),
        created_at: "2026-02-13T05:00:00Z".to_string(),
    };
    incidents.push(incident.clone());

    Json(Envelope {
        status: "ok",
        data: incident,
    })
}

async fn update_incident(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateIncidentRequest>,
) -> Json<Envelope<IncidentPayload>> {
    let mut incidents = state.incidents.lock().expect("incidents lock poisoned");

    if let Some(incident) = incidents.iter_mut().find(|incident| incident.id == id) {
        incident.state = payload.state;
        return Json(Envelope {
            status: "ok",
            data: incident.clone(),
        });
    }

    let incident = IncidentPayload {
        id,
        title: "synthetic".to_string(),
        severity: "unknown".to_string(),
        state: payload.state,
        created_at: "2026-02-13T05:00:00Z".to_string(),
    };
    incidents.push(incident.clone());

    Json(Envelope {
        status: "ok",
        data: incident,
    })
}

async fn block_user(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<Envelope<UserBlockPayload>> {
    let mut admin_ops = state.admin_ops.lock().expect("admin ops lock poisoned");
    admin_ops.blocked_users.insert(id.clone());

    Json(Envelope {
        status: "ok",
        data: UserBlockPayload {
            user_id: id,
            blocked: true,
        },
    })
}

async fn unblock_user(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<Envelope<UserBlockPayload>> {
    let mut admin_ops = state.admin_ops.lock().expect("admin ops lock poisoned");
    admin_ops.blocked_users.remove(&id);

    Json(Envelope {
        status: "ok",
        data: UserBlockPayload {
            user_id: id,
            blocked: false,
        },
    })
}

async fn adjust_user_resources(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(payload): Json<ResourceAdjustmentRequest>,
) -> Json<Envelope<ResourceAdjustmentPayload>> {
    let mut admin_ops = state.admin_ops.lock().expect("admin ops lock poisoned");

    let user_resources = admin_ops
        .resource_adjustments
        .entry(id.clone())
        .or_default();
    let total = user_resources.entry(payload.resource.clone()).or_default();
    *total += payload.delta;

    Json(Envelope {
        status: "ok",
        data: ResourceAdjustmentPayload {
            user_id: id,
            resource: payload.resource,
            delta: payload.delta,
            total: *total,
        },
    })
}

async fn start_maintenance(State(state): State<AppState>) -> Json<Envelope<MaintenancePayload>> {
    let mut admin_ops = state.admin_ops.lock().expect("admin ops lock poisoned");
    admin_ops.maintenance.active = true;
    admin_ops.maintenance.started_at_epoch_seconds = Some(now_epoch_seconds());
    admin_ops.maintenance.stopped_at_epoch_seconds = None;

    Json(Envelope {
        status: "ok",
        data: admin_ops.maintenance.clone(),
    })
}

async fn stop_maintenance(State(state): State<AppState>) -> Json<Envelope<MaintenancePayload>> {
    let mut admin_ops = state.admin_ops.lock().expect("admin ops lock poisoned");
    admin_ops.maintenance.active = false;
    admin_ops.maintenance.stopped_at_epoch_seconds = Some(now_epoch_seconds());

    Json(Envelope {
        status: "ok",
        data: admin_ops.maintenance.clone(),
    })
}

async fn get_maintenance(State(state): State<AppState>) -> Json<Envelope<MaintenancePayload>> {
    let maintenance = state
        .admin_ops
        .lock()
        .expect("admin ops lock poisoned")
        .maintenance
        .clone();

    Json(Envelope {
        status: "ok",
        data: maintenance,
    })
}

fn snapshot_admin_ops(state: &AppState) -> AdminOpsSnapshot {
    let admin_ops = state.admin_ops.lock().expect("admin ops lock poisoned");
    AdminOpsSnapshot {
        blocked_users: admin_ops.blocked_users.iter().cloned().collect(),
        maintenance: admin_ops.maintenance.clone(),
        resource_adjustments: admin_ops.resource_adjustments.clone(),
    }
}

fn now_epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before epoch")
        .as_secs()
}

fn listen_port(default_port: u16) -> u16 {
    std::env::var("PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(default_port)
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let app = app_router();

    let addr = SocketAddr::from(([0, 0, 0, 0], listen_port(DEFAULT_PORT)));
    tracing::info!(service = SERVICE_NAME, %addr, "startup");

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .expect("server failed");
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode};
    use hyper::body::to_bytes;
    use serde_json::{json, Value};
    use tower::ServiceExt;

    use super::app_router;

    async fn json_body(response: axum::response::Response) -> Value {
        let bytes = to_bytes(response.into_body()).await.expect("read body");
        serde_json::from_slice(&bytes).expect("parse json")
    }

    #[tokio::test]
    async fn health_endpoint_returns_service_status() {
        let app = app_router();

        let response = app
            .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = json_body(response).await;
        assert_eq!(body["status"], "ok");
        assert_eq!(body["service"], "app-admin-api");
    }

    #[tokio::test]
    async fn admin_dashboard_has_envelope_contract() {
        let app = app_router();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/admin/dashboard")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = json_body(response).await;
        assert_eq!(body["status"], "ok");
        assert!(body["data"]["active_users"].is_number());
        assert!(body["data"]["alerts_open"].is_number());
        assert!(body["data"]["requests_per_minute"].is_number());
    }

    #[tokio::test]
    async fn settings_post_updates_in_memory_state() {
        let app = app_router();

        let payload = json!({
            "maintenance_mode": true,
            "notification_channel": "pagerduty://oncall",
            "retention_days": 120
        });

        let post_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/admin/settings")
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(post_response.status(), StatusCode::OK);
        let post_body = json_body(post_response).await;
        assert_eq!(post_body["status"], "ok");
        assert_eq!(post_body["data"]["maintenance_mode"], true);
        assert_eq!(post_body["data"]["retention_days"], 120);

        let get_response = app
            .oneshot(
                Request::builder()
                    .uri("/api/admin/settings")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let get_body = json_body(get_response).await;
        assert_eq!(get_body["data"]["maintenance_mode"], true);
        assert_eq!(get_body["data"]["notification_channel"], "pagerduty://oncall");
    }

    #[tokio::test]
    async fn incident_post_and_patch_flow_returns_expected_payload() {
        let app = app_router();

        let create_payload = json!({
            "title": "Database saturation",
            "severity": "major"
        });

        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/admin/status/incidents")
                    .header("content-type", "application/json")
                    .body(Body::from(create_payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(create_response.status(), StatusCode::OK);
        let created = json_body(create_response).await;
        let id = created["data"]["id"].as_str().unwrap().to_string();
        assert_eq!(created["data"]["state"], "open");

        let patch_payload = json!({ "state": "resolved" });

        let patch_response = app
            .oneshot(
                Request::builder()
                    .method(Method::PATCH)
                    .uri(format!("/api/admin/status/incidents/{id}"))
                    .header("content-type", "application/json")
                    .body(Body::from(patch_payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(patch_response.status(), StatusCode::OK);
        let patched = json_body(patch_response).await;
        assert_eq!(patched["status"], "ok");
        assert_eq!(patched["data"]["id"], id);
        assert_eq!(patched["data"]["state"], "resolved");
    }

    #[tokio::test]
    async fn admin_status_reports_open_incident_count() {
        let app = app_router();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/admin/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = json_body(response).await;
        assert_eq!(body["status"], "ok");
        assert!(body["data"]["incidents_open"].is_number());
        assert!(body["data"]["incidents"].is_array());
        assert!(body["data"]["admin_state"]["blocked_users"].is_array());
        assert!(body["data"]["admin_state"]["resource_adjustments"].is_object());
    }

    #[tokio::test]
    async fn block_user_route_persists_state_in_status() {
        let app = app_router();

        let block_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/admin/users/user-1/block")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(block_response.status(), StatusCode::OK);
        let block_body = json_body(block_response).await;
        assert_eq!(block_body["data"]["user_id"], "user-1");
        assert_eq!(block_body["data"]["blocked"], true);

        let status_response = app
            .oneshot(
                Request::builder()
                    .uri("/api/admin/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let status_body = json_body(status_response).await;
        assert_eq!(status_body["data"]["admin_state"]["blocked_users"], json!(["user-1"]));
    }

    #[tokio::test]
    async fn unblock_user_route_removes_user_from_state() {
        let app = app_router();

        app.clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/admin/users/user-2/block")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let unblock_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/admin/users/user-2/unblock")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(unblock_response.status(), StatusCode::OK);
        let unblock_body = json_body(unblock_response).await;
        assert_eq!(unblock_body["data"]["blocked"], false);

        let status_response = app
            .oneshot(
                Request::builder()
                    .uri("/api/admin/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let status_body = json_body(status_response).await;
        assert_eq!(status_body["data"]["admin_state"]["blocked_users"], json!([]));
    }

    #[tokio::test]
    async fn resource_adjustments_accumulate_by_user_and_resource() {
        let app = app_router();

        let add_payload = json!({ "resource": "credits", "delta": 50 });
        let subtract_payload = json!({ "resource": "credits", "delta": -15 });

        let add_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/admin/users/user-3/resources")
                    .header("content-type", "application/json")
                    .body(Body::from(add_payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(add_response.status(), StatusCode::OK);
        let add_body = json_body(add_response).await;
        assert_eq!(add_body["data"]["total"], 50);

        let subtract_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/admin/users/user-3/resources")
                    .header("content-type", "application/json")
                    .body(Body::from(subtract_payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        let subtract_body = json_body(subtract_response).await;
        assert_eq!(subtract_body["data"]["total"], 35);

        let status_response = app
            .oneshot(
                Request::builder()
                    .uri("/api/admin/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let status_body = json_body(status_response).await;
        assert_eq!(
            status_body["data"]["admin_state"]["resource_adjustments"]["user-3"]["credits"],
            35
        );
    }

    #[tokio::test]
    async fn maintenance_start_and_get_routes_report_active_window() {
        let app = app_router();

        let start_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/admin/maintenance/start")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(start_response.status(), StatusCode::OK);
        let start_body = json_body(start_response).await;
        assert_eq!(start_body["data"]["active"], true);
        assert!(start_body["data"]["started_at_epoch_seconds"].is_number());
        assert!(start_body["data"]["stopped_at_epoch_seconds"].is_null());

        let get_response = app
            .oneshot(
                Request::builder()
                    .uri("/api/admin/maintenance")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let get_body = json_body(get_response).await;
        assert_eq!(get_body["data"]["active"], true);
        assert!(get_body["data"]["started_at_epoch_seconds"].is_number());
    }

    #[tokio::test]
    async fn maintenance_stop_route_transitions_back_to_inactive() {
        let app = app_router();

        app.clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/admin/maintenance/start")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let stop_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/admin/maintenance/stop")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(stop_response.status(), StatusCode::OK);
        let stop_body = json_body(stop_response).await;
        assert_eq!(stop_body["data"]["active"], false);
        assert!(stop_body["data"]["stopped_at_epoch_seconds"].is_number());

        let status_response = app
            .oneshot(
                Request::builder()
                    .uri("/api/admin/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let status_body = json_body(status_response).await;
        assert_eq!(status_body["data"]["admin_state"]["maintenance"]["active"], false);
    }

    #[tokio::test]
    async fn dashboard_reflects_admin_operational_state_counts() {
        let app = app_router();

        app.clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/admin/users/user-5/block")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        app.clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/admin/users/user-6/resources")
                    .header("content-type", "application/json")
                    .body(Body::from(json!({ "resource": "quota", "delta": 1 }).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        app.clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/admin/maintenance/start")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let dashboard_response = app
            .oneshot(
                Request::builder()
                    .uri("/api/admin/dashboard")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let dashboard_body = json_body(dashboard_response).await;
        assert_eq!(dashboard_body["data"]["maintenance_active"], true);
        assert_eq!(dashboard_body["data"]["blocked_users"], 1);
        assert_eq!(dashboard_body["data"]["adjusted_users"], 1);
    }

    #[tokio::test]
    async fn unknown_admin_route_returns_not_found() {
        let app = app_router();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/admin/not-a-route")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
