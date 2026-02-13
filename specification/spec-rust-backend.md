# Universus Full Rust Backend Reframe Specification

## Status
Updated on February 13, 2026.
- Migration mode: hard-cut capable in runtime wiring via `docker compose --profile rust-only`.
- Rust service binaries currently moved and wired:
  - `app-api-gateway` (`rust-api-gateway`)
  - `app-web-frontend` (`rust-web-frontend`, rust-only profile)
  - `app-admin-api` (`rust-admin-api`)
  - `app-bot-api` (`rust-bot-api`)
  - `app-sms-api` (`rust-sms-api`)
  - `app-realtime-gateway` (`rust-realtime-gateway`)
  - `app-core-engine` (`rust-core-engine`)
  - `app-email-worker` (`rust-email-worker`)
  - `app-analytics-worker` (`rust-analytics-worker`)
- Remaining migration gaps before full Node retirement:
  - `backend` route-by-route parity validation and production cutover sign-off.
  - `app-email-worker` and `app-analytics-worker` provider and ingestion parity validation.
  - websocket/event contract parity validation for realtime replacement.
  - decommission of `backend-core-napi` bridge after parity/SLO acceptance.

See `specification/spec-rust-route-ownership.md` for the crate-partitioned route ownership matrix and cutover checklist.

## Current Rust Coverage
| Service family | Implemented crate(s) | Runtime service | Endpoint families now in Rust |
| --- | --- | --- | --- |
| api-gateway | `app-api-gateway` | `rust-api-gateway` | Health/readiness plus `/api/auth/*`, `/api/planets*`, `/api/fleet*`, `/api/combat/simulate`, `/api/alliance*`, `/api/messages*`, `/api/leaderboard*`, `/api/galaxy*`, `/api/shop*`, `/api/research*`, `/api/shipyard*`. |
| web-frontend | `app-web-frontend` | `rust-web-frontend` (rust-only profile) | Server-rendered page routes replacing Node template router during rust-only cutover path (parity checklist tracked in `rust-final-cutover-checklist.md`). |
| admin | `app-admin-api` | `rust-admin-api` | Health/readiness/status plus `/api/admin/dashboard`, `/users`, `/monitoring`, `/settings`, `/analytics`, `/audit`, `/status`, `/events`, `/status/incidents*`. |
| bot | `app-bot-api` | `rust-bot-api` | Health/readiness plus `/api/admin/bots*` families: CRUD, filters, think/process triggers, leaderboard, personalities. |
| sms | `app-sms-api` | `rust-sms-api` | `/api/send`, `/metrics`, `/history`, `/health`, `/ready`. |
| realtime | `app-realtime-gateway` | `rust-realtime-gateway` | `/health`, `/ready`, `/ws-info` (WebSocket metadata endpoint). |
| core-engine | `backend-core` (binary) | `rust-core-engine` | gRPC `GameLoop`: `StartBattle`, `StepBattle`, `StreamBattle`, `SimulateBattle`, `CalculateFleetMovement`. |
| core | `backend-core` (HTTP helper), `game-combat`, `game-fleet` | Consumed by `rust-api-gateway` and `rust-core-engine` | Fleet/combat helper routes: `/api/fleet/helpers/*`, plus deterministic combat/movement handlers. |

## Remaining Legacy Hotspots
- Node `backend` still owns major DB write paths and business parity for many production gameplay routes; hard-cut requires route-level SQL and behavior parity sign-off.
- Node-side bridge/runtime paths remain in circulation (`backend-core-napi`, plus mixed helper fallbacks), so full Rust ownership is not yet complete.
- Frontend cutover now has a dedicated rust-only runtime service (`rust-web-frontend`), but page/auth/session/realtime contract parity still requires final sign-off.

## Objective
Reframe the entire backend as a Rust-first platform with explicit crate boundaries, preserving gameplay behavior while removing Node.js service ownership over time.

## Current Reality (Baseline)
- Existing TypeScript services:
  - `backend` (main gameplay + HTTP + Socket.IO)
  - `backend-admin-service`
  - `backend-bot-service`
  - `backend-sms-service`
  - `email-delivery-service`
- Existing Rust services:
  - `backend-core` (gRPC + optional HTTP helpers + worker IPC)
  - `backend-core-napi` (Node addon bridge)
- Existing top-level Cargo workspace:
  - `Cargo.toml` is a crates-based workspace (members under `crates/*`), including `crates/backend-core` and `crates/backend-core-napi`.

## Target Backend Shape
- All server-side business logic and service processes are Rust.
- Frontend-facing API remains HTTP/JSON + WebSocket semantics compatible with current clients.
- Canonical inter-service contracts are Protobuf/gRPC.
- N-API is transitional only and eventually retired.

