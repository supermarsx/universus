# TODO — Rust Backend Reframe

Status legend: [done], [partial], [missing]

## Multi-tenancy & consensus
- [partial] Ensure `platform-tenancy` injects tenant IDs/logging metadata into Axum/Tower/middleware and queue handlers; audit all HTTP routes to verify they read `TenantContext`.
- [missing] Implement `platform-tenant-routing` so tenant requests map to worker pools with quotas/backpressure and futures.
- [missing] Expand `platform-consensus` observability (lease acquire/renew/release, failover alerts) and document the lease lifecycle for operators.
- [partial] `app-sharding-worker` and `app-scheduler-worker` should acquire the required leases from `platform-consensus`.

## Adapter & multi-database strategy
- [partial] `adapter-db` bootsstraps Postgres + JSON file adapters; the JSON schema and documentation still need clarity.
- [missing] Add MySQL adapter implementation to `adapter-db` plus advisory guidance on connection pooling/backpressure.
- [missing] Build `platform-adapter` to wrap `adapter-db`, inject tenant context, and expose per-adapter readiness/health for `platform-observability`.

## Runtime & sharding platform
- [partial] `platform-sharding` now captures shard ownership, lease-backed leaders, and tenant placement; it still needs integration with the scheduler/runtime hop.
- [partial] `platform-scheduler` now registers and triggers tenant jobs via `platform-tenant-routing`/`platform-sharding`; still needs integration with `platform-worker-runtime`.
- [partial] `platform-worker-runtime` now provides the shared runtime instrumentation for tenants; ensure workers routinely call it plus monitor instrumentation funnels.
- [partial] `platform-adapter` wraps `adapter-db`, loads JSON adapter configs, and gates each tenant within consensus leases; verify health hooks in dashboards/tests.

## Migrations & admin surface
- [partial] `platform-migrations` exists but needs a documented JSON config + CLI endpoints.
- [missing] Add REST/CLI endpoints in `app-admin-api` to list tenant migration status, run new migrations, roll back, and inspect history.
- [missing] Update `scripts/rust/live-rust-cutover-check.ps1` to call migration endpoints and verify tenant health before smoke tests.

## Tests & benchmarks
- [missing] Add tenant isolation integration tests covering HTTP, queue workers, and multi-tenant guard failure paths.
- [missing] Add lease contention/resilience tests targeting `platform-consensus`.
- [missing] Add migration rollback/race tests to `platform-migrations`.
- [missing] Compare Postgres/MySQL/JSON behavior in `adapter-db` via parity tests.
- [missing] Add runtime leak/performance tests for `platform-worker-runtime`.
- [partial] The new 1M-action benchmark resides in `crates/benchmark-actions`; capture and share results under `specification/validation-reports/1m-action-benchmark.md`.

## Documentation & migration tracking
- [partial] `docs/architecture.md` now points to the new platform plan, but the legacy `spec.md`/`spec-main.pdf` still need archiving or rewriting.
- [partial] `specification/spec-rust-backend.md` and `spec-rust-crate-partition.md` describe the plan; keep them aligned with `docs/rust-backend-plan.md`.
- [missing] Consolidate all Node-era docs (deployment guides, phase playbooks, quick references) into the Rust-focused doc set and mark the old documents as archived.
- [missing] Document the 1M-action benchmark process plus consensus/migration validation reports under `specification/validation-reports/`.
