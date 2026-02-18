# JSON-Only Local Mode

## Purpose
Run the Rust services locally without requiring Postgres, MySQL, or any other formal database. The built-in `AppState` defaults (used by the API gateway, admin API, and frontend) plus the adapter registry make it possible to validate the UI, web routes, and helper functions while keeping everything contained inside the repo. This page points to the quick-start commands, the SQLite/JSON adapters, and the test scenarios that rely on this mode.

## Useful Commands

| Scenario | Command |
| --- | --- |
| Run the API gateway (uses in-memory `AppState`) | `API_GATEWAY_TOKEN=dev-token cargo run -p app-api-gateway` |
| Run the admin UI | `cargo run -p app-admin-api` |
| Serve the web frontend | `cargo run -p app-web-frontend` |
| Launch multiple services (API + frontend + admin) | Run each of the commands above in separate terminals simultaneously. |

Ports and service-specific envs (like `PORT` for the frontend) follow the defaults in each crate’s `main.rs`. Since no `DATABASE_URL` value is required, every data-driven handler falls back to the seeded `AppState` and the helpers already exercise `game-fleet`/`game-combat`.

## Adapter & Migration Config

1. `platform-adapter` and `adapter-db` read `database/runtime-adapters.json`. For local work you can use either the JSON driver or the new SQLite driver. Example JSON adapter entry:
   ```json
   [
     {
       "name": "tenant-default-json",
       "driver": "jsonfile",
       "tenant": "tenant-default",
       "path": "database/tenants/tenant-default.json"
     }
   ]
   ```
2. The JSON adapter writes migration output to the configured path and can be used by `platform-migrations` when you run `cargo run -p platform-migrations` or the `scripts/rust/live-rust-cutover-check.ps1` script. No Postgres/MySQL service is needed.
3. The SQLite adapter now surfaces the same metadata (`name`, `tenant`, `info`) while storing migrations in a local `.sqlite3` file. Add entries like this to share the same registry:
   ```json
   {
     "name": "tenant-local-sqlite",
     "driver": "sqlite",
     "tenant": "tenant-local",
     "path": "database/tenants/tenant-local.sqlite3"
   }
   ```
   Running `cargo run -p platform-migrations` or the live Rust bring-up scripts will open the file and execute each migration string inside the adapter-driver guard.

## Observing Tests & Simulated Use

- See `specification/test-scenarios.md` for the catalog of integration tests (including the simulated player flow) that you can run while local services are up.  
- The simulated flow (`crates/app-api-gateway/tests/simulated_flow.rs`) exercises helper, fleet, and protected routes to prove the JSON-only mode behaves like a real deployment.
- `adapter-db/tests` now houses parallel suites for the JSON, SQLite, Postgres, and MySQL adapters; they demonstrate how the same configs run under Docker (when available) and how the migration-transfer CLI uses their exported logs. Run these tests with `cargo test -p adapter-db -- --test-threads 1` if you need deterministic ordering or when you want to ensure the log files land in predictable directories.

## When to Fall Back to Real Databases

 - The JSON/SQLite adapters are great for UI and helper validation, but for migration proofs, tenant consensus, and parity testing you will eventually need Postgres or MySQL.  
- Once you are ready for SQL backends, swap or add entries in `database/runtime-adapters.json`, set `DATABASE_URL`, and restart the services to point to the real adapters via the shared registry.

## Live Migration Transfer

- Use the new `platform-migrations` transfer binary to move tenant data between adapters without spinning up Node services. Run from the repo root:
  ```bash
  cargo run -p platform-migrations --bin migration-transfer -- \
    --source-config database/runtime-adapters.json \
    --source-tenant tenant-default \
    --target-config database/runtime-adapters-to-sqlite.json \
    --target-tenant tenant-default
  ```
  The command loads the two adapter configs, acquires each tenant’s adapter, exports the script log (JSON/SQLite), and replays it into the destination adapter inside `platform-migrations` so the tenant is ready to switch datastores once operators point the registry to the new adapter path.

## Legacy Documentation

Old Node-era guides live under `docs/LEGACY_NODE_ARCHIVE.md`. Unless you are examining the prior stack, continue relying on this page plus `docs/rust-backend-plan.md`, `docs/architecture.md`, and `specification/test-scenarios.md` for current guidance.
