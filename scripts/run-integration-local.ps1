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

function Run-CommandAndThrow([string]$exe, [string[]]$args) {
  Write-Host "Running: $exe $([string]::Join(' ', $args))"
  & $exe @args
  if ($LASTEXITCODE -ne 0) {
    throw "Command failed with exit code $LASTEXITCODE"
  }
}

Write-Host "Starting Postgres container ($ContainerName)..."
Run-CommandAndThrow 'docker' @('run','--name',$ContainerName,'-e',"POSTGRES_PASSWORD=$PGPassword",'-e',"POSTGRES_USER=$PGUser",'-e',"POSTGRES_DB=$PGDatabase",'-p',"$PGPort:5432",'-d',$Image)

Write-Host "Waiting for Postgres to become ready..."
while ($true) {
  & docker exec $ContainerName pg_isready -U $PGUser 2>$null
  if ($LASTEXITCODE -eq 0) { break }
  Start-Sleep -Seconds 1
}

Write-Host "Applying migrations..."
# Call the PowerShell migrate script directly (pure PowerShell path)
$scriptPath = Join-Path (Split-Path -Parent $MyInvocation.MyCommand.Path) '..\database\scripts\migrate-test-db.ps1' | Resolve-Path -ErrorAction Stop
$psArgs = @()
$env:PGHOST = 'localhost'
$env:PGPORT = $PGPort.ToString()
$env:PGUSER = $PGUser
$env:PGPASSWORD = $PGPassword
$env:PGDATABASE = $PGDatabase
# Invoke the PowerShell script in the current process so environment variables are inherited
& $scriptPath.Path

Write-Host "Running backend integration tests..."
$env:DATABASE_URL = "postgres://$PGUser:$PGPassword@localhost:$PGPort/$PGDatabase"
$env:RUN_INTEGRATION = 'true'
Run-CommandAndThrow 'pnpm' @(' --filter','./backend...','run','test:integration')

$exitCode = 0

Write-Host "Stopping and removing Postgres container..."
& docker rm -f $ContainerName

exit $exitCode

