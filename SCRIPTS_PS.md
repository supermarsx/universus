# PowerShell migration entry points

Universus has one database migration implementation:
`database/scripts/migrate-db.sh`. It provides semantic numeric ordering,
checksums, advisory locking, atomic step transactions, and durable run history.

The PowerShell files are compatibility launchers for that implementation:

- `database/scripts/migrate-test-db.ps1` uses the test defaults (`testdb` on
  localhost) and delegates to `migrate-test-db.sh`.
- `database/scripts/init-db.ps1` uses `POSTGRES_*` or `PG*` environment values
  and delegates directly to `migrate-db.sh`.

Both launchers require a POSIX `sh` environment such as Git Bash or WSL. The
runner itself requires the PostgreSQL `psql` and `pg_isready` clients.

```powershell
$env:PGHOST = 'localhost'
$env:PGPORT = '5432'
$env:PGUSER = 'postgres'
$env:PGPASSWORD = '<database-password>'
$env:PGDATABASE = 'universus_rpg'
./database/scripts/migrate-test-db.ps1
```

For the full PostgreSQL 16 durability suite, use a POSIX shell:

```bash
database/scripts/test-migrations.sh
```

The Docker Compose stack runs the same canonical runner automatically through
the one-shot `database-migrate` service before database-backed applications.
