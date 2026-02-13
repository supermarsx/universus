# Rust Route Ownership Matrix

Updated: February 13, 2026

## Purpose
This matrix reframes the Node backend migration by crate ownership. Every legacy route family is mapped to a Rust crate (or marked intentionally deferred) so cutover can be driven by explicit boundaries.

## Runtime Crate Owners (Primary)
| Legacy mount path (Node) | Legacy route file(s) | Rust crate owner | Rust status |
| --- | --- | --- | --- |
| `/api/auth` | `backend/src/routes/auth.ts` | `crates/app-api-gateway` + `crates/platform-auth` | In progress |
| `/api/account` | `backend/src/routes/accountRoutes.ts` | `crates/app-api-gateway` + `crates/platform-auth` | In progress |
| `/api/planets` | `backend/src/routes/planets.ts` | `crates/app-api-gateway` + `crates/game-economy` | In progress |
| `/api/fleet` | `backend/src/routes/fleet.ts` | `crates/app-api-gateway` + `crates/game-fleet` | In progress |
| `/api/research` | `backend/src/routes/research.ts` | `crates/app-api-gateway` + `crates/game-economy` | In progress |
| `/api/shipyard` | `backend/src/routes/shipyard.ts` | `crates/app-api-gateway` + `crates/game-economy` | In progress |
| `/api/galaxy` | `backend/src/routes/galaxy.ts` | `crates/app-api-gateway` + `crates/game-galaxy` | In progress |
| `/api/leaderboard` | `backend/src/routes/leaderboard.ts` | `crates/app-api-gateway` + `crates/game-leaderboard` | In progress |
| `/api/messages` | `backend/src/routes/messages.ts` | `crates/app-api-gateway` + `crates/game-messaging` | In progress |
| `/api/shop` | `backend/src/routes/shop.ts` | `crates/app-api-gateway` + `crates/adapter-provider-payments` | In progress |
| `/api/shop-enhanced` | `backend/src/routes/enhancedShopRoutes.ts` | `crates/app-api-gateway` + `crates/game-economy` | Pending |
| `/api/alliances` | `backend/src/routes/allianceRoutes.ts` | `crates/app-api-gateway` + `crates/game-alliance` | In progress |
| `/api/moons` | `backend/src/routes/moons.ts` | `crates/app-api-gateway` + `crates/game-moon` | In progress |
| `/api/rips` | `backend/src/routes/rips.ts` | `crates/app-api-gateway` + `crates/game-moon` | Pending |
| `/api/debris` | `backend/src/routes/debrisRoutes.ts` | `crates/app-api-gateway` + `crates/game-combat` | In progress |
| `/api/player-blocks` | `backend/src/routes/playerBlocks.ts` | `crates/app-api-gateway` + `crates/game-antiabuse` | Pending |
| `/api/config` | `backend/src/routes/configRoutes.ts` | `crates/app-api-gateway` + `crates/platform-config` | Pending |
| `/api/themes` | `backend/src/routes/themeRoutes.ts` | `crates/app-api-gateway` + `crates/platform-config` | Pending |
| `/api/universe` | `backend/src/routes/universeRoutes.ts` | `crates/app-api-gateway` + `crates/game-universe` | In progress |
| `/api/shards` | `backend/src/routes/shardingRoutes.ts` | `crates/app-api-gateway` + `crates/game-universe` | Pending |
| `/api/realtime` | `backend/src/routes/realtimeRoutes.ts` | `crates/app-realtime-gateway` + `crates/platform-events` | Pending parity pass |
| `/api/analytics` | `backend/src/routes/analytics.ts` | `crates/app-analytics-worker` + `crates/platform-events` | Pending ingestion API parity |
| `/api/admin/*` | `backend/src/routes/admin.ts`, `backend/src/routes/adminRoutes.ts` | `crates/app-admin-api` | In progress |
| `/api/admin/bots/*` | `backend/src/routes/bots.ts` | `crates/app-bot-api` + `crates/app-bot-worker` | In progress |
| `/` template/static routes | `backend/src/routes/templates.ts` | frontend/static hosting; not backend business logic | Deferred from backend cutover |

## Legacy Route Files Not Mounted in `backend/src/index.ts`
These still need explicit retirement or remount decisions.

- `backend/src/routes/achievementRoutes.ts` -> target `crates/game-achievements` + `crates/app-api-gateway`
- `backend/src/routes/marketplaceRoutes.ts` -> target `crates/game-economy` + `crates/app-api-gateway`

## Service Cutover Ownership by Crate
| Legacy service/process | Rust replacement crate(s) | Runtime readiness |
| --- | --- | --- |
| `backend` | `crates/app-api-gateway`, `crates/app-realtime-gateway`, `crates/game-*`, `crates/platform-*` | Partial |
| `backend-admin-service` | `crates/app-admin-api` | Partial |
| `backend-bot-service` API | `crates/app-bot-api` | Partial |
| `backend-bot-service` worker | `crates/app-bot-worker` | Pending full behavior parity |
| `backend-sms-service` | `crates/app-sms-api`, `crates/adapter-provider-sms` | Partial |
| `email-delivery-service` | `crates/app-email-worker`, `crates/adapter-provider-email` | Runtime wired in compose; provider parity pending |
| analytics queue worker (`backend/src/workers`) | `crates/app-analytics-worker` | Runtime wired in compose; ingestion parity pending |
| `backend-core` | `crates/app-core-engine` + domain crates | Partial; legacy gRPC path still active |
| `backend-core-napi` bridge | none (retire) | Pending |

## Execution Gate (for Node retirement)
- 100% route-family ownership mapped to a Rust crate.
- No Node service in production traffic path for API/admin/bot/sms/email/analytics.
- Realtime contract snapshots validated against current frontend client.
- `backend-core-napi` removed from runtime dependency graph.
