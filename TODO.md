# TODO — Rust Backend Reframe

Status legend: [done], [partial], [missing]

## Domain & game logic crates (stub → full implementation)
- [done] `game-domain` — core types: Resources, Coordinates, Player, Planet, Building/Ship/Defense/Research enums (56 variants), FleetMission, queue items, FleetMovement, DebrisField, BattleReport, Message, UniverseSettings. (56 tests)
- [done] `game-economy` — OGame production formulas, building/research/ship/defense costs, construction times, storage capacity, lazy resource evaluation, trade ratios. (41 tests)
- [done] `game-galaxy` — GalaxyConfig, GalaxyStore (HashMap-backed), coordinate validation, planet placement, debris fields, free position finding, NPC generation. (18 tests)
- [done] `game-universe` — UniverseSettings, UniverseStatus state machine, UniverseManager, merge support, presets. (26 tests)
- [done] `game-alliance` — Alliance CRUD, role hierarchy, membership, applications, diplomacy pacts. (24 tests)
- [done] `game-moon` — Moon creation from debris (OGame formula), RIP destruction, sensor phalanx, jump gate cooldown. (32 tests)
- [done] `game-messaging` — 8 MessageTypes, MessageStore, combat/espionage reports, spam guard, bulk operations. (27 tests)
- [done] `game-leaderboard` — 8 ScoreCategories, ranking stores, recalculation, search, history snapshots. (20 tests)
- [done] `game-antiabuse` — Noob protection, pushing detection, rate limiting, IP/account monitoring, violation tracking, bot detection. (26 tests)

## Platform crates (stub → full implementation)
- [done] `platform-auth` — SHA-256/HMAC-SHA256 JWT, password hashing, SessionStore, role hierarchy, auth middleware helpers. (25 tests)
- [done] `platform-cache` — Cache trait, InMemoryCache (LRU/FIFO/TTL), TypedCache, TwoLevelCache, glob patterns, stats. (27 tests)
- [done] `platform-common` — ID generation, time utilities, validation, pagination, string/math utils, env helpers. (42+ tests)
- [done] `platform-proto` — ApiRequest/Response, 18-variant GameEvent enum, WorkerTask/Result, RealtimeMessage, ServiceHealth, pagination. (18 tests)

## Adapter crates (stub → full implementation)
- [done] `adapter-provider-payments` — PaymentProvider trait, LoggingPaymentProvider, product catalog, webhook parsing/verification. (32 tests)
- [done] `adapter-provider-bot` — 6 BotPersonality types, BotDecisionEngine AI, BotScheduler, activity simulation. (25 tests)
- [done] `adapter-http-compat` — HttpCompatAdapter trait, LegacyCompatAdapter, PathMapper, camelCase/snake_case conversion. (26 tests)

## CI & cleanup
- [done] Fixed pre-existing build error in `platform-adapter` (pattern matching for `log_path` field and `Sqlite` variant).
- [done] Fixed `.github/workflows/ci.yml` — removed broken Node/pnpm/frontend steps, switched to `dtolnay/rust-toolchain`, consolidated cache steps.
- [done] Removed residual root `package.json` (Node.js artifact from pre-migration era).
- [done] Created `.env.example` (was referenced by README but missing).

## Multi-tenancy & consensus
- [partial] Ensure `platform-tenancy` injects tenant IDs/logging metadata into Axum/Tower/middleware and queue handlers; audit all HTTP routes to verify they read `TenantContext`.
- [partial] `platform-tenant-routing` maps tenant requests to worker pools with quotas/backpressure and optional leases; remaining work is end-to-end worker adoption and failover test automation.
- [partial] `platform-consensus` now emits lease lifecycle events/metrics and supports acquire/renew/release/status paths; scheduler/sharding workers now log consensus snapshots via `platform-observability`, with dashboard/alert plumbing still pending.
- [partial] `app-scheduler-worker` and `app-sharding-worker` now acquire cycle/task leases via `platform-consensus`; broader worker/runtime lease adoption remains.

## Adapter & multi-database strategy
- [done] `adapter-db` now wires Postgres, MySQL, JSON file, and SQLite adapters and exposes `execute_script` for migration runners.
- [done] `platform-adapter` wraps `adapter-db`, honors the JSON registry (`database/runtime-adapters.json`), and reports adapter readiness/health through its definitions snapshot. Now handles all adapter drivers including SQLite.
- [done] Documented how `platform-adapter` consumes JSON registry metadata (`driver`, `tenant`, `url/path`, `lease_resource_hint`, diagnostics tags) and maps lease hints into `platform-consensus` guards.
- [done] `adapter-http-compat` now provides `HttpCompatAdapter` trait with `SnakeToCamelAdapter`, `CamelToSnakeAdapter`, and `PassthroughAdapter` implementations for JSON key format conversion.
- [done] `adapter-provider-bot` now provides `BotProviderAdapter` with bot registration, webhook management, and event logging.
- [done] `adapter-provider-payments` now provides `PaymentsProviderAdapter` with full transaction lifecycle (create, complete, fail, refund) and revenue tracking.