## Bounded Contexts to Preserve
- Auth and account security.
- Admin and operations.
- Universe and sharding.
- Core gameplay (planets/buildings/research/shipyard/fleet/combat/galaxy/moons/debris/ACS).
- Alliances and messaging.
- Economy/marketplace/payments.
- Realtime notifications.
- Bot AI and automation.
- Analytics/events.
- SMS verification and outbound delivery.
- Email queue worker and provider dispatch.

## Proposed Rust Workspace
```text
universus-rs/
  Cargo.toml
  crates/
    platform-common
    platform-config
    platform-observability
    platform-errors
    platform-auth
    platform-db
    platform-cache
    platform-events
    platform-proto
    game-domain
    game-combat
    game-fleet
    game-economy
    game-galaxy
    game-moon
    game-alliance
    game-messaging
    game-leaderboard
    game-achievements
    game-antiabuse
    game-universe
    app-api-gateway
    app-realtime-gateway
    app-admin-api
    app-bot-api
    app-bot-worker
    app-sms-api
    app-email-worker
    app-analytics-worker
    app-core-engine
    adapter-http-compat
    adapter-provider-email
    adapter-provider-sms
    adapter-provider-payments
    adapter-provider-bot
```

## Crate Responsibilities
1. `platform-common`: shared primitives, IDs, time/clock traits, serialization helpers.
2. `platform-config`: layered config loader (env/file/remote), runtime feature flags.
3. `platform-observability`: tracing, metrics, OpenTelemetry, request IDs.
4. `platform-errors`: unified error taxonomy and transport mapping.
5. `platform-auth`: JWT/session/permission checks and authz policy evaluation.
6. `platform-db`: Postgres repositories, transaction wrappers, migration helpers.
7. `platform-cache`: Redis abstractions, pub/sub, locks, cache policies.
8. `platform-events`: RabbitMQ/Redis stream producers-consumers and event envelopes.
9. `platform-proto`: shared `.proto` contracts and generated Rust types.
10. `game-domain`: core entity/state models with invariant checks.
11. `game-combat`: deterministic battle simulation and combat helpers.
12. `game-fleet`: movement, missions, fuel/cargo math, scheduler logic.
13. `game-economy`: construction/research/shipyard costs, marketplace calculations, pricing.
14. `game-galaxy`: coordinates, distance/topology, scanning and location rules.
15. `game-moon`: jump gate/phalanx/moon destruction and related cooldown logic.
16. `game-alliance`: alliance membership, diplomacy, wars, announcements.
17. `game-messaging`: player messaging, chat, reactions, blocking policy hooks.
18. `game-leaderboard`: ranking snapshots and global aggregation logic.
19. `game-achievements`: achievements/badges/reward triggers.
20. `game-antiabuse`: throttle/challenge heuristics and bot-protection policy.
21. `game-universe`: seeding, sharding placement, maintenance and health policies.
22. `app-api-gateway`: main HTTP API replacing `backend`.
23. `app-web-frontend`: frontend/page delivery service replacing Node template router in rust-only path.
24. `app-realtime-gateway`: websocket gateway replacing Socket.IO orchestration.
25. `app-admin-api`: admin endpoints replacing `backend-admin-service`.
26. `app-bot-api`: bot admin APIs replacing `backend-bot-service`.
27. `app-bot-worker`: autonomous bot thinking loop worker.
28. `app-sms-api`: outbound SMS/WhatsApp/Telegram/Discord dispatch API.
29. `app-email-worker`: Redis email queue consumer and provider dispatch.
30. `app-analytics-worker`: analytics ingestion/aggregation async worker.
31. `app-core-engine`: evolved version of `backend-core` gRPC engine service.
32. `adapter-http-compat`: compatibility surface for existing HTTP payload shapes.
33. `adapter-provider-email`: SMTP/SendGrid/SES/MailerSend integrations.
34. `adapter-provider-sms`: Twilio/Baileys/Telegram/Discord/custom HTTP channels.
35. `adapter-provider-payments`: Stripe integration and webhook verification.
36. `adapter-provider-bot`: remote bot integrations if externalized.

