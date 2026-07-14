[CmdletBinding()]
param(
    [switch]$SkipRender,
    [switch]$SkipTests,
    [switch]$Release,
    [ValidateSet("480p", "720p", "1080p", "4k", "8k", "square-1k", "square-2k", "square-4k", "vertical-720p", "vertical-1080p", "vertical-4k")]
    [string]$Preset = "1080p",
    [ValidateSet("preview", "standard", "ultra")]
    [string]$Quality = "standard",
    [ValidateSet("small", "medium", "large")]
    [string]$PlanetSize = "medium",
    [string]$Seed = "0x5EED_1208_0001",
    [string]$Archetype,
    [string]$OutputDir,
    [switch]$EmitMaterialMaps,
    [switch]$EmitManifest,
    [switch]$EmitRaytracePreview,
    [ValidateRange(1, [int]::MaxValue)]
    [int]$TraceSize = 192,
    [ValidateRange(0, [int]::MaxValue)]
    [int]$TraceSamples = 0,
    [ValidateSet("human", "json", "quiet")]
    [string]$Progress = "human",
    [int]$Threads = 0,
    [string]$Cargo = "cargo"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..\..")
$runArgs = @("run", "-p", "game-planet-visuals", "--bin", "render_planet")
$testArgs = @("test", "-p", "game-planet-visuals", "--test", "render_quality")

if ($Release) {
    $runArgs += "--release"
    $testArgs += "--release"
}

if ($Threads -lt 0) {
    throw "-Threads must be 1 or greater when provided."
}

$runArgs += @(
    "--",
    "--preset", $Preset,
    "--quality", $Quality,
    "--planet-size", $PlanetSize,
    "--seed", $Seed,
    "--progress", $Progress
)

if ($PSBoundParameters.ContainsKey("Archetype")) {
    $runArgs += @("--archetype", $Archetype)
}

if ($PSBoundParameters.ContainsKey("OutputDir")) {
    $runArgs += @("--output-dir", $OutputDir)
}

if ($EmitMaterialMaps) {
    $runArgs += "--emit-material-maps"
}

if ($EmitManifest) {
    $runArgs += "--emit-manifest"
}

if ($EmitRaytracePreview) {
    $runArgs += @("--emit-raytrace-preview", "--trace-size", $TraceSize.ToString([Globalization.CultureInfo]::InvariantCulture))
    if ($TraceSamples -gt 0) {
        $runArgs += @("--trace-samples", $TraceSamples.ToString([Globalization.CultureInfo]::InvariantCulture))
    }
}

if ($Threads -gt 0) {
    $runArgs += @("--threads", $Threads.ToString([Globalization.CultureInfo]::InvariantCulture))
}

function Invoke-CargoChecked {
    param(
        [Parameter(Mandatory = $true)]
        [string[]]$Arguments
    )

    & $Cargo @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "cargo $($Arguments -join ' ') failed with exit code $LASTEXITCODE"
    }
}

Push-Location $repoRoot
try {
    if (-not $SkipRender) {
        Write-Host "Rendering Rust planet prototype ($PlanetSize planet, $Preset, $Quality, seed $Seed)..."
        Invoke-CargoChecked -Arguments $runArgs
        if ($PSBoundParameters.ContainsKey("OutputDir")) {
            Write-Host "Rust planet outputs: $OutputDir"
        }
        else {
            Write-Host "Rust planet outputs: assets\planet-rust-prototype"
        }
    }

    if (-not $SkipTests) {
        Write-Host "Running Rust planet render-quality tests..."
        Invoke-CargoChecked -Arguments $testArgs
    }

    if ($SkipRender -and $SkipTests) {
        Write-Host "No actions selected."
    }
}
finally {
    Pop-Location
}
