# Rust Backend Reframe by Crate Boundaries

Updated: 2026-02-17

## Intent
Define the backend as a Rust-first system partitioned into platform, adapter, domain, and runtime crates so the remaining services, multi-tenancy work, and migration tooling can be completed without relying on legacy Node code.

## Crate Partition Matrix
| Crate | Responsibility | Dependencies | Notes |
| --- | --- | --- | --- |
| `platform-tenancy` | Tenant context + guards for HTTP/queue/worker paths | `platform-config`, `platform-auth`, `platform-observability` | Core crate that surfaces `TenantContext`, `TenantAccessLevel`, and tenant metadata for logging/metrics. Must emit tenant tags automatically. |
| `platform-consensus` | Lease/leader election guard before tenant touches shared resources (scheduler, shards, migrations) | `platform-db`, `platform-config`, `platform-observability` | Emits lease lifecycle metrics; new failover instrumentation planned. |
| `platform-tenant-routing` (new) | Maps tenants to shard/queue pools, enforces rate limits/backpressure, surfaces tenant lifecycle hooks | `platform-tenancy`, `platform-cache`, `platform-observability` | Keeps tenants isolated even when they share worker threads. |
| `platform-sharding` (new) | Tracks shard metadata, node assignment, and lease-backed shard leaders | `platform-consensus`, `platform-tenancy`, `platform-db` | Drives `app-sharding-worker` and tenant scheduling decisions. |
| `platform-scheduler` (new) | Job registration, cron metadata, tenant-aware retries, worker dispatch | `platform-sharding`, `platform-events`, `platform-consensus` | Will own scheduling instead of spreading logic across worker crates. |
| `platform-worker-runtime` (new) | Thread/task pool utilities, graceful shutdown, leak detection, instrumentation | `tokio`, `platform-observability`, `platform-config` | Standardizes the runtime experience of chat/notifications/email/analytics workers. |
| `platform-adapter` (new) | Unified adapter registry that injects tenant context, enforces consensus leases, exposes health | `platform-tenancy`, `adapter-db`, `platform-observability` | Wraps existing `adapter-db` + `adapter-provider-*` to centralize multi-database lifecycle. |
| `platform-migrations` | Tenant-aware migration runner surfaced via admin API/CLI | `platform-db`, `platform-consensus`, `platform-observability` | Requires REST/CLI integration and strong telemetry. |
| `adapter-db` | JSON-configured adapters (Postgres/MySQL/JSON) | `platform-db`, `platform-config` | Need to document JSON schema and add MySQL driver coverage. |
| `app-*` | HTTP/gateway/worker entrypoints (API, admin, bots, SMS, email, analytics, realtime, core, sharding, scheduler) | Platform + domain crates | Already in place; should be refactored to depend on the new runtime/adapter crates. |
| `game-*` | Domain logic (fleet, combat, moon, economy, etc.) | Platform crates | Pure domain logic that should remain free of runtime concerns.

## Dependency Rules
- App crates depend on domain, platform, and adapter crates.
- Domain crates depend only on platform crates (no app-level dependencies).
- Platform crates remain low-level and share primitives; new runtime/platform crates avoid depending on `app-*`.
- Adapter crates depend on platform crates (to reuse tenancy/auth/events instrumentation) but the platform cannot depend on adapters.

## Gaps & Priorities
1. **Adapter wiring** – Document the JSON schema for `AdapterRegistry` and add runtime discovery in `platform-adapter`. Each tenant should be able to swap between Postgres, MySQL, or JSON file storage without code changes.
2. **Consensus instrumentation** – `platform-consensus` needs observability hooks for lease health, auto-failover, and tenant-specific logs to prove the multi-tenancy boundaries.
3. **Scheduler/sharding runtime** – Implement `platform-tenant-routing`, `platform-sharding`, `platform-scheduler`, and `platform-worker-runtime` so the workers share consistent threading, tenant placement, and job scheduling semantics.
4. **Migration surface** – `platform-migrations` must integrate with `app-admin-api` (REST/CLI), expose tenant migration status, and guard runs using consensus leases plus instrumentation.
5. **Test matrix** – Add tenant isolation, adapter parity, consensus lease contention, migration rollback, and worker runtime leak tests to prove the system's readiness.

## Execution Plan
1. **Phase 0 – Foundation**: Finish wiring `platform-tenancy`, `platform-consensus`, `adapter-db`, and `platform-migrations` with updated docs and JSON schema plus instrumentation.
2. **Phase 1 – Runtime plumbing**: Build `platform-tenant-routing`, `platform-sharding`, `platform-scheduler`, `platform-worker-runtime`, and `platform-adapter`; update app/worker crates to consume these primitives.
3. **Phase 2 – Admin/migration/benchmarks**: Surface migration controls through `app-admin-api`, support multi-tenant migration orchestration, and capture the 1M-action benchmark (see `crates/benchmark-actions`) for throughput verification.
4. **Phase 3 – Cutover**: Remove the last Node documentation references, retire old `spec.md`/`spec-main.pdf` content, and rely entirely on the Rust crate matrix/outgoing docs.

Progress on each phase is tracked via `docs/rust-backend-plan.md`, `TODO.md`, and the validation reports under `specification/validation-reports/`.
