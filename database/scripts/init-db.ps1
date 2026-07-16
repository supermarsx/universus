<#
.SYNOPSIS
Runs the canonical Universus migration engine from PowerShell.

.DESCRIPTION
This file is retained for callers that historically used init-db.ps1. It now
delegates all ordering and durability behavior to migrate-db.sh.
#>

$ErrorActionPreference = 'Stop'
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$runner = Join-Path $scriptDir 'migrate-db.sh'
$shell = Get-Command sh -ErrorAction SilentlyContinue

if (-not $env:PGUSER) {
    $env:PGUSER = if ($env:POSTGRES_USER) { $env:POSTGRES_USER } else { 'postgres' }
}
if (-not $env:PGDATABASE) {
    $env:PGDATABASE = if ($env:POSTGRES_DB) { $env:POSTGRES_DB } else { 'postgres' }
}
if (-not $env:PGPASSWORD -and $env:POSTGRES_PASSWORD) {
    $env:PGPASSWORD = $env:POSTGRES_PASSWORD
}
if (-not $shell) {
    Write-Error 'sh is required to run the canonical database migrator (Git Bash, WSL, or a POSIX environment).'
    exit 2
}

& $shell.Source $runner @args
exit $LASTEXITCODE
