# TODO — Rust Backend Reframe

Status legend: [done], [partial], [missing]

## Multi-tenancy & consensus
- [partial] Ensure `platform-tenancy` injects tenant IDs/logging metadata into Axum/Tower/middleware and queue handlers; audit all HTTP routes to verify they read `TenantContext`.
- [missing] Implement `platform-tenant-routing` so tenant requests map to worker pools with quotas/backpressure and futures.
- [missing] Expand `platform-consensus` observability (lease acquire/renew/release, failover alerts) and document the lease lifecycle for operators.
- [partial] `app-sharding-worker` and `app-scheduler-worker` should acquire the required leases from `platform-consensus`.

## Adapter & multi-database strategy
- [done] `adapter-db` now wires Postgres, MySQL, and JSON file adapters and exposes `execute_script` for migration runners.
- [done] `platform-adapter` wraps `adapter-db`, honors the JSON registry (`database/runtime-adapters.json`), and reports adapter readiness/health through its definitions snapshot.
- [missing] Detail how `platform-adapter` consumes the JSON registry (driver metadata, optional `lease_resource_hint`, diagnostics tags) and how it maps those hints into `platform-consensus` leases so per-tenant adapters cannot be double-assigned.

## Runtime & sharding platform
- [partial] `platform-sharding` now captures shard ownership, lease-backed leaders, and tenant placement; it still needs integration with the scheduler/runtime hop.
- [partial] `platform-scheduler` now registers and triggers tenant jobs via `platform-tenant-routing`/`platform-sharding`; still needs integration with `platform-worker-runtime`.
- [partial] `platform-worker-runtime` now provides the shared runtime instrumentation for tenants; ensure workers routinely call it plus monitor instrumentation funnels.
- [partial] `platform-adapter` wraps `adapter-db`, loads JSON adapter configs, and gates each tenant within consensus leases; verify health hooks in dashboards/tests.

## Migrations & admin surface
- [done] `platform-migrations` now tracks per-tenant migration state, acquires consensus leases, and exposes `MigrationStatus` w/ lease metadata.
- [done] `app-admin-api` surfaces `/api/admin/tenants/{tenant_id}/migrations`, `/run`, and `/rollback` so operators can launch tenancy-safe migration runs.
- [missing] Update `scripts/rust/live-rust-cutover-check.ps1` to call migration endpoints and verify tenant health before smoke tests.

## Tests & benchmarks
- [partial] Tenant isolation and migration guard tests now cover the admin API flows and migration runner, but broader queue/HTTP isolation suites remain outstanding.
- [missing] Add lease contention/resilience tests targeting `platform-consensus`.
- [done] `platform-migrations` regression suite now covers rollback paths through `MigrationRunner`.
- [missing] Compare Postgres/MySQL/JSON behavior in `adapter-db` via parity tests (Postgres/MySQL drivers still need real backends).
- [missing] Add runtime leak/performance tests for `platform-worker-runtime`.
- [done] The 1M-action benchmark has a fresh run recorded under `specification/validation-reports/1m-action-benchmark.md`.

## Documentation & migration tracking
- [partial] `docs/architecture.md` now highlights the Rust platform plan and the new adapter/migration endpoints; the legacy Node docs still await archival.
- [partial] `specification/spec-rust-backend.md` and `spec-rust-crate-partition.md` continue to capture the crate layout; keep them synchronized with `docs/rust-backend-plan.md`.
- [done] Legacy Node-era docs are cataloged in `docs/LEGACY_NODE_ARCHIVE.md`, so updates should target the Rust documentation instead.
- [done] The 1M-action benchmark (see `specification/validation-reports/1m-action-benchmark.md`) now documents the latest run and stability notes.
