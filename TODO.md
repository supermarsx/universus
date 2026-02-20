# TODO — Rust Backend Reframe

Status legend: [done], [partial], [missing]

## Multi-tenancy & consensus
- [partial] Ensure `platform-tenancy` injects tenant IDs/logging metadata into Axum/Tower/middleware and queue handlers; audit all HTTP routes to verify they read `TenantContext`.
- [partial] `platform-tenant-routing` maps tenant requests to worker pools with quotas/backpressure and optional leases; remaining work is end-to-end worker adoption and failover test automation.
- [partial] `platform-consensus` now emits lease lifecycle events/metrics and supports acquire/renew/release/status paths; scheduler/sharding workers now log consensus snapshots via `platform-observability`, with dashboard/alert plumbing still pending.
- [partial] `app-scheduler-worker` and `app-sharding-worker` now acquire cycle/task leases via `platform-consensus`; broader worker/runtime lease adoption remains.

## Adapter & multi-database strategy
- [done] `adapter-db` now wires Postgres, MySQL, and JSON file adapters and exposes `execute_script` for migration runners.
- [done] `platform-adapter` wraps `adapter-db`, honors the JSON registry (`database/runtime-adapters.json`), and reports adapter readiness/health through its definitions snapshot.
- [done] Documented how `platform-adapter` consumes JSON registry metadata (`driver`, `tenant`, `url/path`, `lease_resource_hint`, diagnostics tags) and maps lease hints into `platform-consensus` guards.

## Runtime & sharding platform
- [partial] `platform-sharding` now captures shard ownership, lease-backed leaders, and tenant placement; `app-sharding-worker` now syncs shard leader/catalog state, with deeper scheduler/runtime interoperability still pending.
- [partial] `platform-scheduler` now registers and triggers tenant jobs via `platform-tenant-routing`/`platform-sharding`, and `app-scheduler-worker` is wired into those APIs; next step is integrating worker-runtime execution around scheduler handlers.
- [partial] `platform-worker-runtime` now provides the shared runtime instrumentation for tenants; `app-notifications-worker`, `app-chat-worker`, `app-email-worker`, `app-analytics-worker`, and `app-bot-worker` are wired through it, with remaining workers still to migrate.
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
- [partial] Added `platform-worker-runtime` regression tests for leak-counter reset, backpressure (`MaxInflight`), and shutdown gating; extend with CPU/heap/lease-aware integration suites.
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
