# Rust Infrastructure Architecture

## Purpose
This is the single-page overview of the Rust backend infrastructure, its current crates, and the remaining work that ties together the multi-tenancy, consensus, adapter, migration, and observability goals described in `docs/rust-backend-plan.md`.

## Crate Landscape
- `platform-tenancy`: propagates `TenantContext`, enforces `TenantAccessLevel`, and wraps Axum/Tower middleware plus queue handlers so every request or job knows which tenant it serves.
- `platform-consensus`: provides lease/leader election semantics for multi-tenant resources, ensuring sharding, scheduling, and migrations never run in parallel for the same tenant/region.
- `adapter-db`: runtime-configurable adapter registry; currently supports Postgres + JSON file drivers and is being extended with MySQL/JSON schema-backed options.
- `platform-migrations`: tenant-aware migration runner that will be exposed via the admin surface and CLI; it relies on `platform-consensus` to prevent concurrent schema changes.
- `platform-config`, `platform-observability`, `platform-db`, `platform-cache`, `platform-events`, `platform-auth`, `platform-errors`, `platform-proto`, `platform-common`: shared infra helpers for configuration, logging, telemetry, persistence, caches, pub/sub, authentication, and protobuf contracts.
- `app-*`, `game-*`, `adapter-provider-*`: the feature crates that depend on the platform layer to expose APIs, workers, and domain logic in Rust-only binaries.

## Multi-Tenancy, Consensus, and Sharding
1. **Tenant routing**: Every HTTP/gRPC request and queue message derives its tenant from `platform-tenancy`. The planned `platform-tenant-routing` crate will map tenanted traffic to shard/worker pools, enforce quotas/backpressure, and surface tenant lifecycle hooks.
2. **Lease-backed resource guards**: `platform-consensus` acts as the gatekeeper for shared resources (schedulers, shard leaders, migration runners). Leases are time-bound, auto-renew, expose health metrics, and unblock failover when a lease expires.
3. **Sharding & scheduling**: The new `platform-sharding` crate (backed by `platform-consensus`) will track shard ownership, leader assignment, and thread-level placement so workers (chat, notifications, analytics, etc.) know which shard/tenant they can process. The `platform-scheduler` crate will orchestrate cron jobs/tasks using that placement data.
4. **Thread/runtime stability**: `platform-worker-runtime` will give each worker binary consistent graceful shutdown, leak detection, observability wiring, and memory/CPU caps to avoid stray nodes taking down the cluster under tenant stress.

## Adapter Configuration and Multi-Database Support
- `adapter-db` is configured via JSON files, e.g.:
  ```json
  [
    { "name": "tenant-default", "driver": "postgres", "url": "postgres://...", "tenant": "default" },
    { "name": "tenant-staging", "driver": "mysql", "url": "mysql://...", "tenant": "staging" },
    { "name": "tenant-dev", "driver": "jsonfile", "path": "./tenant-dev.json", "tenant": "dev" }
  ]
  ```
- The `platform-adapter` crate (planned) will centralize adapter lifecycle, inject tenant context, enforce consensus guards, and expose health/readiness for each adapter.
- `adapter-db` must support Postgres, MySQL, and JSON file drivers; it should expose diagnostics (`trace_id`, `tenant`, `driver`) so dashboards can correlate to tenant/lease metrics.
- App-level crates should rely on `platform-adapter` rather than instantiating their own `tokio_postgres` clients. Each adapter registers itself with `platform-db`/`platform-config` via the shared registry.

## Migration Runner & Admin Integration
- `platform-migrations` is the tenant-aware migration runner. It must:
  1. Acquire a `platform-consensus` lease per tenant so migrations never collide.
  2. Expose status and control (run, rollback, skip) via new endpoints in `app-admin-api`.
  3. Hook into the live-cutover validation script (`scripts/rust/live-rust-cutover-check.ps1`) and CLI helpers so operators can assert that each tenant’s migrations succeeded before retiring Node services.
- Migration runs should be observable (logs + metrics) and annotated with the tenant/lease/shard metadata for easier post-mortem.

## Observability and Fail-Safe Processing
- Logging must automatically include tenant IDs, lease details, shard assignments, and worker runtime identifiers. `platform-observability` wires tracing/metrics across all platform and app crates, and these tags must propagate through `app-*` and worker crates.
- Lease transitions (acquire, renew, release, fail) should emit metrics that `platform-observability` collects, allowing auto-failover dashboards to trigger actions or alerts before a tenant loses access.
- Worker runtime instrumentation (thread counts, queue depth, blocking durations) plugs into `platform-worker-runtime` for consistent fail-safe wiring.
- Adapters must report health/readiness for each tenant driver so `platform-observability` can detect partial adapter outages (Postgres vs MySQL, etc.).

## TODO
1. Document the JSON schema and runtime discovery for `AdapterRegistry`, including how `platform-adapter` selects drivers per tenant and environment.
2. Integrate `MigrationRunner` into `app-admin-api` (REST + CLI) so admins can view tenant migration status, trigger runs/rollbacks, and read detailed telemetry on failures.
3. Update `scripts/rust/live-rust-cutover-check.ps1` to invoke migration health checks (per tenant) before other smoke tests.
4. Implement the planned crates (`platform-tenant-routing`, `platform-sharding`, `platform-scheduler`, `platform-worker-runtime`, `platform-adapter`) to standardize tenancy, threading, consensus, and adapter lifecycle.
5. Expand docs/tests for multi-tenancy, consensus, migrations, adapters, and the 1M action benchmark (see `crates/benchmark-actions`).

This page links back to `docs/rust-backend-plan.md`, which contains the cross-cutting plan for tests, docs, and benchmarks.
