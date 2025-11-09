PowerShell equivalents for repository shell scripts

This file lists PowerShell (.ps1) scripts added as Windows-friendly alternatives to existing shell scripts.

Files added

- `database/scripts/migrate-test-db.ps1`
  - PowerShell equivalent of `database/scripts/migrate-test-db.sh`.
  - Applies SQL files in `database/sql/steps/` to a running Postgres instance using `psql`.
  - Environment variables supported: `PGHOST`, `PGPORT`, `PGUSER`, `PGPASSWORD`, `PGDATABASE`, `VERBOSE_MIGRATE`
  - Usage (PowerShell): `.\ackend\database\scripts\migrate-test-db.ps1` (ensure `psql` is available in PATH)

- `database/scripts/init-db.ps1`
  - PowerShell equivalent of `database/scripts/init-db.sh` used for Docker init scripts.
  - Intended for use inside containers or Windows environments where PowerShell is preferred.

- `scripts/run-integration-local.ps1`
  - PowerShell equivalent of `scripts/run-integration-local.sh`.
  - Starts a Postgres Docker container, waits for readiness, runs the migration script, runs backend integration tests, and removes the container.
  - Requires: Docker, pnpm, and psql (for the migration script).
  - Usage (PowerShell): `.\scripts\run-integration-local.ps1`

Notes

- The PowerShell scripts call `psql` and (in some places) `sh -c` to keep behavior consistent with the existing shell scripts. Ensure `psql` (Postgres client) is installed and available in PATH.
- Scripts that manage Docker containers require Docker Desktop or Docker Engine to be available on the host.
- I did not change the CI workflow; GitHub Actions still runs the shell scripts. The PowerShell files are primarily for local Windows development convenience.

If you want, I can:
- Update `backend/TESTING.md` to mention the PowerShell scripts (I can add short usage examples there).
- Add a PowerShell script for any other shell helpers you’d like ported.
