# Rust Infrastructure Architecture

## Purpose
This is the single-page overview of the Rust backend infrastructure, its current crates, and the remaining work that ties together the multi-tenancy, consensus, adapter, migration, and observability goals described in `docs/rust-backend-plan.md`.

## Crate Landscape
- `platform-tenancy`: propagates `TenantContext`, enforces `TenantAccessLevel`, and wraps Axum/Tower middleware plus queue handlers so every request or job knows which tenant it serves.
- `platform-consensus`: provides lease/leader election semantics for multi-tenant resources, ensuring sharding, scheduling, and migrations never run in parallel for the same tenant/region.
- `platform-worker-runtime`: supplies the shared executor/leak instrumentation for chat/notifications/email/analytics workers, capturing tenant context, queue depth, and graceful shutdown gating.
- `platform-adapter`: wraps `adapter-db` with JSON configuration + per-tenant consensus lease guards, producing health metadata so each adapter is ready for instrumentation dashboards.
- `platform-sharding`: maintains shard metadata, leader assignments, and tenant placement data so workers and schedulers know which tenants/shards they serve; it relies on `platform-consensus` leases for guarded leadership.
  - `adapter-db`: runtime-configurable adapter registry; the Postgres adapter has basic wiring and the JSON file backend can load tenant dumps, but the MySQL branch currently still instantiates a `tokio_postgres` client and lacks a real `mysql_async`/pool-backed implementation.
    Documenting the adapter registry JSON schema (per-driver `driver`, `tenant`, `url`/`path`, plus optional `lease_resource_hint` or driver metadata that ties back to consensus guards) will make it possible to expose diagnostics and gate leases more reliably.
- `platform-migrations`: tenant-aware migration runner that will be exposed via the admin surface and CLI; it relies on `platform-consensus` to prevent concurrent schema changes.
- `platform-config`, `platform-observability`, `platform-db`, `platform-cache`, `platform-events`, `platform-auth`, `platform-errors`, `platform-proto`, `platform-common`: shared infra helpers for configuration, logging, telemetry, persistence, caches, pub/sub, authentication, and protobuf contracts.
- `app-*`, `game-*`, `adapter-provider-*`: the feature crates that depend on the platform layer to expose APIs, workers, and domain logic in Rust-only binaries.

## Game Domain & Logic Crates (fully implemented)
All 16 previously-stub crates are now fully implemented with real logic and comprehensive test suites (433 total tests):

### Game crates
- `game-domain`: Core types shared across the game layer — Resources, Coordinates, Player, Planet, 4 type enums (Building/Ship/Defense/Research, 56 variants total), FleetMission, queue items, FleetMovement, DebrisField, BattleReport, Message, UniverseSettings.
- `game-economy`: OGame-faithful production formulas, building/research/ship/defense costs with exponential scaling, construction times, storage capacity, lazy resource evaluation, and trade ratios.
- `game-galaxy`: Galaxy configuration, HashMap-backed GalaxyStore, coordinate validation, planet placement, debris field tracking, free position finding, and NPC generation with deterministic PRNG.
- `game-universe`: UniverseSettings with 15 configurable fields, UniverseStatus state machine (Creating→Online→Maintenance→Closed), UniverseManager with merge support, and 3 speed presets.
- `game-alliance`: Alliance CRUD, 6-tier AllianceRole with authority hierarchy, membership management, application lifecycle, and diplomacy pact system.
- `game-moon`: Moon creation from debris fields (OGame probability formula), moon destruction via RIP attacks, sensor phalanx range calculation, jump gate cooldown, and moon building costs.
- `game-messaging`: 8 MessageTypes, HashMap-backed MessageStore with send/inbox/read/archive/delete, combat/espionage report generation, spam guard with rate limiting, and bulk operations.
- `game-leaderboard`: 8 ScoreCategories, score calculation functions, PlayerRanking/AllianceRanking types, LeaderboardStore with rank recalculation, search, and history snapshots.
- `game-antiabuse`: Noob protection by points threshold, pushing detection, per-action rate limiting, IP/account monitoring, violation tracking with auto-ban thresholds, and behavioral analysis for bot detection.

