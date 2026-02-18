# Rust Backend Reframe Plan

## Purpose
This document captures the current state of the Rust-native backend and the outstanding work that still keeps the legacy Node code (and the remaining operational complexity) in motion. The goal is to finish the migration by:

1. Partitioning the runtime into libraries/crates that encapsulate **tenancy**, **consensus**, **sharding**, **threading/runtime**, **adapter orchestration**, and **migrations**.
2. Saturating the system with tenant-aware checks, instrumentation, multi-database adapters, and consensus-backed leases so services stay safe under load, multi-tenancy, and failover.
3. Consolidating documentation, tests, and operational guidance (incl. the 1M action benchmark) to prove readiness for decommissioning any non-Rust surface.

## Crate Matrix
| Crate | Responsibility | Key Dependencies | Status |
| --- | --- | --- | --- |
| `platform-tenancy` | Tenant routing, context propagation, access levels, tenant health guards for Axum/Tower and queue workers | `platform-config`, `platform-auth`, `platform-observability` | core crate needs richer tenancy metadata and logging context |
| `platform-consensus` | Lease/arbitration service for shared resources (schedulers, shards, migrations) | `platform-db`, `platform-config`, `platform-observability` | in place; requires instrumented failover hooks and auto-expire policies |
| `platform-tenant-routing` *(planned)* | Maps tenant IDs to shard/queue pools, enforces rate limits/backpressure, ties into consensus leases, and exposes lifecycle hooks for tenant metadata changes | `platform-tenancy`, `platform-observability`, `platform-cache`, `platform-consensus` | new crate whose API returns `TenantRoutingDecision`s that include guard, route summary, optional lease token, and a permit ensuring per-tenant quotas |
| `platform-sharding` | Shard metadata + assignment, lease-aware shard leaders, and tenant placement info; feeds `platform-tenant-routing`/`platform-scheduler` | `platform-consensus`, `platform-db`, `platform-tenancy` | newly minted crate that tracks shard assignments, exposes tenant leases, and keeps workers aware of assigned nodes |
| `platform-scheduler` | Central scheduler owning cron metadata, job registration, backpressure, and retries; orchestrates `JobHandler`s with tenant routing + shard placement | `platform-consensus`, `platform-sharding`, `platform-tenant-routing`, `platform-events` | new crate coordinating job dispatch via guarded `TenantRoutingDecision`s |
| `platform-worker-runtime` | Thread/task pool foundations with graceful shutdown, leak counters, and tenant-aware instrumentation for every worker binary | `tokio`, `platform-observability`, `platform-config` | runtime helper now running tenant tasks with per-tenant stats/tests (`platform-worker-runtime::tests`) |
| `platform-adapter` | Unified adapter registry that loads JSON config, injects tenant context, acquires consensus leases, and reports adapter health | `platform-tenancy`, `adapter-db`, `platform-consensus` | implemented with `PlatformAdapterRegistry` wrapping `adapter-db` and reporting adapter definitions |
| `platform-migrations` | Tenant-aware migration runner surfaced through admin API/CLI for Postgres/MySQL/JSON | `platform-db`, `platform-consensus`, `platform-observability` | integrate `MigrationRunner` UI + CLI surfaces (admin, scripts) |
| `adapter-db` | Runtime-configurable DB adapters (Postgres, MySQL, JSON mock) | `platform-db`, `platform-config` | needs MySQL driver, JSON schema docs |
| `app-*` / `game-*` / `adapter-provider-*` | Business logic | pick relevant platform crates | ongoing cutover |


## Multi-Tenancy, Consensus & Threading
- **Tenant isolation**: Every HTTP request, queue message, and worker thread must derive its `TenantContext` from `platform-tenancy`. Logs/metrics must automatically include tenant identifiers to trace cross-service flows and to distinguish tenant lease state.
- **Lease-backed fail-safes**: `platform-consensus` must provide instrumentation for lease acquisition/release, automatically expire stale leaders, and propagate state to the scheduler/sharding stack so multi-tenant migrations/cron jobs pause gracefully during failover.
- **Sharding + routing**: `platform-sharding` now owns shard metadata and lease-backed leadership; it feeds `platform-tenant-routing` and the upcoming `platform-scheduler` with assignment information so tenant tasks stay on the right worker nodes, quotas are respected, and consensus interaction stays deterministic.
- **Threading & runtime stability**: `platform-worker-runtime` should be the runtime glue used by chat/notifications/email/analytics workers, ensuring graceful shutdown, leak detection, CPU/memory caps, and centralized instrumentation dashboards.
- **Adapter lifecycle**: `platform-adapter` will orchestrate JSON config loading, register Postgres/MySQL/JSON adapters, enforce tenant filters, and report adapter health to `platform-observability`. This prevents per-service ad-hoc adapter loading logic.

