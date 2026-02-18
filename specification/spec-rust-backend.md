# Universus Full Rust Backend Reframe Specification

Updated: 2026-02-17

## Status
- Rust-only runtime is the default deployment path described in `docker-compose.yml` (each service is reimplemented in `crates/app-*`, `crates/game-*`, and platform crates).
- Legacy Node/Express services (backend, admin-service, bot-service, SMS/email workers) are archived and no longer part of the runtime; the remaining surface is entirely hosted by the Rust workspace.
- Key platform crates (`platform-tenancy`, `platform-consensus`, `adapter-db`, `platform-migrations`, upcoming `platform-adapter`, `platform-tenant-routing`, and `platform-sharding`) drive the multi-tenancy/sharding/consensus story.
- Benchmarking & validation evidence are tracked under `specification/validation-reports/`, including the new 1M-action benchmark (`1m-action-benchmark.md`).

## Rust Coverage Snapshot
| Service family | Crate(s) | Runtime binary | Role |
| --- | --- | --- | --- |
| HTTP API | `app-api-gateway` | `rust-api-gateway` | All client-facing REST routes are handled here; uses `platform-auth`, `platform-tenancy`, `platform-adapter`, `platform-events`, and domain crates. |
| Admin | `app-admin-api` | `rust-admin-api` | Admin dashboards, monitoring status, migration controls, and tenancy management (futures). |
| Realtime | `app-realtime-gateway` | `rust-realtime-gateway` | WebSocket metadata, connection upgrades, and bridging to `platform-events` with tenant tagging. |
| SMS | `app-sms-api` | `rust-sms-api` | Outbound SMS/Telegram/Discord multi-channel API running on top of adapter/provider crates. |
| Email | `app-email-worker` | `rust-email-worker` | Queue consumer using Redis/adapter-provider-email while staying tenant aware. |
| Analytics | `app-analytics-worker` | `rust-analytics-worker` | RabbitMQ ingestion worker with `platform-consensus` leases for multi-tenant pipelines. |
| Core engine | `app-core-engine` | `rust-core-engine` | Replaces `backend-core`, exposing gRPC + HTTP helpers for combat/fleet logic. |
| Scheduler/sharder | `app-scheduler-worker`, `app-sharding-worker` | `rust-scheduler-worker`, `rust-sharding-worker` | Plan to rewire onto `platform-sharding` (newly stabilised), `platform-scheduler`, and `platform-worker-runtime` for consistent threading/backpressure. |

## Platform Pillars
- **Multi-tenancy**: `platform-tenancy` stores `TenantContext`, enforces `TenantAccessLevel`, and automatically enriches logs/metrics with tenant IDs for every HTTP/queue/worker path. Additional crates (`platform-tenant-routing`, `platform-sharding`, `platform-worker-runtime`) share this context to keep tenants isolated. `platform-tenant-routing` returns `TenantRoutingDecision`s that carry guard, route summary, per-tenant quota permits, and optional consensus leases so the router can throttle, tag telemetry, and guard shared resource access.
- **Worker runtime**: `platform-worker-runtime` now provides the shared executor for scheduler-driven jobs, wrapping each tenant task with instrumentation, leak detection, and graceful shutdown hooks so worker binaries stay observable and controllable.
- **Sharding metadata**: `platform-sharding` already tracks shard ownership, allowed tenants, and lease-backed leaders; it publishes assignment summaries for workers and feeds `platform-scheduler` so cron jobs stay pinned to valid shards.
- **Scheduling**: `platform-scheduler` now owns job registration plus dispatch logic; it consults `platform-tenant-routing` for quotas/leases, the `platform-sharding` catalog for placement, and forwards work to handlers that run under the upcoming `platform-worker-runtime`.
- **Sharding metadata**: `platform-sharding` already tracks shard ownership, allowed tenants, and lease-backed leaders; it publishes assignment summaries for workers and is ready to feed `platform-scheduler` and `platform-tenant-routing` with deterministic placement data.
- **Consensus**: `platform-consensus` coordinates leases for schedulers, shards, and migrations. Leases time out, auto-renew, and emit health metrics that feeds failover dashboards in `platform-observability`.
- **Adapter registry**: `adapter-db` plus the planned `platform-adapter` unify Postgres/MySQL/JSON adapters through a JSON configuration schema, ensuring adapters are tenant-aware, instrumented, and share a common lifecycle.
- **Migrations**: `platform-migrations` drives per-tenant schema changes. It must acquire a consensus lease, publish status through `app-admin-api`, and hook into `scripts/rust/live-rust-cutover-check.ps1` for validation before each cutover.
- **Observability**: `platform-observability` ensures tracing, metrics, and structured logging include tenant IDs, lease states, adapter health, and worker runtime stats (threads, queue depth, GC events).

