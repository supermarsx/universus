# Rust Final Cutover Checklist (Unrepresented Node Surfaces)

Updated: 2026-02-15

## Audit Scope
This checklist covers Node routes/services that are still active in entrypoints but do not yet have explicit Rust crate representation.

Audit inputs:
- `backend/src/index.ts`
- `backend/src/routes/*.ts`
- `backend-admin-service/src/index.ts`
- `backend-bot-service/src/index.ts`
- `backend-sms-service/src/index.ts`
- `email-delivery-service/src/index.ts`
- `crates/*`

Excluded from this file:
- Node surfaces already represented by existing Rust crates (API/admin/bot/sms/realtime/email/analytics/core families).

## Route-Level Checklist
All previously unrepresented mounted route surfaces (`/api/users`, `/api/acs`) are now represented in `crates/app-api-gateway` with parity tests. Remaining route-level work is behavior-depth hardening (DB parity), tracked in `specification/spec-rust-route-ownership.md`.

## Service-Level Checklist
| TODO ID | Node service/surface | Node source of truth | Crate owner (primary) | Crate owner (supporting) | Concrete cutover checklist |
| --- | --- | --- | --- | --- | --- |
| S-001 | Server-rendered template/page routing (`/`, `/overview`, `/admin/*`, `/account/*`, `/alliance/*`, etc.) currently served by Node templates router | `backend/src/routes/templates.ts` mounted at `/` in `backend/src/index.ts` | `app-web-frontend` | `app-api-gateway`, `platform-auth` | [x] Ownership model chosen: Rust-owned frontend service (`app-web-frontend`) for rust-only cutover path.<br>[x] Runtime wiring switched: `docker-compose.yml` now uses `rust-web-frontend` as the rust-only default UI entrypoint on host `:8080` and moves legacy Node `frontend` behind explicit `legacy-frontend` profile on host `:8081` to avoid conflicts.<br>[x] `crates/app-web-frontend` route map parity established for page endpoints from `templates.ts`.<br>[ ] Preserve auth gate behavior (`authenticateToken`, `assertAuthenticated`, `requireAdmin`) or migrate auth gating to frontend + API-only backend policy explicitly.<br>[ ] Reproduce Nunjucks page delivery behavior (or approved replacement) and validate all current page URLs.<br>[ ] Remove Node template router from runtime after parity sign-off. |
| S-002 | Legacy Node backend runtime services (`backend`, `bot-service`, `admin-service`, `backend-core`) | `docker-compose.yml` | Rust service set (`app-api-gateway`, `app-admin-api`, `app-bot-api`, `app-core-engine`) | `app-realtime-gateway`, domain crates | [x] Legacy services moved behind explicit `legacy-node` compose profile.<br>[x] Rust services set as default compose runtime path (legacy remains opt-in via profile).<br>[ ] Validate production rollout with `legacy-node` profile disabled.<br>[ ] Remove legacy services from compose after final rollback window. |
| S-003 | `backend-core-napi` bridge source ownership | `crates/backend-core-napi` | `app-core-engine` + `platform-proto` | `game-combat`, `game-fleet` | [x] Remove remaining runtime and Node unit-test dependencies on N-API bridge (runtime service paths migrated; benchmark scripts migrated).<br>[x] Remove crate from default workspace build graph (source retained; full deletion deferred until parity acceptance).<br>[ ] Archive compatibility notes in migration docs.<br>[ ] Delete crate source after final parity/SLO acceptance window. |
| S-004 | Notification orchestration and unread state | `backend/src/services/notificationService.ts` | `game-notifications`, `app-api-gateway` | `platform-db`, `platform-events`, `app-realtime-gateway`, `app-notifications-worker` | [x] Base notification domain crate created and integrated into Rust API gateway with protected CRUD/read-state routes.<br>[x] DB-first persistence/read-state implemented in `platform-db` and wired into gateway with in-memory fallback when DB is unavailable.<br>[x] Preference endpoints/filtering implemented (`enabled`, `minPriority`) and enforced on create path.<br>[x] Shared event envelope + HTTP publish path implemented via `platform-events` and wired into notification create/read flows.<br>[x] Scheduled cleanup worker implemented (`app-notifications-worker`) with DB cleanup operations for expired and old archived notifications.<br>[x] Realtime gateway now captures recent published events via `/api/realtime/events/recent` with contract test coverage.<br>[ ] Validate full client integration snapshots under production-like traffic. |
| S-005 | Chat restriction cleanup scheduler | `backend/src/services/chatService.ts` (auto-expire restrictions) | `game-chat`, `app-chat-worker` | `platform-db`, `app-realtime-gateway` | [x] Rust domain crate and worker loop implemented for restriction cleanup cycles.<br>[ ] Move restriction source of truth to `platform-db` and retain process-safe semantics.<br>[ ] Port moderation/restriction APIs and realtime propagation behavior.<br>[ ] Validate parity of expiry side effects against Node service behavior. |
| S-006 | Runtime scheduler orchestration (`gameLoop`, fleet, moon destroy, shard health cadence) | `backend/src/index.ts`, `backend/src/services/gameLoopService.ts`, `backend/src/services/fleetScheduler.ts`, `backend/src/services/destroyMoonService.ts` | `app-scheduler-worker` | `app-api-gateway`, `app-core-engine`, `platform-events` | [x] Rust scheduler worker scaffold created with env-driven intervals and run-once mode.<br>[x] Typed scheduler tick events emitted through shared `platform-events` publisher to realtime gateway ops channels.<br>[ ] Replace remaining bridge/noop semantics with domain-native scheduler triggers and durable queues/events.<br>[ ] Validate tick cadence/ordering/idempotency parity against Node runtime behavior.<br>[ ] Remove Node scheduler intervals from production traffic path. |
| S-007 | Shard heartbeat/discovery maintenance cadence | `backend/src/services/serverDiscoveryService.ts`, `backend/src/services/crossServerCommunicationService.ts` | `app-sharding-worker` | `platform-db`, `app-api-gateway`, `platform-events` | [x] Rust worker implemented for shard heartbeat upsert cadence and stale-server expiration checks.<br>[x] Sharding ops events now emitted through shared `platform-events` publisher.<br>[ ] Port cross-server message bus semantics and delivery guarantees.<br>[ ] Validate routing/failover behavior parity under server churn.<br>[ ] Remove Node sharding maintenance loops from production traffic path. |

## Cutover Exit Criteria For This File
- [ ] Every row above has an assigned crate owner (no `TBD`).
- [ ] Rust integration/contract tests exist for each row.
- [ ] Node mounts for these rows are removed from production traffic path.
- [ ] `specification/spec-rust-route-ownership.md` reflects final ownership status.