## Runtime & sharding platform
- [partial] `platform-sharding` now captures shard ownership, lease-backed leaders, and tenant placement; `app-sharding-worker` now syncs shard leader/catalog state, with deeper scheduler/runtime interoperability still pending.
- [partial] `platform-scheduler` now registers and triggers tenant jobs via `platform-tenant-routing`/`platform-sharding`, and `app-scheduler-worker` now runs those handlers through `platform-worker-runtime`; next step is broader end-to-end scheduler/shard integration coverage.
- [partial] `platform-worker-runtime` now provides the shared runtime instrumentation for tenants; `app-scheduler-worker`, `app-sharding-worker`, `app-notifications-worker`, `app-chat-worker`, `app-email-worker`, `app-analytics-worker`, and `app-bot-worker` are wired through it, with remaining workers still to migrate.
- [partial] `platform-adapter` wraps `adapter-db`, loads JSON adapter configs, and gates each tenant within consensus leases; verify health hooks in dashboards/tests.

## Migrations & admin surface
- [done] `platform-migrations` now tracks per-tenant migration state, acquires consensus leases, and exposes `MigrationStatus` w/ lease metadata.
- [done] `app-admin-api` surfaces `/api/admin/tenants/{tenant_id}/migrations`, `/run`, and `/rollback` so operators can launch tenancy-safe migration runs.
- [done] Update `scripts/rust/live-rust-cutover-check.ps1` to call migration endpoints and verify tenant health before smoke tests.

## Tests & benchmarks
- [partial] Tenant isolation and migration guard tests now cover the admin API flows and migration runner, but broader queue/HTTP isolation suites remain outstanding.
- [partial] Lease contention/resilience tests now exist at the `platform-consensus` crate level, plus scheduler/tenant-routing failover coverage in `platform-scheduler`; expand further into end-to-end worker integration scenarios.
- [done] `platform-migrations` regression suite now covers rollback paths through `MigrationRunner`.
- [partial] Compare Postgres/MySQL/JSON behavior in `adapter-db` via parity tests (Postgres/MySQL drivers now have test-container coverage but production adapters still need operational validation).
- [partial] Added `platform-worker-runtime` regression tests for leak-counter reset, backpressure (`MaxInflight`), shutdown gating, and leased-task release semantics; extend with CPU/heap integration suites.
- [done] The 1M-action benchmark has a fresh run recorded under `specification/validation-reports/1m-action-benchmark.md`.
- [partial] Ship the `platform-migrations` transfer CLI plus docs to show how to export/import tenants across adapters (JSON/SQLite and the upcoming SQL drivers); capture the new flow in testing docs and scripts.
- [done] Provide a consensus + worker runtime validation harness under `scripts/rust/run-consensus-worker-validation.ps1` and document its usage.
- [partial] `specification/test-scenarios.md` exists and tracks major suites; continue syncing it with new consensus/worker-runtime automation as tests land.
- [partial] Rust startup automation exists (`scripts/rust/start-rust-only.ps1`) and JSON-only local crate flows are documented; still missing a dedicated one-command JSON-only smoke harness that avoids Postgres/Redis/RabbitMQ bring-up.

## Documentation & migration tracking
- [partial] `docs/architecture.md` now highlights the Rust platform plan and the new adapter/migration endpoints; the legacy Node docs still await archival.
- [partial] `specification/spec-rust-backend.md` and `spec-rust-crate-partition.md` continue to capture the crate layout; keep them synchronized with `docs/rust-backend-plan.md`.
- [done] Legacy Node-era docs are cataloged in `docs/LEGACY_NODE_ARCHIVE.md`, so updates should target the Rust documentation instead.
- [done] The 1M-action benchmark (see `specification/validation-reports/1m-action-benchmark.md`) now documents the latest run and stability notes.
- [missing] Keep the Rust documentation/README references in sync with the new `specification/test-scenarios.md` so operators know where to find fresh integration/benchmark coverage; deprecate Node-era HOWTOs once the Rust docs are stable.
- [done] Documented the `platform-tenant-routing` interface and validation harness in `docs/tenant-routing.md`.
