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

## Multi-Tenancy, Consensus, and Sharding
1. **Tenant routing**: Every HTTP/gRPC request and queue message derives its tenant from `platform-tenancy`. The planned `platform-tenant-routing` crate will map tenanted traffic to shard/worker pools, enforce quotas/backpressure, and surface tenant lifecycle hooks.
2. **Lease-backed resource guards**: `platform-consensus` acts as the gatekeeper for shared resources (schedulers, shard leaders, migration runners). Leases are time-bound, auto-renew, expose health metrics, and unblock failover when a lease expires.
3. **Sharding & scheduling**: The `platform-sharding` crate (backed by `platform-consensus`) now tracks shard ownership, leader assignment, and thread-level placement so workers (chat, notifications, analytics, etc.) know which shard/tenant they are allowed to process. The `platform-scheduler` crate now orchestrates cron jobs/tasks that follow those assignments while emitting tenant-aware leases and telemetry.
4. **Thread/runtime stability**: `platform-worker-runtime` will give each worker binary consistent graceful shutdown, leak detection, observability wiring, and memory/CPU caps to avoid stray nodes taking down the cluster under tenant stress.

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
- Lease transitions (acquire, renew, release, fail) should emit metrics that `platform-observability` collects, allowing auto-failover dashboards to trigger actions or alerts before a tenant loses access.
- Worker runtime instrumentation (thread counts, queue depth, blocking durations) plugs into `platform-worker-runtime` for consistent fail-safe wiring.
- Adapters must report health/readiness for each tenant driver so `platform-observability` can detect partial adapter outages (Postgres vs MySQL, etc.).

**Legacy documents:** Node-era guides live in `docs/LEGACY_NODE_ARCHIVE.md`; do not edit those files, and rely on the Rust docs listed above for current operations.

## TODO
1. Document the JSON schema and runtime discovery for `AdapterRegistry`, including how `platform-adapter` selects drivers per tenant and environment.
2. Integrate `MigrationRunner` into `app-admin-api` (REST + CLI) so admins can view tenant migration status, trigger runs/rollbacks, and read detailed telemetry on failures.
3. Capture the new migration-health gate in `scripts/rust/live-rust-cutover-check.ps1` so operators know the cutover script refuses to run smoke checks when any tenant reports failed migrations.
4. Implement the remaining runtime crates (`platform-scheduler`, `platform-worker-runtime`, `platform-adapter`) to standardize tenancy, threading, consensus, and adapter lifecycle; `platform-tenant-routing` and `platform-sharding` are already in place.
5. Document the `platform-tenant-routing` interface (route summaries, quota/per-tenant rate limits, optional lease acquisition) plus the test harness that validates tenant isolation, queue pacing/backpressure, consensus lease failures, and route decision recomputation.
6. Capture the tenant-routing-focused test cases described in `docs/rust-backend-plan.md` inside `specification/validation-reports/` once implemented.
7. Use `docs/spec-gap-analysis.md` as the current canonical state tracker so readers can see which adapter/migration/routing gaps remain before retiring the legacy Node surface.
8. Expand docs/tests for multi-tenancy, consensus, migrations, adapters, and the 1M action benchmark (see `crates/benchmark-actions`).

This page links back to `docs/rust-backend-plan.md`, which contains the cross-cutting plan for tests, docs, and benchmarks.