## Multi-Database Strategy
- Add documented JSON schema for `AdapterRegistry`:
  ```json
  [
    {
      "name": "tenant-a-postgres",
      "default_adapter": true,
      "driver": "postgres",
      "url": "...",
      "tenant": "tenant-a"
    }
  ]
  ```
- Support three adapters shipped by `adapter-db`: Postgres, MySQL, JSON file. Document how `platform-adapter` picks the right adapter per tenant and how to extend with new drivers.
- Ensure `app-*` services depend on `platform-adapter` to fetch DB/queue clients; remove any hardcoded connection strings from workers.

## Migration & Admin Surface
- `platform-migrations` must expose:
  - Tenant-scoped migration status (pending/applied/failed) via a REST endpoint in `app-admin-api`.
  - Migration run/rollback operations tied to `platform-consensus` leases, so only one runner manipulates a tenant’s schema at a time.
  - Admin CLI hooks (`scripts/rust/live-rust-cutover-check.ps1`, future cross-platform helper) that validate tenant migration health before marking a node ready.

## Tests and Observability
 - Expand testing matrix to include:
   1. Tenant isolation integration tests (Axum + queue guards) in `platform-tenancy`.
   2. Consensus lease contention/resilience tests in `platform-consensus`.
   3. Migration runner tests verifying transactional rollbacks per tenant in `platform-migrations`.
4. Multi-database adapter parity tests (Postgres/MySQL/JSON) under `adapter-db`.
5. Runtime leak/performance regression tests pumped through `platform-worker-runtime`.
6. Adapter health/lease tests (`platform-adapter`) that confirm JSON registry entries, per-tenant leases, and consensus guard behavior.
 - Add a dedicated `platform-tenant-routing` test harness that exercises:
   * Tenant isolation under shared executors so no tenant state bleeds between worker threads.
   * Queue pacing/backpressure (per-tenant RPS + concurrency) to ensure slow tenants do not starve others.
   * Consensus lease renewal/expiration transitions so routing halts or reroutes when leases fail.
   * Route decision tables that validate tenant-to-shard mapping rules and lifecycle event recomputations.
- The 1M-action benchmark (see `crates/benchmark-actions`) measures worker throughput and energy per loop while maintaining multi-tenant context.
- Observability dashboards must include tenant ID labels, lease states, adapter health, and worker runtime stats for failover analysis.

## Documentation & Migration Plan
- Consolidate the architecture notes, crate spec, and quick-start instructions into a single reference (this doc plus `docs/architecture.md` and updated TODO). Mark legacy Node docs as archived.
- Add `docs/rust-backend-plan.md` to cross-reference:
  - Crate matrix
  - Multi-tenancy & consensus requirements
  - Tests to ship
  - Migration/admin integration
- Ensure `docs/architecture.md` links back to this plan and describes the JSON adapter schema + admin endpoints.
- Track progress in `TODO.md` and `specification/spec-rust-crate-partition.md`.

## Next Steps
1. Finish wiring `platform-adapter`, document JSON configuration/resolved adapter metadata, and ensure all workers use the new `platform-tenant-routing`/`platform-sharding` decisions.
2. Implement `platform-scheduler` and `platform-worker-runtime` so we have uniform job dispatch and runtime tooling on top of the new crates.
3. Integrate `platform-migrations` into `app-admin-api` and surface migration status/controls.
4. Expand the test matrix (tenant isolation, consensus, migrations, adapters, routing, sharding) and document how to run/trust them.
5. Run the 1M-action benchmark regularly as part of load validation before each major cutover wave.
