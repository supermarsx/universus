param(
  [switch]$WithGrpc,
  [int]$Iterations = 150,
  [int]$Warmup = 25
)

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..\..")
$env:BENCH_ITERATIONS = [string]$Iterations
$env:BENCH_WARMUP = [string]$Warmup
Write-Host "[bench] BENCH_ITERATIONS=$($env:BENCH_ITERATIONS) BENCH_WARMUP=$($env:BENCH_WARMUP)"

$coreProc = $null
try {
  if ($WithGrpc) {
    Write-Host "[bench] starting backend-core on 127.0.0.1:50051..."
    $env:CORE_BIND_ADDR = "127.0.0.1:50051"
    $coreProc = Start-Process -FilePath cargo -ArgumentList "run","-p","backend-core" -WorkingDirectory $repoRoot -PassThru
    Start-Sleep -Seconds 3
    $env:BACKEND_CORE_ADDR = "127.0.0.1:50051"
  }

  Write-Host "[bench] running benchmark..."
  Push-Location $repoRoot
  pnpm --dir backend run bench:core | Out-Host
  Pop-Location
} finally {
  if ($coreProc -and (Get-Process -Id $coreProc.Id -ErrorAction SilentlyContinue)) {
    Stop-Process -Id $coreProc.Id -Force
  }
}