## Service-to-Crate Mapping (From Current Code)
- `backend` -> `app-api-gateway`, `app-realtime-gateway`, domain crates (`game-*`) and platform crates.
- `frontend` + Node template routing path -> `app-web-frontend` (rust-only cutover path), with `app-api-gateway` and `app-realtime-gateway` as backend dependencies.
- `backend-admin-service` -> `app-admin-api` + `platform-auth`, `platform-db`, `platform-observability`.
- `backend-bot-service` -> `app-bot-api`, `app-bot-worker`, `game-antiabuse`, `game-fleet`, `game-economy`.
- `backend-sms-service` -> `app-sms-api`, `adapter-provider-sms`, `platform-events`.
- `email-delivery-service` -> `app-email-worker`, `adapter-provider-email`, `platform-cache`.
- `backend-core` -> `app-core-engine`, `game-combat`, `game-fleet`, `platform-proto`.
- `backend-core-napi` -> temporary bridge only; no long-term ownership.

## Transport and Compatibility Rules
- External client interface:
  - Keep existing REST endpoint contracts and auth semantics during migration.
  - Keep websocket event names/payload contracts during migration.
- Internal service interface:
  - gRPC/Protobuf is canonical for service-to-service.
  - HTTP helper endpoints are compatibility-only and sunset once clients are migrated.
- N-API policy:
  - Development/perf fallback only in transition.
  - Production path targets gRPC consistently.

## Data and Persistence Rules
- Database remains PostgreSQL with existing schema evolution from `database/sql/steps`.
- Every domain crate owns repository interfaces; SQL implementations stay in `platform-db`.
- Idempotency must be explicit for outbound operations (email/SMS/payments/webhooks).
- Exactly-once is not assumed; at-least-once plus dedup keys is required.

## Runtime and Deployment Model
- Per-service binaries with independent autoscaling.
- Stateless API services; state in Postgres/Redis/RabbitMQ.
- Worker binaries for asynchronous responsibilities:
  - `app-bot-worker`
  - `app-email-worker`
  - `app-analytics-worker`
- Shared observability stack (Prometheus/Grafana/OpenTelemetry) preserved.

## Migration Phases
1. Phase 0: Workspace bootstrap.
   - Expand root `Cargo.toml` workspace.
   - Split `backend-core` into `app-core-engine` + reusable `game-combat/game-fleet`.
2. Phase 1: Core gameplay domain extraction.
   - Port deterministic/pure logic first: combat, movement, economy calculators.
   - Keep Node API layer calling Rust over gRPC as facade.
3. Phase 2: HTTP gateway replacement.
   - Introduce `app-api-gateway` route-by-route parity with existing `backend/src/routes`.
   - Run shadow traffic + response diffing.
4. Phase 3: Service replacement.
   - Replace admin, bot, SMS, and email services with Rust binaries.
   - Keep original endpoints and auth contracts stable.
5. Phase 4: Realtime replacement.
   - Replace Socket.IO orchestration with `app-realtime-gateway`.
   - Preserve event contracts for frontend compatibility.
6. Phase 5: Node retirement.
   - Decommission TS services after SLO and parity acceptance windows.
   - Remove N-API and HTTP helper compatibility paths.

## Phase Checklist (Current)
- [x] Phase 0: Workspace bootstrap and Rust service scaffold in place.
- [~] Phase 1: Core gameplay domain extraction (in progress; mixed `backend-core` and `app-core-engine` path).
- [~] Phase 2: HTTP gateway replacement (Rust gateway wired; parity validation pending).
- [~] Phase 3: Service replacement (admin/bot/sms moved; email/analytics pending).
- [~] Phase 4: Realtime replacement (service wired; contract validation pending).
- [ ] Phase 5: Node retirement and N-API removal.

## Acceptance Criteria (Per Phase)
- No gameplay regression in deterministic simulation test corpus.
- p95 latency non-inferior for core gameplay APIs.
- Error rate non-inferior under load tests.
- Contract compatibility checks pass (HTTP schema + websocket payload snapshots).
- Rollback path exists for each cutover wave.

## Risks and Controls
- Risk: semantic drift during porting.
  - Control: golden fixtures and replay tests for combat/fleet/economy.
- Risk: mixed-language operational complexity.
  - Control: short-lived coexistence, strict migration ownership, phase gates.
- Risk: provider integration regressions (Stripe/SMS/Email).
  - Control: adapter crates with contract tests and sandbox integration tests.

## Immediate Execution Plan (Next 2 Sprints)
1. Create expanded Rust workspace manifest and crate skeletons.
2. Extract `backend-core` domain code into `game-combat` and `game-fleet`.
3. Generate shared protobuf crate (`platform-proto`) and move contracts there.
4. Add compatibility test harness:
   - Compare Node vs Rust outputs for combat/fleet/economy fixtures.
5. Start `app-admin-api` as first full service rewrite target.

## Non-Goals During Migration
- Frontend redesign.
- Database replatforming away from PostgreSQL.
- Introducing new gameplay mechanics while parity migration is in-flight.
