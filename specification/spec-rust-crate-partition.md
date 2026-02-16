# Rust Backend Reframe by Crate Boundaries

Updated: 2026-02-15

## Intent
Define the backend as a Rust-first system partitioned by strict crate layers so legacy Node services can be retired in controlled tranches.

## Layering Model
| Layer | Responsibility | Crates |
| --- | --- | --- |
| App (edge/runtime) | HTTP APIs, workers, process entrypoints | `app-api-gateway`, `app-admin-api`, `app-bot-api`, `app-bot-worker`, `app-chat-worker`, `app-email-worker`, `app-sms-api`, `app-analytics-worker`, `app-notifications-worker`, `app-scheduler-worker`, `app-sharding-worker`, `app-realtime-gateway`, `app-web-frontend`, `app-core-engine` |
| Domain (game logic) | Pure game rules and domain state transitions | `game-fleet`, `game-combat`, `game-economy`, `game-galaxy`, `game-alliance`, `game-moon`, `game-universe`, `game-achievements`, `game-messaging`, `game-notifications`, `game-chat`, `game-antiabuse`, `game-leaderboard`, `game-domain` |
| Platform (cross-cutting infra) | Config, auth, DB, cache, events, observability, shared errors/common | `platform-config`, `platform-auth`, `platform-db`, `platform-cache`, `platform-events`, `platform-observability`, `platform-errors`, `platform-common`, `platform-proto` |
| Adapter (external providers/compat) | Provider integrations and protocol adapters | `adapter-provider-email`, `adapter-provider-sms`, `adapter-provider-bot`, `adapter-provider-payments`, `adapter-http-compat` |

## Dependency Rules
- App crates can depend on domain/platform/adapter crates.
- Domain crates cannot depend on app crates.
- Platform crates cannot depend on app/domain crates.
- Adapter crates may depend on platform crates but not app crates.
- Runtime state should move from in-memory mocks to `platform-db` + `platform-events` for parity cutover.

## Migration Status Snapshot
| Area | Current Rust owner | Status | Gap to close |
| --- | --- | --- | --- |
| API route families | `app-api-gateway` | Broad coverage landed | Deep parity for DB-backed behavior and side-effects |
| Notifications | `game-notifications` + `app-api-gateway` + `platform-db` + `app-realtime-gateway` + `app-notifications-worker` + `platform-events` | DB-first endpoints, preference filtering, realtime publish hook, cleanup worker, and shared event envelope/publisher landed | Client integration parity validation under production traffic |
| Chat restriction cleanup | `game-chat` + `app-chat-worker` | Worker + domain cleanup loop landed | DB-backed restriction state + moderation parity |
| Scheduler orchestration | `app-scheduler-worker` + `platform-events` | Interval orchestration + typed event emission landed | Replace remaining bridge semantics with full domain-triggered scheduler behavior parity |
| Shard heartbeat/discovery cadence | `app-sharding-worker` + `platform-db` + `platform-events` | Worker heartbeat + stale-expiration + ops-event emission landed | Cross-server messaging and routing policy-depth parity |
| SMS | `app-sms-api` + `adapter-provider-sms` | API + SQLite + idempotency + circuit breaker landed | Provider-level production integration hardening |
| Email worker | `app-email-worker` + `adapter-provider-email` | Queue parse/provider interface landed | Production provider + retry/backoff policy parity |
| Bot processing | `app-bot-api` + `app-bot-worker` + `adapter-provider-bot` | Worker-trigger path landed | DB-backed bot scheduling parity |
| Analytics | `app-analytics-worker` | RabbitMQ consume + DB persist landed | Aggregations and dashboards parity |
| Node template routes | `app-web-frontend` | Rust frontend service present | Full route/auth parity versus `templates.ts` |
| Core engine bridge | `app-core-engine` | Rust path active | Final decommission of `backend-core-napi` source |

## Highest-Priority Remaining Legacy Surfaces
1. `backend/src/routes/templates.ts` and server-rendered auth/page gates.
2. `backend/src/services/notificationService.ts` full parity hardening (realtime contract validation).
3. `backend/src/services/chatService.ts` moderation behavior-depth parity (cleanup loop is now crate-owned).
4. `backend/src/services/serverDiscoveryService.ts` and `crossServerCommunicationService.ts` behavior-depth parity hardening (messaging + failover policy).
5. `backend/src/services/gameLoopService.ts`, `fleetScheduler.ts`, `destroyMoonService.ts` behavior-depth scheduler parity in Rust workers.
6. `backend-admin-service/src/index.ts` admin behavior-depth parity and production rollout validation.
7. `backend-bot-service` rule parity and long-running worker control-plane behavior.

## Execution Tranches
1. Tranche A: Notifications + chat + realtime events on Rust-only path.
2. Tranche B: Scheduler and sharding services moved to dedicated Rust workers.
3. Tranche C: Frontend template route parity and auth gate migration.
4. Tranche D: Retire legacy Node services from default runtime path.
5. Tranche E: Remove `backend-core-napi` source after rollback window.
