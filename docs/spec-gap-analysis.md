# Spec Gap Analysis

## Completed highlights
- Rust backend now lives inside dedicated crates (`platform-tenancy`, `platform-consensus`, `platform-adapter`, `platform-migrations`, etc.) and is orchestrated via the workspace `Cargo.toml`.  
- Adapter registry exposes JSON, SQLite, Postgres, and MySQL drivers with migration logs, the new export/import helpers, and the migration-transfer CLI (`cargo run -p platform-migrations --bin migration-transfer`).  
- Integration tests cover API/route smoke flows, simulated player journeys, and the 1M-action benchmark; the JSON/SQLite dev flow and test harness are documented in `docs/json-dev-mode.md` and `specification/test-scenarios.md`.

## Gaps by concern

| Concern | Status | Remaining work |
| --- | --- | --- |
| **Adapter parity** | Partial | Postgres/MySQL adapters now log migrations and we ran test-container scripts, but we still need true production Postgres/MySQL smoke/integration coverage (migration transfer replay + consensus leasable logs) and a documented migration vector for SQL → SQL transitions. |
| **Migration tooling** | Partial | `MigrationTransfer` covers JSON/SQLite exports; the spec needs more depth on live SQL transfers and how admin/UI/CLI instruments consensus leases during a cross-adapter migration (`specification/test-scenarios.md` and `docs/json-dev-mode.md` outline the CLI). |
| **Consensus & tenant routing** | Partial | `platform-tenancy`, `platform-consensus`, `platform-sharding`, and `platform-tenant-routing` are wired, but we still need the lease contention/resilience tests, scheduler-worker runtime integration, and the tenant routing test matrix described in `docs/rust-backend-plan.md`. |
| **Tests & observability** | Partial | Additional consensus lease coverage, runtime leak/performance tests for `platform-worker-runtime`, and sharding/scheduler validation still await implementation; documentation should explicitly call out how to run those suites once available. |
| **Documentation consolidation** | Partial | Several focused docs exist (`docs/architecture.md`, `docs/json-dev-mode.md`, `specification/test-scenarios.md`), but a single canonical handbook that points operators to the adapter parity matrix, migration-transfer CLI, and outstanding stability/regression tests is still pending. This file aims to be that reference until a formal consolidation is published. |

## Next actions
1. Expand `docs/json-dev-mode.md` and `docs/architecture.md` with the new Postgres/MySQL parity tests and the migration-transfer CLI usage so operators can reproduce live exports/imports on SQL backends.  
2. Record the outstanding contractor work (consensus lease tests, `platform-worker-runtime` leak/performance coverage, scheduler/router validation) inside this analysis and `TODO.md` so nothing slips through the cracks.  
3. Once the above gaps are addressed, convert this document into the canonical “Rust backend status” page and retire the remaining Node-era guides (they are already archived under `docs/LEGACY_NODE_ARCHIVE.md`).  

For reference, the current partition plan and obligations are laid out in `specification/spec-rust-crate-partition.md` and `docs/rust-backend-plan.md`.