## Adapter & Multi-Database Bridge
- Document the JSON schema for `AdapterRegistry` so operators can define per-tenant drivers:
  ```json
  [
    {
      "name": "tenant-eu-postgres",
      "driver": "postgres",
      "url": "postgres://...",
      "tenant": "tenant-eu"
    }
  ]
  ```
- `adapter-db` currently ships Postgres and JSON file adapters; MySQL support is on the roadmap. Each adapter surfaces `describe()`/`connection_info()` strings for dashboards.
- `platform-adapter` will wrap `adapter-db`, ensure adapters are only used after acquiring consensus leases, and push health/readiness through `platform-observability`.

## Migration & Admin Surface
- Tenant migrations emit telemetry (`tenant`, `migration_id`, `state`, `lease`) and are exposed through new `app-admin-api` endpoints: `/api/admin/tenants/{tenant_id}/migrations` (status/list) plus `/run`/`/rollback`.
- Admin CLI integrations (powershell scripts, future cross-platform helpers) must call these endpoints to verify success; the live-cutover validation script now runs per-tenant migration checks before other smoke tests.

## Benchmarking & Tests
- A 1M-action benchmark (`crates/benchmark-actions`) measures throughput while manipulating tenant contexts via `platform-tenancy`. Results are captured under `specification/validation-reports/1m-action-benchmark.md`.
- The regression matrix expands to include:
  1. Tenant isolation tests (`platform-tenancy`, `app-api-gateway`).
  2. Lease contention/resilience tests (`platform-consensus`).
  3. Migration rollbacks and locking (`platform-migrations` + `platform-consensus`).
  4. Adapter parity (Postgres/MySQL/JSON) and runtime health (`adapter-db`, `platform-adapter`).
  5. Worker runtime leak/performance tests (`platform-worker-runtime`).
  6. Targeted `platform-tenant-routing` scenarios that simulate tenant isolation on shared executors, enqueue pacing/backpressure, lease expiration failover, and route-decision lifecycle hooks.
  7. Shard ownership/leadership tests (`platform-sharding`) that verify tenant placement summaries, leader reassignments, and consensus-fueled routing under failure.
  8. Platform-adapter health/lease tests confirming JSON registry entries and per-tenant lease guards.

## Next Steps
1. Rewire app workers to consume the runtime platform crates—`platform-tenant-routing` and `platform-sharding` are already available, so finish wiring `platform-scheduler`, `platform-worker-runtime`, and `platform-adapter`, ensuring adapters have per-tenant leases before runtime tasks run.
2. Rewrite/retire legacy Node documentation (`spec.md`, `spec-main.pdf`, etc.) in favor of the Rust-first docs (`docs/rust-backend-plan.md`, `docs/architecture.md`).
3. Document tests, adapters, and migrations within `TODO.md` and `docs/rust-backend-plan.md` so every remaining action item is explicit.
4. Keep `specification/validation-reports/` up to date with benchmarks/validation (1M actions, migration runs, consensus failovers).
