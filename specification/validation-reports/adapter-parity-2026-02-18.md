# Adapter Parity Validation — 2026-02-18

## Objective
Capture the current status of `adapter-db` integration suites covering JSON, SQLite, Postgres, and MySQL drivers. These results feed into the Rust backend migration plan and signal operators when parity coverage shifts to the SQL stacks.

## Tests executed
- `cargo test -p adapter-db --test json_adapter` (JSON configuration sanity and registry routing).  
- `cargo test -p adapter-db --test sqlite_adapter` (SQLite scripting, JSON export/import migration transfer).  
- `cargo test -p adapter-db --test sql_adapters` (Postgres/MySQL containers via `testcontainers`; each run performs `execute_script`, logs the SQL, and confirms migration logs capture the schema changes).

## Notes
- The SQL tests rely on Docker; they skip gracefully when Docker access is unavailable but otherwise record per-tenant log files under `target/adapter-db/tests`.
- Each adapter under test writes header-prefixed migration logs (`-- migration <adapter> @ <epoch> --`) so the migration-transfer CLI (`platform-migrations` binary) can replay them across adapters.
- The new `docs/json-dev-mode.md`, `docs/architecture.md`, and `specification/test-scenarios.md` reference these suites to keep operators aligned with the Rust-only story.

## Outstanding validation
1. Run the SQL suites against production-quality Postgres/MySQL deployments (beyond Docker) to prove real-world latency, FIPS, and failover behaviors.  
2. Capture consensus lease telemetry whenever SQL tenants acquire adapters to ensure `platform-consensus` reports leasing health during migrations or scheduled work.  
3. Incorporate these tests into the eventual canonical status page so the `adapter parity` row shows “verified” once the outstanding validations pass.
