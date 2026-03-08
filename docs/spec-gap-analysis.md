# Spec Gap Analysis (2026-03-08)

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
- **All 16 previously-stub crates are now fully implemented** with real game logic and comprehensive test suites (433 total tests across the 16 crates):
  - Game domain layer: `game-domain`, `game-economy`, `game-galaxy`, `game-universe`, `game-alliance`, `game-moon`, `game-messaging`, `game-leaderboard`, `game-antiabuse`
  - Platform layer: `platform-auth`, `platform-cache`, `platform-common`, `platform-proto`
  - Adapter layer: `adapter-provider-payments`, `adapter-provider-bot`, `adapter-http-compat`
- CI workflow fixed: removed broken Node/pnpm/frontend steps, switched to `dtolnay/rust-toolchain`, consolidated caching.
- Root `package.json` (residual Node artifact) removed; `.env.example` created.

## High-confidence gaps

| Area | Current state | Gap |
| --- | --- | --- |
| Game crate integration into API gateway | All game domain/economy/galaxy/universe/alliance/moon/messaging/leaderboard/antiabuse crates are implemented with full test coverage. | The `app-api-gateway` `state.rs` (1615 lines) still contains inline game logic that should be replaced by calls into these crates. |
| Platform auth integration | `platform-auth` provides JWT, sessions, roles, password hashing. | API gateway and admin API still use ad-hoc auth; need to wire `platform-auth` into middleware. |
| Platform cache integration | `platform-cache` supports LRU/FIFO/TTL with two-level caching. | No hot-path reads (galaxy view, leaderboard, player profiles) use `platform-cache` yet. |
| Worker adoption of platform runtime stack | `app-scheduler-worker` and `app-sharding-worker` now execute cycle handlers through `platform-worker-runtime` with scheduler/sharding platform integration; `app-notifications-worker`, `app-chat-worker`, `app-email-worker`, `app-analytics-worker`, and `app-bot-worker` also run cycles/jobs via `platform-worker-runtime`. | The rest of the worker fleet still needs consistent `platform-worker-runtime` and platform routing/scheduler/sharding adoption. |
| Scheduler/sharding consensus integration | `platform-scheduler`/`platform-sharding` crates are ready; `app-scheduler-worker` now boots through scheduler+routing+sharding APIs, and `app-sharding-worker` now syncs shard leader/catalog state through `platform-sharding` plus cycle lease guards. | Remaining worker fleet still needs consistent lease/runtime adoption and sharding/scheduler end-to-end integration tests in live worker paths. |
| Consensus observability rollout | `platform-consensus` now exposes lifecycle metrics/events and renew/status methods, and scheduler/sharding workers emit snapshot logs through `platform-observability`. | Shared dashboards/alerts and broader service-level exports are still incomplete. |
| Worker runtime regression coverage | `platform-worker-runtime` now has unit tests plus regression tests for leak counter reset, max in-flight backpressure, shutdown gating, and leased task release semantics. | Extend into CPU/heap cap integration suites that include cross-worker failover behavior. |
| Tenant isolation validation depth | Route-level pieces exist across crates, and scheduler/tenant-routing lease failover is now covered in `platform-scheduler` tests. | End-to-end HTTP + queue isolation and reroute/failover automation across live workers remains incomplete. |
| Adapter operational readiness | SQL parity tests exist in `adapter-db`. | Production-grade runbooks and continuous operational checks for Postgres/MySQL behavior are still thin. |

## Documentation gaps
- ~~`TODO.md` status markers are stale in several places~~ — updated to reflect all completed stub implementations and CI fixes.
- ~~`docs/architecture.md` TODO section still describes crates as pending~~ — updated with full game/platform/adapter crate documentation.
- `specification/test-scenarios.md` exists, but command coverage for newer consensus/runtime suites should be kept in sync with actual executable tests.

## Priority order for next implementation
1. Integrate game domain crates into `app-api-gateway` — replace inline logic in `state.rs` with calls to `game-economy`, `game-galaxy`, `game-universe`, etc.
2. Wire `platform-auth` into API gateway and admin API middleware.
3. Connect `platform-cache` to hot-path reads (galaxy view, leaderboard, player profiles).
4. Wire worker binaries to platform runtime primitives (`platform-worker-runtime` + tenant routing + scheduler/sharding).
5. Add worker runtime leak/backpressure/performance test suites.
6. Extend consensus/tenant-routing tests from crate-level unit coverage into integration scenarios.

## Recently completed
- All 16 stub crates fully implemented with 433 tests (see `TODO.md` for per-crate details).
- CI workflow cleaned up (Node/frontend references removed, Rust toolchain fixed).
- Root `package.json` removed, `.env.example` created.
- `platform-adapter` build error fixed (pattern matching for `log_path` and `Sqlite` variant).
- `platform-consensus` lifecycle observability primitives and contention/resilience tests.
- `platform-worker-runtime` regression coverage expansion, scheduler/routing lease-failover regression.
- Runtime adoption in `app-notifications-worker`, `app-chat-worker`, and other workers via `platform-worker-runtime`.
