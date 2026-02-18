# Resolves commands to run the consensus/team routing/worker runtime validation suites.
# This orchestrates the suite described in docs/consensus-tests.md and docs/worker-runtime-tests.md.
param(
  [switch]$NoDocker
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$root = Resolve-Path (Join-Path $PSScriptRoot "..\..")
Push-Location $root
try {
  function Invoke-CommandStep {
    param([string]$Cmd, [string]$Label)
    Write-Host "[step] $Label"
    $result = & pwsh -NoProfile -ExecutionPolicy Bypass -Command $Cmd
    if ($LASTEXITCODE -ne 0) {
      throw "$Label failed: $LASTEXITCODE"
    }
  }

  Invoke-CommandStep -Cmd 'cargo test -p platform-consensus -- --test-threads 1' -Label 'platform-consensus leases'
  Invoke-CommandStep -Cmd 'cargo test -p platform-worker-runtime -- --test-threads 1' -Label 'worker-runtime caps'

  if (-not $NoDocker) {
    if (-not (Get-Command docker -ErrorAction SilentlyContinue)) {
      throw 'Docker CLI is required for the adapter-db SQL parity tests.'
    }
    Invoke-CommandStep -Cmd 'cargo test -p adapter-db --test sql_adapters -- --test-threads 1' -Label 'adapter-db SQL parity'
  } else {
    Write-Host "[step] Skipping adapter-db SQL parity tests (--NoDocker set)"
  }

  Write-Host "Consensus + worker validation completed."
}
finally {
  Pop-Location
}
