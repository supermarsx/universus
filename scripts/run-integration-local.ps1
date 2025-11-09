<#
PowerShell wrapper to start a Postgres container via Docker, apply migrations, run integration tests, then remove the container.
Requires: Docker, pnpm, psql (psql is used inside migrate script)
#>
param()

$ContainerName = 'universus-test-db'
$Image = 'postgres:15'
$PGUser = 'postgres'
$PGPassword = 'postgres'
$PGDatabase = 'testdb'
$PGPort = 5432

function Run-Command($cmd) {
  Write-Host "Running: $cmd"
  & sh -c $cmd
  if ($LASTEXITCODE -ne 0) {
    throw "Command failed with exit code $LASTEXITCODE"
  }
}

Write-Host "Starting Postgres container ($ContainerName)..."
Run-Command "docker run --name $ContainerName -e POSTGRES_PASSWORD=$PGPassword -e POSTGRES_USER=$PGUser -e POSTGRES_DB=$PGDatabase -p $PGPort:5432 -d $Image"

Write-Host "Waiting for Postgres to become ready..."
while ($true) {
  $res = & docker exec $ContainerName pg_isready -U $PGUser 2>$null
  if ($LASTEXITCODE -eq 0) { break }
  Start-Sleep -Seconds 1
}

Write-Host "Applying migrations..."
$scriptPath = Join-Path (Split-Path -Parent $MyInvocation.MyCommand.Path) '..\database\scripts\migrate-test-db.sh' | Resolve-Path
Run-Command "PGHOST=localhost PGPORT=$PGPort PGUSER=$PGUser PGPASSWORD=$PGPassword PGDATABASE=$PGDatabase sh -c '$scriptPath'"

Write-Host "Running backend integration tests..."
$env:DATABASE_URL = "postgres://$PGUser:$PGPassword@localhost:$PGPort/$PGDatabase"
$env:RUN_INTEGRATION = 'true'
Run-Command "RUN_INTEGRATION=true pnpm --filter ./backend... run test:integration"

$exitCode = 0

Write-Host "Stopping and removing Postgres container..."
& docker rm -f $ContainerName

exit $exitCode