### Platform crates
- `platform-auth`: Self-contained SHA-256 + HMAC-SHA256 JWT implementation (no external crypto deps), password hashing with salt, SessionStore with max sessions enforcement, role hierarchy (Player→SuperAdmin), and auth middleware helpers.
- `platform-cache`: Cache trait with InMemoryCache supporting LRU/FIFO/TTL eviction policies, TypedCache<C> serde wrapper, TwoLevelCache (L1/L2 with promotion), glob pattern matching for invalidation, and cache statistics.
- `platform-common`: ID generation, time utilities (ISO 8601 without chrono), validation (username/email/password/alliance tag), pagination helpers, string utilities (slugify, mask), math utilities, and environment helpers.
- `platform-proto`: ApiRequest/ApiResponse types, 18-variant GameEvent enum with tagged serde, WorkerTask/WorkerResult, RealtimeMessage for WebSocket, ServiceHealth, PageRequest/PageResponse, and serialization helpers.

### Adapter crates
- `adapter-provider-payments`: PaymentProvider trait, LoggingPaymentProvider mock, product catalog with 4 DM packages, webhook parsing/verification, and payment validation.
- `adapter-provider-bot`: 6 BotPersonality types (Rusher/Miner/Turtle/Raider/Researcher/Balanced), BotDecisionEngine AI with per-personality logic, BotScheduler, activity simulation, and fleet/trade decisions.
- `adapter-http-compat`: HttpCompatAdapter trait, LegacyCompatAdapter for Node.js→Rust translation, PathMapper, recursive camelCase↔snake_case key conversion, PassthroughAdapter, and API version detection.

## Multi-Tenancy, Consensus, and Sharding
1. **Tenant routing**: Every HTTP/gRPC request and queue message derives its tenant from `platform-tenancy`. The `platform-tenant-routing` crate maps tenanted traffic to shard/worker pools, enforces quotas/backpressure, surfaces tenant lifecycle hooks, and is now documented in `docs/tenant-routing.md`.
2. **Lease-backed resource guards**: `platform-consensus` acts as the gatekeeper for shared resources (schedulers, shard leaders, migration runners). Leases are time-bound, auto-renew, expose health metrics, and unblock failover when a lease expires.
3. **Sharding & scheduling**: The `platform-sharding` crate (backed by `platform-consensus`) now tracks shard ownership, leader assignment, and thread-level placement so workers (chat, notifications, analytics, etc.) know which shard/tenant they are allowed to process. The `platform-scheduler` crate now orchestrates cron jobs/tasks that follow those assignments while emitting tenant-aware leases and telemetry.
   - `app-scheduler-worker` now registers/executes its task families through `platform-scheduler` + `platform-tenant-routing` + `platform-sharding` bootstrap wiring, with handler execution pushed through `platform-worker-runtime`.
   - `app-sharding-worker` now synchronizes shard leader ownership and shard summary state through `platform-sharding` APIs during heartbeat cycles.
4. **Thread/runtime stability**: `platform-worker-runtime` provides shared graceful shutdown, leak counters, and instrumentation primitives; `app-scheduler-worker`, `app-sharding-worker`, `app-notifications-worker`, `app-chat-worker`, `app-email-worker`, `app-analytics-worker`, and `app-bot-worker` are wired, while the rest of the worker fleet still needs migration.

## Adapter Configuration and Multi-Database Support
- The adapter registry schema in `database/runtime-adapters.json` exposes per-driver metadata (`driver`, `tenant`, `url`/`path`, optional `logPath`, `lease_resource_hint`, and diagnostics tags). Operators can reference `adapter-db/src/lib.rs` alongside `docs/json-dev-mode.md` to see how `platform-adapter` and `platform-consensus` use those hints to offer per-tenant leases, health checks, and diagnostic breadcrumbs without double-assigning tenants.
- `adapter-db/tests` now holds dedicated integrations that spin up JSON, SQLite, Postgres, and MySQL adapters via `testcontainers` (when Docker is available) so we clearly signal how parity coverage is captured inside the Rust workspace.
- `adapter-db` is configured via `database/runtime-adapters.json`. For local dev we currently point the JSON adapter to `database/tenants/tenant-default.json`:
  ```json
  [
    {
      "name": "tenant-default-json",
      "driver": "jsonfile",
      "tenant": "tenant-default",
      "path": "database/tenants/tenant-default.json"
    }
  ]
  ```
