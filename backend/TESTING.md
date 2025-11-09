Testing Guide — Backend

This document explains how to run the backend test suites locally and in CI, and how we separate fast, DB-free unit tests from DB-backed integration tests.

Overview

- Unit tests (fast, run without a real Postgres): placed under `tests/unit` and run by default in CI.
- Integration tests (require Postgres): placed under `tests/integration` and run only when `RUN_INTEGRATION=true`.
- A lightweight DB mock is provided during unit runs so imports that reference the `pool` object do not fail at module load time.

Commands

From the repository root (recommended):

- Install dependencies (pnpm):
  - `pnpm install`

- Run all tests (this will NOT run integration tests unless `RUN_INTEGRATION=true`):
  - `pnpm --filter ./backend... run test`

- Run unit tests only (no DB required):
  - `pnpm --filter ./backend... run test:unit`

- Run integration tests (requires a running Postgres and proper DB connection):
  - `RUN_INTEGRATION=true pnpm --filter ./backend... run test:integration`

Notes for environments without `pnpm` installed

- If you don't have `pnpm`, use `npm` to install it globally or run the npm equivalents:
  - `npm install -g pnpm`

Why tests are separated

- Unit tests are intended to run quickly in CI and developer machines without requiring DB services.
- Integration tests exercise end-to-end behaviour and require a database; they are isolated and opt-in to avoid flakiness in the main CI pass.

How the DB mock works

- `backend/tests/setup/dbMock.ts` is loaded for unit test runs (via the `--setupFilesAfterEnv` option in the `test:unit` script).
- When `RUN_INTEGRATION !== 'true'`, the setup injects a minimal `pool` mock so modules importing `pool` during initialization do not throw. Tests that need specific DB responses should mock `pool.query` / `pool.connect` per-test (many existing unit tests already do this).

Running integration tests locally (Postgres via Docker)

PowerShell users: we provide PowerShell helpers to run the same flow on Windows. The PowerShell scripts are pure PowerShell and do not rely on `sh -c`.

Examples (PowerShell):

1) Start a Postgres container and apply migrations using the provided helper:

   .\scripts\run-integration-local.ps1

2) Or run the migrate script directly against a running Postgres:

   $env:PGPASSWORD = 'postgres'
   .\database\scripts\migrate-test-db.ps1




1) Start a Postgres container (example):

   docker run --name universus-test-db -e POSTGRES_PASSWORD=postgres -e POSTGRES_USER=postgres -e POSTGRES_DB=testdb -p 5432:5432 -d postgres:15

2) (Optional) Apply migrations/schema to the test DB so the tables required by integration tests exist. The project may include SQL or migration scripts under `database/` or `scripts/` — run those against the test DB. Example using psql:

   PGPASSWORD=postgres psql -h localhost -U postgres -d testdb -f path/to/schema.sql

3) Export the DB connection URL and run integration tests:

   export DATABASE_URL=postgres://postgres:postgres@localhost:5432/testdb
   RUN_INTEGRATION=true pnpm --filter ./backend... run test:integration

CI configuration notes

- The repository includes a GitHub Actions workflow (`.github/workflows/ci.yml`). The workflow runs the fast unit test pass by default.
- A separate `integration-tests` job has been added that starts a Postgres service and runs integration tests with `RUN_INTEGRATION=true`.
- If your CI environment requires different DB credentials or uses service containers differently, adapt the `DATABASE_URL` env var accordingly.

Troubleshooting

- `pnpm` not found: install with `npm install -g pnpm`.
- Tests failing due to missing tables: ensure your integration DB has been seeded or migrations applied.
- If a unit test attempts to use a real DB, mock `pool.query` for that test or move the test to `tests/integration` if it must use a real DB.

Recommended next improvements

- Add a small helper to `tests/setup/dbMock.ts` to let unit tests pre-seed default `pool.query` responses more ergonomically (now available via `__setDefaultQueryResponse` and `__getMockPool`).
- Add a one-step migration script or container that seeds the test DB schema in the CI integration job before tests run.

Helper usage example

- In a unit test that relies on `pool.query` returning rows by default:
  - `import { __setDefaultQueryResponse } from 'tests/setup/dbMock';`
  - `__setDefaultQueryResponse({ rows: [{ id: 1 }], rowCount: 1 });`

If you want, I can add the migration step to the `integration-tests` job (it will require a schema file or a migration script).