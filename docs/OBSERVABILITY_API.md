# Admin Observability API Endpoints

## Overview
These endpoints allow authorized admin users to view and configure observability/monitoring settings for the Universus backend. Access is controlled by the new RBAC model.

---

## Endpoints

### 1. Get Observability Config
- **URL:** `/api/admin/observability/config`
- **Method:** `GET`
- **Auth:** JWT + Admin
- **RBAC:** `superadmin` and `super_game_master` only
- **Response:**
```json
{
  "prometheusUrl": "http://localhost:9090",
  "grafanaUrl": "http://localhost:3000",
  "alertmanagerUrl": "http://localhost:9093",
  "otelCollectorUrl": "http://localhost:4317",
  "blackboxUrl": "http://localhost:9115",
  "enabled": true
}
```

### 2. Update Observability Config
- **URL:** `/api/admin/observability/config`
- **Method:** `PUT`
- **Auth:** JWT + Admin
- **RBAC:** `superadmin` and `super_game_master` only
- **Body:** Partial or full config object (see above)
- **Response:**
```json
{
  "success": true,
  "config": { ...updatedConfig }
}
```

### 3. Get Observability Status
- **URL:** `/api/admin/observability/status`
- **Method:** `GET`
- **Auth:** JWT + Admin
- **RBAC:** `game_master`, `super_mod`, `mod`, `super_game_master`, `superadmin` (all roles above support)
- **Response:**
```json
{
  "prometheus": { "url": "...", "status": "ok" },
  "grafana": { "url": "...", "status": "ok" },
  "alertmanager": { "url": "...", "status": "ok" },
  "otel_collector": { "url": "...", "status": "ok" },
  "blackbox": { "url": "...", "status": "ok" },
  "enabled": true,
  "lastChecked": "2025-11-09T..."
}
```

---

## RBAC Model (Admin Levels)
- `superadmin`: Full access to all admin features
- `super_game_master`: All observability and game config
- `game_master`: Read-only observability, game config
- `super_mod`: Read-only observability, advanced moderation
- `mod`: Read-only observability, basic moderation
- `auditor`: Read-only access to observability and audit logs
- `support`: No observability access

---

## Notes
- These endpoints currently use an in-memory config; swap for DB-backed config for production.
- Extend `/status` to return real health/metrics as needed.
- All endpoints require JWT authentication and proper admin role.