- The `platform-adapter` crate already wraps `adapter-db`, centralizing adapter lifecycle, injecting tenant context, enforcing consensus guards, and exposing health/readiness metadata for each adapter via the shared registry.
- `adapter-db` exposes Postgres, MySQL, JSON file, and SQLite drivers plus diagnostics (`trace_id`, `tenant`, `driver`, `path`, `logPath`) so dashboards can correlate tenant/lease metrics across storage backends.
- App-level crates should rely on `platform-adapter` rather than instantiating their own `tokio_postgres` clients; the registry ensures every adapter is linked to `platform-consensus` leases and the same configuration metadata.
- The integration coverage in `specification/test-scenarios.md` and the JSON/SQLite-local workflow in `docs/json-dev-mode.md` explain how to verify adapter shards without hitting Postgres/MySQL and how to leverage the new CLI for exports/imports between adapters.

## Migration Runner & Admin Integration
- `platform-migrations` is the tenant-aware migration runner. It must:
  1. Acquire a `platform-consensus` lease per tenant so migrations never collide.
  2. Expose status and control (run, rollback, skip) via new endpoints in `app-admin-api`.
  3. Hook into the live-cutover validation script (`scripts/rust/live-rust-cutover-check.ps1`) and CLI helpers so operators can assert that each tenant’s migrations succeeded before retiring Node services.
- `scripts/rust/live-rust-cutover-check.ps1` now reads `database/runtime-adapters.json`, waits for the admin API, queries `/api/admin/tenants/{tenant_id}/migrations` before the smoke/cutover checks, and refuses to proceed if any tenant reports failed migrations.
- `app-admin-api` now exposes `/api/admin/tenants/{tenant_id}/migrations`, `/run`, and `/rollback` so operators can highlight tenant statuses and trigger `platform-migrations` actions from the dashboard or automation scripts.
- Migration runs should be observable (logs + metrics) and annotated with the tenant/lease/shard metadata for easier post-mortem.
- The new `cargo run -p platform-migrations --bin migration-transfer -- ...` binary exports a tenant’s script log and replays it on another adapter, giving ops a documented CLI for migrating between JSON/SQLite/Postgres backends before switching `database/runtime-adapters.json`.

## Observability and Fail-Safe Processing
- Logging must automatically include tenant IDs, lease details, shard assignments, and worker runtime identifiers. `platform-observability` wires tracing/metrics across all platform and app crates, and these tags must propagate through `app-*` and worker crates.
- Lease transitions (acquire, renew, release, fail) should emit metrics that `platform-observability` collects, allowing auto-failover dashboards to trigger actions or alerts before a tenant loses access; the required validation scenarios are captured in `docs/consensus-tests.md`.
- `platform-observability` now exposes a consensus snapshot emitter used by scheduler/sharding workers so lease metrics/events are emitted into runtime logs each cycle.
- Worker runtime instrumentation (thread counts, queue depth, blocking durations) plugs into `platform-worker-runtime` for consistent fail-safe wiring and is documented under `docs/worker-runtime-tests.md` so operators can reproduce the leak/performance suites.
- Use `scripts/rust/run-consensus-worker-validation.ps1` to execute the consensus and worker runtime automation along with the optional adapter parity suite; pass `-NoDocker` when running on machines without Docker.
- Adapters must report health/readiness for each tenant driver so `platform-observability` can detect partial adapter outages (Postgres vs MySQL, etc.).

**Legacy documents:** Node-era guides live in `docs/LEGACY_NODE_ARCHIVE.md`; do not edit those files, and rely on the Rust docs listed above for current operations.

## TODO
1. Migrate remaining workers (`app-email-worker`, `app-analytics-worker`, `app-bot-worker`, and others) onto `platform-worker-runtime`.
2. Wire `app-scheduler-worker`/`app-sharding-worker` into `platform-scheduler` and `platform-sharding` decision APIs rather than mostly app-local loops.
3. Export `platform-consensus` lifecycle metrics/events through shared `platform-observability` dashboards/alerts.
4. Expand `platform-worker-runtime` coverage with CPU/heap cap and lease-aware integration tests.
5. Add end-to-end tenant-routing/consensus scenarios (HTTP isolation, queue failover, migration lock handshake) beyond crate-level tests.
6. Integrate the new game domain/economy/galaxy/universe crates into `app-api-gateway` routes, replacing inline logic in `state.rs`.
7. Wire `platform-auth` JWT/session management into the API gateway and admin API middleware.
8. Connect `platform-cache` to hot-path reads (galaxy view, leaderboard, player profiles) in the API gateway.
9. Keep `TODO.md`, `docs/spec-gap-analysis.md`, and `specification/test-scenarios.md` synchronized as those suites land.

This page links back to `docs/rust-backend-plan.md`, which contains the cross-cutting plan for tests, docs, and benchmarks.
