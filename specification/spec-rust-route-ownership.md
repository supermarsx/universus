# Rust Route Ownership Matrix

Updated: February 16, 2026

## Purpose
This matrix reframes the Node backend migration by crate ownership. Every legacy route family is mapped to a Rust crate (or marked intentionally deferred) so cutover can be driven by explicit boundaries.

## Runtime Crate Owners (Primary)
| Legacy mount path (Node) | Legacy route file(s) | Rust crate owner | Rust status |
| --- | --- | --- | --- |
| `/api/auth` | `backend/src/routes/auth.ts` | `crates/app-api-gateway` + `crates/platform-auth` | In progress |
| `/api/account` | `backend/src/routes/accountRoutes.ts` | `crates/app-api-gateway` + `crates/platform-auth` | In progress |
| `/api/users` | `backend/src/routes/users.ts` | `crates/app-api-gateway` + `crates/game-leaderboard` | In progress |
| `/api/planets` | `backend/src/routes/planets.ts` | `crates/app-api-gateway` + `crates/game-economy` | In progress |
| `/api/fleet` | `backend/src/routes/fleet.ts` | `crates/app-api-gateway` + `crates/game-fleet` | In progress |
| `/api/research` | `backend/src/routes/research.ts` | `crates/app-api-gateway` + `crates/game-economy` | In progress |
| `/api/shipyard` | `backend/src/routes/shipyard.ts` | `crates/app-api-gateway` + `crates/game-economy` | In progress |
| `/api/galaxy` | `backend/src/routes/galaxy.ts` | `crates/app-api-gateway` + `crates/game-galaxy` | In progress |
| `/api/leaderboard` | `backend/src/routes/leaderboard.ts` | `crates/app-api-gateway` + `crates/game-leaderboard` | In progress |
| `/api/messages` | `backend/src/routes/messages.ts` | `crates/app-api-gateway` + `crates/game-messaging` | In progress |
| `/api/shop` | `backend/src/routes/shop.ts` | `crates/app-api-gateway` + `crates/adapter-provider-payments` | In progress |
| `/api/shop-enhanced` | `backend/src/routes/enhancedShopRoutes.ts` | `crates/app-api-gateway` + `crates/game-economy` | In progress |
| `/api/marketplace` | `backend/src/routes/marketplaceRoutes.ts` | `crates/app-api-gateway` + `crates/game-economy` | In progress |
| `/api/alliances` | `backend/src/routes/allianceRoutes.ts` | `crates/app-api-gateway` + `crates/game-alliance` | In progress |
| `/api/achievements` | `backend/src/routes/achievementRoutes.ts` | `crates/app-api-gateway` + `crates/game-achievements` | In progress |
| `/api/acs` | `backend/src/routes/acs.ts` | `crates/app-api-gateway` + `crates/game-fleet` | In progress |
| `/api/moons` | `backend/src/routes/moons.ts` | `crates/app-api-gateway` + `crates/game-moon` | In progress |
| `/api/rips` | `backend/src/routes/rips.ts` | `crates/app-api-gateway` + `crates/game-moon` | In progress |
| `/api/debris` | `backend/src/routes/debrisRoutes.ts` | `crates/app-api-gateway` + `crates/game-combat` | In progress |
| `/api/player-blocks` | `backend/src/routes/playerBlocks.ts` | `crates/app-api-gateway` + `crates/game-antiabuse` | In progress |
| `/api/config` | `backend/src/routes/configRoutes.ts` | `crates/app-api-gateway` + `crates/platform-config` | In progress |
| `/api/themes` | `backend/src/routes/themeRoutes.ts` | `crates/app-api-gateway` + `crates/platform-config` | In progress |
| `/api/universe` | `backend/src/routes/universeRoutes.ts` | `crates/app-api-gateway` + `crates/game-universe` | In progress |
| `/api/shards` | `backend/src/routes/shardingRoutes.ts` | `crates/app-api-gateway` + `crates/game-universe` | In progress |
| `/api/realtime` | `backend/src/routes/realtimeRoutes.ts` | `crates/app-realtime-gateway` + `crates/platform-events` | In progress |
| `/api/analytics` | `backend/src/routes/analytics.ts` | `crates/app-api-gateway`, `crates/app-analytics-worker` + `crates/platform-events` | In progress |
| `/api/notifications` | `backend/src/services/notificationService.ts` (service-owned surface) | `crates/app-api-gateway` + `crates/game-notifications` | In progress |
| `/api/admin/*` | `backend/src/routes/admin.ts`, `backend/src/routes/adminRoutes.ts` | `crates/app-admin-api` | In progress |
| `/api/admin/bots/*` | `backend/src/routes/bots.ts` | `crates/app-bot-api` + `crates/app-bot-worker` | In progress |
| `/` template/static routes | `backend/src/routes/templates.ts` | `crates/app-web-frontend` | In progress |

