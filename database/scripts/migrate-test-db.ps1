<#
.SYNOPSIS
Runs the canonical Universus migration engine from PowerShell.

.DESCRIPTION
Ordering, checksums, advisory locking, history, and atomic transactions live in
migrate-db.sh. This compatibility entry point deliberately delegates instead
of maintaining a second migration implementation.
#>

$ErrorActionPreference = 'Stop'
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$runner = Join-Path $scriptDir 'migrate-test-db.sh'
$shell = Get-Command sh -ErrorAction SilentlyContinue

if (-not $shell) {
    Write-Error 'sh is required to run the canonical database migrator (Git Bash, WSL, or a POSIX environment).'
    exit 2
}

& $shell.Source $runner @args
exit $LASTEXITCODE
