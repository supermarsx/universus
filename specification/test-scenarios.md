# Test Scenario Reference

## Purpose
Track the high-level coverage for the Rust-only backend so every integration, benchmark, and simulated journey is cataloged in one place. This document mirrors `docs/rust-backend-plan.md`, extends the `TODO.md` targets, and points operators toward the local JSON-backed startup flow described in `docs/json-dev-mode.md`.

## Integration Suites

| Scenario | Source | Coverage |
| --- | --- | --- |
| Health + fleet ecosystem smoke | `crates/app-api-gateway/tests/integration.rs` | Verifies `/health`, `/api/fleet`, `/api/fleet/:id` and error handling remain stable across changes. |
| Template guards & token flows | `crates/app-web-frontend/tests/integration.rs` | Ensures the web frontend renders public routes and protects `/buildings` & `/admin` behind the expected bearer tokens. |
| Simulated player journey | `crates/app-api-gateway/tests/simulated_flow.rs` | Executes a real-ish usage path: health, planet & fleet listings, tokenized constructions, fleet movement helpers, and a fleet send request including helper/movement calculations. |

## Simulated Flow Breakdown

1. **Health + catalog checks**: `GET /health` and `GET /api/planets` confirm the router wiring and `AppState` defaults.  
2. **Fleet readiness**: `GET /api/fleet` returns the seeded fleet summaries and proves the helpers respond even without a backing database.  
3. **Protected only path**: A tokenless `POST /api/planets/p-001/build` is rejected before the authorized request succeeds and returns a queue payload.  
4. **Helper computation**: `POST /api/fleet/helpers/movement` uses `game-fleet::FleetMovementInput` and asserts the derived distance/travel-time numbers.  
5. **Action submission**: `POST /api/fleet/send` with `Authorization: Bearer dev-token` exercises `AppState::enqueue_fleet_mission` and checks the generated command id plus `accepted` flag.  

This scenario is the easiest way to prove the API, helpers, and `AppState` interactions stay intact while also validating `Bearer`-protected routes.

## Local JSON / No-DB Mode

See `docs/json-dev-mode.md` for two operating tips:

1. Start any `app-*` crate via `cargo run -p <crate-name>` without setting `DATABASE_URL` — the routers fall back to the in-memory `AppState` and report healthy while ignoring `platform-db`.  
2. Use `database/runtime-adapters.json` to describe JSON adapters (`driver: "jsonfile"`) or the SQLite adapter (`driver: "sqlite"`) for tenants that need migrations or bookkeeping; `platform-adapter`/`platform-migrations` will honor that JSON registry and allow fully local multi-tenant simulations without Postgres/MySQL.

## Migration Transfer Testing

The new `platform-migrations` transfer binary (see `docs/json-dev-mode.md`) supports exporting a tenant’s script log, shipping it to another adapter, and replaying it inside the target while obeying the migration lease guard. Run the CLI against JSON/SQLite configs to smoke test migration workflows and compare whether the target adapter reports the expected tenant rows. The CLI also doubles as documentation for how to perform live migrations between any two adapters that expose `execute_script`.

## Benchmarks & Load

- `crates/benchmark-actions`: the 1M-action benchmark (see `specification/validation-reports/1m-action-benchmark.md`) continues to capture throughput, latency, and tenant-state pressure on the Rust worker runtime.

## Running the Tests

```bash
cargo test --test integration
cargo test --test simulated_flow
cargo test --test integration_simulated_flow --test-threads 1 # if you need deterministic log order
```

Append `-- --nocapture` when you need to inspect the emitted helper logs or `AppState` diagnostics.

## Outstanding Coverage

- Add consensus lease contention/resilience tests (see `TODO.md` and `platform-consensus` tests).  
- Compare Postgres/MySQL/JSON parity under `adapter-db` once the drivers are wired to real backends and collection scripts.  
- Expand platform-worker-runtime leak/performance tests and ensure `platform-tenant-routing` rerouting scenarios are codified (see `specification/spec-rust-crate-partition.md`).  
- Continue updating this document as the 1M-action benchmark, `docs/json-dev-mode`, and the migration/admin guides change.