## Legacy Route Files Not Mounted in `backend/src/index.ts`
No remaining unmounted route files requiring new Rust ownership mapping from this audit slice.

## Service Cutover Ownership by Crate
| Legacy service/process | Rust replacement crate(s) | Runtime readiness |
| --- | --- | --- |
| `backend` | `crates/app-api-gateway`, `crates/app-realtime-gateway`, `crates/game-*`, `crates/platform-*` | Partial |
| `backend-admin-service` | `crates/app-admin-api` | Partial |
| `backend-bot-service` API | `crates/app-bot-api` | Partial |
| `backend-bot-service` worker | `crates/app-bot-worker` | Pending full behavior parity |
| `backend-sms-service` | `crates/app-sms-api`, `crates/adapter-provider-sms` | Partial |
| `email-delivery-service` | `crates/app-email-worker`, `crates/adapter-provider-email` | Runtime wired in compose; provider parity pending |
| notification orchestration (`notificationService`) | `crates/game-notifications`, `crates/app-api-gateway` | Base API parity landed (list/create/read-state endpoints); DB + realtime fanout parity pending |
| analytics queue worker (`backend/src/workers`) | `crates/app-analytics-worker` | Runtime wired in compose; RabbitMQ consumer + DB persistence implemented; aggregation parity pending |
| `backend-core` | `crates/app-core-engine` + domain crates | Partial; legacy gRPC path still active |
| `backend-core-napi` bridge | none (retire) | Runtime service paths migrated to gRPC/local fallback and Node unit-test cleanup completed; source retention/deletion steps pending |

## Execution Gate (for Node retirement)
- 100% route-family ownership mapped to a Rust crate.
- No Node service in production traffic path for API/admin/bot/sms/email/analytics.
- Realtime contract snapshots validated against current frontend client.
- `backend-core-napi` removed from default workspace build graph and runtime dependency graph.

## Latest parity increments
- Added Rust API compatibility aliases for legacy Node paths:
  - `/api/health` now served by `app-api-gateway` in addition to `/health`.
  - `/api/alliances/*` now aliases to existing alliance handlers (`/api/alliance/*`).
- Notifications parity slice advanced:
  - DB-backed notification preferences (`enabled`, `minPriority`) now represented on Rust API routes.
  - Notification create/read events now publish via shared `platform-events` envelope/publisher to realtime gateway (`REALTIME_GATEWAY_URL`).
  - Scheduled cleanup moved to Rust worker (`app-notifications-worker`) using `platform-db` cleanup methods.
- Scheduler parity slice advanced:
  - Rust `app-scheduler-worker` introduced for interval-based orchestration replacement (`gameLoop`, fleet, moon-destroy, shard-health cadence scaffold).
  - Scheduler ticks emit typed ops events through `platform-events`.
  - Scheduler now uses DB-backed queued task lifecycle (`enqueue -> claim -> complete/fail`) via `platform-db`.
  - Scheduler enqueue now includes cadence-bucket dedupe keys in `platform-db` to reduce duplicate tasks across concurrent workers.
  - `app-core-engine` now includes `/engine/tasks/process` for centralized scheduled task processing.
- Sharding parity slice advanced:
  - Rust `app-sharding-worker` introduced for server heartbeat upsert cadence and stale shard expiration using `platform-db`.
  - Sharding maintenance cycles emit typed ops events through `platform-events`.
  - `app-realtime-gateway` now exposes `/api/realtime/events/recent` for contract/ops validation of published events.
  - Shard message bus ops now expose queue metrics and recovery controls on Rust API:
    - `GET /api/shards/messages/status` returns queue state counts and lag when DB is configured.
    - `GET /api/shards/messages/failed` returns failed messages for diagnostics and replay planning.
    - `POST /api/shards/messages/requeue-failed` requeues failed cross-server messages for retry.
- Default runtime compose wiring now includes crate-owned workers:
  - `rust-app-core-engine` (HTTP task processing API for queued scheduler work)
  - `rust-notifications-worker`
  - `rust-chat-worker`
  - `rust-scheduler-worker`
  - `rust-sharding-worker`
  - `rust-bot-worker`
  - Rust service env wiring now includes `DATABASE_URL`/`REALTIME_GATEWAY_URL`/`REDIS_URL`/`RABBITMQ_URL` where required.
- Chat moderation/restriction parity advanced:
  - Chat restriction source of truth moved to `platform-db` (`chat_restrictions` + cleanup API methods).
  - `app-chat-worker` now performs DB-backed expired restriction cleanup and emits `ops.chat` cleanup events.
  - `app-realtime-gateway` now exposes restriction moderation endpoints:
    - `GET /api/realtime/chat/restrictions`
    - `POST /api/realtime/chat/restrictions`
    - `DELETE /api/realtime/chat/restrictions`
