# Spec Gap Analysis (2026-02-20)

## Scope
This analysis is based on the current repository state across:
- `crates/platform-*` and `crates/app-*` implementations
- active test suites under `crates/*/tests` and crate-level unit tests
- live operational docs in `docs/` and `specification/`

## What is clearly implemented
- Core platform crates for tenancy/routing/sharding/scheduler/runtime/adapter/migrations exist and compile in the workspace.
- `platform-tenant-routing` enforces per-tenant quota/rate semantics and optional consensus lease acquisition in `crates/platform-tenant-routing/src/lib.rs`.
- `platform-scheduler` and `platform-sharding` are implemented as reusable crates with tests in `crates/platform-scheduler/src/lib.rs` and `crates/platform-sharding/src/lib.rs`.
- `platform-adapter` and `adapter-db` support JSON/SQLite/Postgres/MySQL registry flows; parity tests exist under `crates/adapter-db/tests`.
- Migration APIs and transfer tooling exist (`app-admin-api` migration endpoints + `platform-migrations` transfer CLI).
- As of this update, `platform-consensus` now includes lease lifecycle metrics/events plus contention/renewal/release tests in `crates/platform-consensus/src/lib.rs`.

## High-confidence gaps

| Area | Current state | Gap |
| --- | --- | --- |
| Worker adoption of platform runtime stack | `app-scheduler-worker` and `app-sharding-worker` now lease-guard cycles; `app-notifications-worker`, `app-chat-worker`, `app-email-worker`, `app-analytics-worker`, and `app-bot-worker` run cycles/jobs via `platform-worker-runtime`. | The rest of the worker fleet still needs consistent `platform-worker-runtime` and platform routing/scheduler/sharding adoption. |
| Scheduler/sharding consensus integration | `platform-scheduler`/`platform-sharding` crates are ready; `app-scheduler-worker` now boots through scheduler+routing+sharding APIs, and `app-sharding-worker` now syncs shard leader/catalog state through `platform-sharding` plus cycle lease guards. | Remaining worker fleet still needs consistent lease/runtime adoption and sharding/scheduler end-to-end integration tests in live worker paths. |
| Consensus observability rollout | `platform-consensus` now exposes lifecycle metrics/events and renew/status methods, and scheduler/sharding workers emit snapshot logs through `platform-observability`. | Shared dashboards/alerts and broader service-level exports are still incomplete. |
| Worker runtime regression coverage | `platform-worker-runtime` now has unit tests plus regression tests for leak counter reset, max in-flight backpressure, shutdown gating, and leased task release semantics. | Extend into CPU/heap cap integration suites that include cross-worker failover behavior. |
| Tenant isolation validation depth | Route-level pieces exist across crates, and scheduler/tenant-routing lease failover is now covered in `platform-scheduler` tests. | End-to-end HTTP + queue isolation and reroute/failover automation across live workers remains incomplete. |
| Adapter operational readiness | SQL parity tests exist in `adapter-db`. | Production-grade runbooks and continuous operational checks for Postgres/MySQL behavior are still thin. |

## Documentation gaps
- `TODO.md` status markers are stale in several places (some items marked `[missing]` are already implemented).
- `docs/architecture.md` TODO section still describes crates as pending even when they now exist.
- `specification/test-scenarios.md` exists, but command coverage for newer consensus/runtime suites should be kept in sync with actual executable tests.

## Priority order for implementation
1. Wire worker binaries to platform runtime primitives (`platform-worker-runtime` + tenant routing + scheduler/sharding).
2. Add worker runtime leak/backpressure/performance test suites.
3. Extend consensus/tenant-routing tests from crate-level unit coverage into integration scenarios.
4. Align docs and TODO status with current code ownership and executable test commands.

## Immediate follow-up in progress
- Implemented in this pass: `platform-consensus` lifecycle observability primitives and contention/resilience tests.
- Implemented after that pass: `platform-worker-runtime` regression coverage expansion, scheduler/routing lease-failover regression, and runtime adoption in `app-notifications-worker` + `app-chat-worker`.
