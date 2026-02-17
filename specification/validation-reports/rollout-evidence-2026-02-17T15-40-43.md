# Rust Rollout Evidence (Non-Docker Runtime Execution)

Timestamp: 2026-02-17T15:40:45.8690719+00:00

| Check | Result |
| --- | --- |
| Compose services rendered | PASS |
| Legacy compose service names absent | PASS |
| Legacy compose profiles absent | PASS |
| N-API source/build references absent in workspace paths | PASS |

## Compose Services

```text
database
rabbitmq
redis
rust-analytics-worker
rust-bot-api
rust-bot-worker
rust-sms-api
rust-core-engine
rust-api-gateway
rust-web-frontend
alertmanager
blackbox-exporter
node-exporter
prometheus
grafana
rust-realtime-gateway
rust-app-core-engine
rust-chat-worker
otel-collector
rust-sharding-worker
rust-admin-api
rust-email-worker
rust-notifications-worker
rust-scheduler-worker
```

## Legacy Service Name Scan

```text
(none)
```

## Legacy Profile Scan

```text
(none)
```

## backend-core-napi Scan

```text
specification\spec-rust-route-ownership.md:57:| `backend-core-napi` bridge | none (retire) | Runtime service paths migrated; source files retired and compatibility notes archived |
specification\spec-rust-route-ownership.md:63:- `backend-core-napi` removed from default workspace build graph and runtime dependency graph.
specification\spec-rust-crate-partition.md:36:| Core engine bridge | `app-core-engine` | Rust path active | Final decommission of `backend-core-napi` source |
specification\spec-rust-crate-partition.md:52:5. Tranche E: Remove `backend-core-napi` source after rollback window.
specification\spec-rust-backend.md:165:- `backend-core-napi` -> retired.
specification\rust-final-cutover-checklist.md:28:| S-003 | `backend-core-napi` bridge source ownership | `crates/backend-core-napi` | `app-core-engine` + `platform-proto` | `game-combat`, `game-fleet` | [x] Remove remaining runtime and Node unit-test dependencies on N-API bridge (runtime service paths migrated; benchmark scripts migrated).<br>[x] Remove crate from default workspace build graph.<br>[x] Archive compatibility notes in migration docs (`specification/backend-core-napi-retirement.md`).<br>[x] N-API crate source files removed from repository. |
specification\backend-core-napi-retirement.md:1:# backend-core-napi Retirement Notes
specification\backend-core-napi-retirement.md:6:- The legacy N-API bridge crate `crates/backend-core-napi` has been retired from active backend migration paths.
specification\backend-core-napi-retirement.md:8:- The workspace already excluded `backend-core-napi`; source files are now removed.
specification\backend-core-napi-retirement.md:16:- No compose/runtime service depends on `backend-core-napi`.
```
