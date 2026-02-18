# Spec Gap Analysis

## Completed highlights
- Rust backend now lives inside dedicated crates (`platform-tenancy`, `platform-consensus`, `platform-adapter`, `platform-migrations`, etc.) and is orchestrated via the workspace `Cargo.toml`.  
- Adapter registry exposes JSON, SQLite, Postgres, and MySQL drivers with migration logs, the new export/import helpers, and the migration-transfer CLI (`cargo run -p platform-migrations --bin migration-transfer`).  
- Integration tests cover API/route smoke flows, simulated player journeys, adapter-db parity suites (JSON/SQLite/Postgres/MySQL), and the 1M-action benchmark; the JSON/SQLite dev flow, adapter testing guidance, and test harness are documented in `docs/json-dev-mode.md`, `docs/architecture.md`, and `specification/test-scenarios.md`.

## Gaps by concern

| Concern | Status | Remaining work |
| --- | --- | --- |
| **Adapter parity** | Partial | Postgres/MySQL adapters now log migrations, parity suites can start Postgres/MySQL containers, and the migration-transfer CLI captures per-tenant logs; we still need full production validation of those adapters, cross-SQL migration runbooks, and consensus lease telemetry when SQL tenants dominate. |
| **Migration tooling** | Partial | `MigrationTransfer` covers JSON/SQLite exports; the spec still needs more depth on live SQL transfers and how admin/UI/CLI instruments consensus leases during cross-adapter migrations (`specification/test-scenarios.md` and `docs/json-dev-mode.md` outline the CLI). |
| **Consensus & tenant routing** | Partial | `platform-tenancy`, `platform-consensus`, `platform-sharding`, and `platform-tenant-routing` are wired, but we still need lease contention/resilience tests, scheduler-worker runtime integration, and the tenant routing test matrix described in `docs/rust-backend-plan.md`. |
| **Tests & observability** | Partial | Additional consensus lease coverage, runtime leak/performance tests for `platform-worker-runtime`, and sharding/scheduler validations still await implementation; documentation should explicitly call out how to run those suites once available. |
| **Documentation consolidation** | Partial | Multiple docs exist (`docs/architecture.md`, `docs/json-dev-mode.md`, `specification/test-scenarios.md`), but a single canonical handbook referencing the adapter parity matrix, migration-transfer CLI, and outstanding regression tests still needs publication; this file should become that reference when the remaining gaps close. |

## Next actions
1. Keep `docs/json-dev-mode.md`, `docs/architecture.md`, `docs/tenant-routing.md`, `docs/consensus-tests.md`, `docs/worker-runtime-tests.md`, and `specification/test-scenarios.md` synchronized with the parity suites, migration-transfer CLI, tenant-routing validation, lease contention/resilience guides, and worker runtime leak/performance coverage so operators can reproduce the documented flows without Docker gaps.  
2. Record the remaining contractor work (consensus lease contention suites, `platform-worker-runtime` leak/performance coverage, scheduler/router validation, adapter diagnostics) inside this analysis and `TODO.md` so nothing slips through the cracks.  
3. Determine when to promote this file into the canonical “Rust backend status” page, retire duplicate Node-era guides, and point readers at the consolidated docs once the runtime crates/tests finish landing.  

For reference, the current partition plan and obligations are laid out in `specification/spec-rust-crate-partition.md` and `docs/rust-backend-plan.md`.
