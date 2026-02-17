<#
.SYNOPSIS
Runs Rust cutover validation suites and writes a timestamped report.
#>
param(
  [switch]$RunBenchmark
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$root = Resolve-Path (Join-Path $PSScriptRoot "..\..")
$timestamp = Get-Date -Format "yyyy-MM-ddTHH-mm-ss"
$reportDir = Join-Path $root "specification\validation-reports"
$reportPath = Join-Path $reportDir "cutover-validation-$timestamp.md"

if (!(Test-Path $reportDir)) {
  New-Item -ItemType Directory -Path $reportDir | Out-Null
}

$steps = @(
  @{ Name = "workspace-check"; Cmd = "cargo check --workspace" },
  @{ Name = "web-frontend-routes"; Cmd = "cargo test -p app-web-frontend all_template_routes_have_expected_auth_gating_and_render -- --nocapture" },
  @{ Name = "realtime-chat-moderation"; Cmd = "cargo test -p app-realtime-gateway chat_message_moderation_endpoints_update_state -- --nocapture" },
  @{ Name = "api-notifications-load"; Cmd = "cargo test -p app-api-gateway notifications_high_volume_create_flow_stays_consistent -- --nocapture" },
  @{ Name = "api-sharding-churn"; Cmd = "cargo test -p app-api-gateway sharding_registration_churn_keeps_routing_stats_coherent -- --nocapture" },
  @{ Name = "scheduler-key-dedupe"; Cmd = "cargo test -p app-scheduler-worker -- --nocapture" }
)

$results = @()
foreach ($step in $steps) {
  Write-Host "[run] $($step.Name): $($step.Cmd)"
  $start = Get-Date
  $ok = $true
  $output = ""
  try {
    $output = (& pwsh -NoProfile -Command $step.Cmd 2>&1 | Out-String)
  } catch {
    $ok = $false
    $output = $_ | Out-String
  }
  $end = Get-Date
  $duration = [math]::Round(($end - $start).TotalSeconds, 2)
  $results += [pscustomobject]@{
    Name = $step.Name
    Command = $step.Cmd
    Ok = $ok
    DurationSeconds = $duration
    Output = $output.Trim()
  }
  if (!$ok) {
    break
  }
}

if ($RunBenchmark -and @($results | Where-Object { -not $_.Ok }).Count -eq 0) {
  $benchCmd = "pnpm --dir backend run bench:core:pure"
  Write-Host "[run] benchmark: $benchCmd"
  $start = Get-Date
  $ok = $true
  $output = ""
  try {
    $output = (& pwsh -NoProfile -Command $benchCmd 2>&1 | Out-String)
  } catch {
    $ok = $false
    $output = $_ | Out-String
  }
  $end = Get-Date
  $duration = [math]::Round(($end - $start).TotalSeconds, 2)
  $results += [pscustomobject]@{
    Name = "core-pure-benchmark-1m"
    Command = $benchCmd
    Ok = $ok
    DurationSeconds = $duration
    Output = $output.Trim()
  }
}

$failed = @($results | Where-Object { -not $_.Ok }).Count
$status = if ($failed -eq 0) { "PASS" } else { "FAIL" }

$lines = @()
$lines += "# Rust Cutover Validation Report"
$lines += ""
$lines += "Timestamp: $(Get-Date -Format o)"
$lines += "Status: **$status**"
$lines += ""
$lines += "| Step | Status | Duration (s) |"
$lines += "| --- | --- | --- |"
foreach ($r in $results) {
  $state = if ($r.Ok) { "PASS" } else { "FAIL" }
  $lines += "| $($r.Name) | $state | $($r.DurationSeconds) |"
}
$lines += ""
foreach ($r in $results) {
  $lines += "## $($r.Name)"
  $lines += ""
  $lines += ('Command: `' + $r.Command + '`')
  $lines += ""
  $lines += '```text'
  $lines += $r.Output
  $lines += '```'
  $lines += ""
}

$lines -join "`n" | Set-Content -Path $reportPath -Encoding utf8
Write-Host "Report written to $reportPath"

if ($failed -gt 0) {
  exit 1
}
exit 0
