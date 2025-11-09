<#
.SYNOPSIS
Apply SQL migration steps to a running Postgres DB using psql (PowerShell version).

ENV VARIABLES (optional):
 - PGHOST (default: localhost)
 - PGPORT (default: 5432)
 - PGUSER (default: postgres)
 - PGPASSWORD (default: postgres)
 - PGDATABASE (default: testdb)
 - VERBOSE_MIGRATE (if 'true', prints sample rows)
#>

param()

$PGHOST = if ($env:PGHOST) { $env:PGHOST } else { 'localhost' }
$PGPORT = if ($env:PGPORT) { $env:PGPORT } else { '5432' }
$PGUSER = if ($env:PGUSER) { $env:PGUSER } else { 'postgres' }
$PGPASSWORD = if ($env:PGPASSWORD) { $env:PGPASSWORD } else { 'postgres' }
$PGDATABASE = if ($env:PGDATABASE) { $env:PGDATABASE } else { 'testdb' }

# Ensure psql is available
if (-not (Get-Command psql -ErrorAction SilentlyContinue)) {
  Write-Host "psql not found in PATH. Please install Postgres client tools and ensure psql is available." -ForegroundColor Red
  exit 1
}

$env:PGPASSWORD = $PGPASSWORD

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$SQL_DIR = Join-Path $ScriptDir '..\sql\steps' | Resolve-Path -ErrorAction Stop
$SQL_DIR = $SQL_DIR.Path

Write-Host "Applying SQL files from $SQL_DIR to $PGDATABASE@$PGHOST:$PGPORT"

function Run-PsqlCommand($command) {
  $attempt = 0
  $maxAttempts = 3
  $delay = 2
  while ($attempt -lt $maxAttempts) {
    try {
      & sh -c $command
      return $true
    } catch {
      $attempt++
      Write-Host "psql command failed (attempt $attempt/$maxAttempts). Retrying in $delay seconds..." -ForegroundColor Yellow
      Start-Sleep -Seconds $delay
    }
  }
  Write-Host "psql command failed after $maxAttempts attempts." -ForegroundColor Red
  return $false
}

# Apply each .sql file in the steps directory in alphabetical order
Get-ChildItem -Path $SQL_DIR -Filter '*.sql' -File | Sort-Object Name | ForEach-Object {
  $file = $_.FullName
  Write-Host "Applying $($_.Name)"
  $cmd = "psql -v ON_ERROR_STOP=1 -h `"$PGHOST`" -p `"$PGPORT`" -U `"$PGUSER`" -d `"$PGDATABASE`" -f `"$file`""
  if (-not (Run-PsqlCommand $cmd)) { exit 1 }
}

Write-Host "Applying schema completed. Running sanity checks..."

function Check-Table($tbl) {
  $cmd = "psql -h `"$PGHOST`" -p `"$PGPORT`" -U `"$PGUSER`" -d `"$PGDATABASE`" -t -c \"SELECT COUNT(*) FROM $tbl;\""
  try {
    $output = & sh -c $cmd 2>$null
    if ($LASTEXITCODE -ne 0) { throw }
    $value = ($output -join "").Trim()
    Write-Host "$tbl: $value"
    return $true
  } catch {
    Write-Host "Sanity check failed: $tbl table missing or psql error" -ForegroundColor Red
    return $false
  }
}

if (-not (Check-Table 'users')) { exit 1 }
if (-not (Check-Table 'planets')) { exit 1 }
if (-not (Check-Table 'fleets')) { exit 1 }

if ($env:VERBOSE_MIGRATE -eq 'true') {
  Write-Host "Listing sample rows from users, planets, fleets for verification:"
  & sh -c "psql -h `"$PGHOST`" -p `"$PGPORT`" -U `"$PGUSER`" -d `"$PGDATABASE`" -c \"SELECT * FROM users LIMIT 3;\""
  & sh -c "psql -h `"$PGHOST`" -p `"$PGPORT`" -U `"$PGUSER`" -d `"$PGDATABASE`" -c \"SELECT * FROM planets LIMIT 3;\""
  & sh -c "psql -h `"$PGHOST`" -p `"$PGPORT`" -U `"$PGUSER`" -d `"$PGDATABASE`" -c \"SELECT * FROM fleets LIMIT 3;\""
}

Write-Host "Sanity checks passed." -ForegroundColor Green
