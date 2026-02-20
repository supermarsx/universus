# TODO — Rust Backend Reframe

Status legend: [done], [partial], [missing]

## Multi-tenancy & consensus
- [partial] Ensure `platform-tenancy` injects tenant IDs/logging metadata into Axum/Tower/middleware and queue handlers; audit all HTTP routes to verify they read `TenantContext`.
- [partial] `platform-tenant-routing` maps tenant requests to worker pools with quotas/backpressure and optional leases; remaining work is end-to-end worker adoption and failover test automation.
- [partial] `platform-consensus` now emits lease lifecycle events/metrics and supports acquire/renew/release/status paths; remaining work is wiring these signals into shared observability dashboards/alerts.
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
- [done] Update `scripts/rust/live-rust-cutover-check.ps1` to call migration endpoints and verify tenant health before smoke tests.

## Tests & benchmarks
- [partial] Tenant isolation and migration guard tests now cover the admin API flows and migration runner, but broader queue/HTTP isolation suites remain outstanding.
- [partial] Lease contention/resilience tests now exist at the `platform-consensus` crate level; expand coverage into cross-crate integration scenarios with tenant routing/workers.
- [done] `platform-migrations` regression suite now covers rollback paths through `MigrationRunner`.
- [partial] Compare Postgres/MySQL/JSON behavior in `adapter-db` via parity tests (Postgres/MySQL drivers now have test-container coverage but production adapters still need operational validation).
- [partial] Added `platform-worker-runtime` regression tests for leak-counter reset, backpressure (`MaxInflight`), and shutdown gating; extend with CPU/heap/lease-aware integration suites.
- [done] The 1M-action benchmark has a fresh run recorded under `specification/validation-reports/1m-action-benchmark.md`.
- [partial] Ship the `platform-migrations` transfer CLI plus docs to show how to export/import tenants across adapters (JSON/SQLite and the upcoming SQL drivers); capture the new flow in testing docs and scripts.
- [done] Provide a consensus + worker runtime validation harness under `scripts/rust/run-consensus-worker-validation.ps1` and document its usage.
- [partial] `specification/test-scenarios.md` exists and tracks major suites; continue syncing it with new consensus/worker-runtime automation as tests land.
- [missing] Surface automation/scripts for firing up a Rust instance with only the JSON adapter (`database/runtime-adapters.json`) so smoke/integration suites run without external Postgres/MySQL dependencies.

## Documentation & migration tracking
- [partial] `docs/architecture.md` now highlights the Rust platform plan and the new adapter/migration endpoints; the legacy Node docs still await archival.
- [partial] `specification/spec-rust-backend.md` and `spec-rust-crate-partition.md` continue to capture the crate layout; keep them synchronized with `docs/rust-backend-plan.md`.
- [done] Legacy Node-era docs are cataloged in `docs/LEGACY_NODE_ARCHIVE.md`, so updates should target the Rust documentation instead.
- [done] The 1M-action benchmark (see `specification/validation-reports/1m-action-benchmark.md`) now documents the latest run and stability notes.
- [missing] Keep the Rust documentation/README references in sync with the new `specification/test-scenarios.md` so operators know where to find fresh integration/benchmark coverage; deprecate Node-era HOWTOs once the Rust docs are stable.
- [done] Documented the `platform-tenant-routing` interface and validation harness in `docs/tenant-routing.md`.
