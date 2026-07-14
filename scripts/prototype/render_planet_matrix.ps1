[CmdletBinding()]
param(
    [ValidateSet("480p", "720p", "1080p", "4k", "8k", "square-1k", "square-2k", "square-4k", "vertical-720p", "vertical-1080p", "vertical-4k")]
    [string[]]$Presets = @("720p", "square-1k"),

    [ValidateSet("preview", "standard", "ultra")]
    [string[]]$Qualities = @("preview"),

    [ValidateSet("small", "medium", "large")]
    [string[]]$PlanetSizes = @("small", "medium", "large"),

    [string[]]$Seeds = @(
        "104372302774273",
        "104372302774274",
        "104372302774275"
    ),

    [string[]]$ArchetypeLabels = @(
        "catalog.archetype.temperate-continents",
        "catalog.archetype.global-ocean",
        "catalog.archetype.banded-gas-giant"
    ),

    [ValidateRange(0, [int]::MaxValue)]
    [int]$Limit = 0,

    [switch]$SkipMaterialMaps,
    [switch]$EmitRaytracePreview,
    [ValidateRange(1, [int]::MaxValue)]
    [int]$TraceSize = 192,
    [ValidateRange(0, [int]::MaxValue)]
    [int]$TraceSamples = 0,
    [switch]$Release,
    [switch]$DryRun,
    [ValidateSet("human", "json", "quiet")]
    [string]$Progress = "human",
    [ValidateRange(0, [int]::MaxValue)]
    [int]$Threads = 0,
    [string]$Cargo = "cargo"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..\..")

function Format-CommandArgument {
    param(
        [AllowEmptyString()]
        [string]$Argument
    )

    if ($Argument -eq "") {
        return "''"
    }

    if ($Argument -match "\s|'") {
        $escaped = $Argument -replace "'", "''"
        return "'$escaped'"
    }

    return $Argument
}

function Format-CommandLine {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Executable,

        [Parameter(Mandatory = $true)]
        [string[]]$Arguments
    )

    $parts = @($Executable) + $Arguments
    return ($parts | ForEach-Object { Format-CommandArgument -Argument $_ }) -join " "
}

function New-CargoRunArguments {
    param(
        [string[]]$RendererArguments,
        [switch]$UseRelease
    )

    $arguments = @("run", "-p", "game-planet-visuals", "--bin", "render_planet")
    if ($UseRelease) {
        $arguments += "--release"
    }

    $arguments += "--"
    $arguments += $RendererArguments
    return ,$arguments
}

function Invoke-CargoChecked {
    param(
        [Parameter(Mandatory = $true)]
        [string[]]$Arguments,

        [switch]$CaptureOutput,
        [switch]$DryRun
    )

    Write-Host ("> " + (Format-CommandLine -Executable $Cargo -Arguments $Arguments))

    if ($DryRun) {
        Write-Host "Dry run: command not executed."
        return ""
    }

    if ($CaptureOutput) {
        $previousErrorActionPreference = $ErrorActionPreference
        try {
            $ErrorActionPreference = "Continue"
            $output = & $Cargo @Arguments 2>&1
            $exitCode = $LASTEXITCODE
        }
        finally {
            $ErrorActionPreference = $previousErrorActionPreference
        }

        if ($exitCode -ne 0) {
            if ($output) {
                $output | ForEach-Object { Write-Host $_ }
            }

            throw "cargo $($Arguments -join ' ') failed with exit code $exitCode"
        }

        $outputLines = @($output | ForEach-Object { $_.ToString() })
        return [string]::Join([Environment]::NewLine, $outputLines)
    }

    & $Cargo @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "cargo $($Arguments -join ' ') failed with exit code $LASTEXITCODE"
    }
}

function Select-AdvertisedFlag {
    param(
        [Parameter(Mandatory = $true)]
        [string]$HelpText,

        [Parameter(Mandatory = $true)]
        [string[]]$Candidates
    )

    foreach ($candidate in $Candidates) {
        $pattern = "(?m)(^|\s)" + [regex]::Escape($candidate) + "($|[\s=,<])"
        if ($HelpText -match $pattern) {
            return $candidate
        }
    }

    return $null
}

function Get-RenderPlanetCapabilities {
    $helpArgs = New-CargoRunArguments -RendererArguments @("--help") -UseRelease:$Release
    $helpText = Invoke-CargoChecked -Arguments $helpArgs -CaptureOutput

    return [pscustomobject]@{
        SeedFlag             = Select-AdvertisedFlag -HelpText $helpText -Candidates @("--seed", "--planet-seed")
        ArchetypeLabelFlag   = Select-AdvertisedFlag -HelpText $helpText -Candidates @("--archetype-label", "--archetype-key", "--archetype", "--planet-archetype")
        SkipMaterialMapsFlag = Select-AdvertisedFlag -HelpText $helpText -Candidates @("--skip-material-maps", "--no-material-maps", "--skip-maps")
    }
}

function Assert-NonEmptyList {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Name,

        [string[]]$Values
    )

    if ($null -eq $Values -or $Values.Count -eq 0) {
        throw "$Name must contain at least one value."
    }
}

function New-RequestedSamples {
    $seedValues = @($Seeds | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
    $archetypeValues = @($ArchetypeLabels | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
    $sampleCount = [Math]::Max($seedValues.Count, $archetypeValues.Count)

    if ($sampleCount -eq 0) {
        return ,([pscustomobject]@{
            Seed           = $null
            ArchetypeLabel = $null
            Label          = "default"
        })
    }

    $samples = for ($index = 0; $index -lt $sampleCount; $index += 1) {
        $seed = $null
        if ($index -lt $seedValues.Count) {
            $seed = $seedValues[$index]
        }
        elseif ($seedValues.Count -eq 1) {
            $seed = $seedValues[0]
        }

        $archetype = $null
        if ($index -lt $archetypeValues.Count) {
            $archetype = $archetypeValues[$index]
        }
        elseif ($archetypeValues.Count -eq 1) {
            $archetype = $archetypeValues[0]
        }

        $labelParts = @()
        if ($archetype) {
            $labelParts += $archetype
        }
        if ($seed) {
            $labelParts += "seed-$seed"
        }

        $label = "sample-$($index + 1)"
        if ($labelParts.Count -gt 0) {
            $label = $labelParts -join " "
        }

        [pscustomobject]@{
            Seed           = $seed
            ArchetypeLabel = $archetype
            Label          = $label
        }
    }

    return ,@($samples)
}

function Select-SupportedSamples {
    param(
        [Parameter(Mandatory = $true)]
        [object[]]$Samples,

        [Parameter(Mandatory = $true)]
        [object]$Capabilities
    )

    if (-not $Capabilities.SeedFlag -and ($Seeds | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })) {
        Write-Host "render_planet does not advertise a seed flag; seed samples will use the binary default."
    }

    if (-not $Capabilities.ArchetypeLabelFlag -and ($ArchetypeLabels | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })) {
        Write-Host "render_planet does not advertise an archetype flag; archetype samples will use the binary default."
    }

    $seen = @{}
    $supported = foreach ($sample in $Samples) {
        $seed = $null
        if ($Capabilities.SeedFlag) {
            $seed = $sample.Seed
        }

        $archetype = $null
        if ($Capabilities.ArchetypeLabelFlag) {
            $archetype = $sample.ArchetypeLabel
        }

        $key = "$seed|$archetype"
        if (-not $seen.ContainsKey($key)) {
            $seen[$key] = $true
            [pscustomobject]@{
                Seed           = $seed
                ArchetypeLabel = $archetype
                Label          = $sample.Label
            }
        }
    }

    if (-not $supported) {
        return ,([pscustomobject]@{
            Seed           = $null
            ArchetypeLabel = $null
            Label          = "default"
        })
    }

    return ,@($supported)
}

function ConvertTo-SafePathSegment {
    param(
        [AllowEmptyString()]
        [string]$Value
    )

    if ([string]::IsNullOrWhiteSpace($Value)) {
        return "default"
    }

    $segment = $Value.ToLowerInvariant() -replace "[^a-z0-9._-]+", "-"
    $segment = $segment.Trim("-._")
    if ([string]::IsNullOrWhiteSpace($segment)) {
        return "sample"
    }

    return $segment
}

Assert-NonEmptyList -Name "Presets" -Values $Presets
Assert-NonEmptyList -Name "Qualities" -Values $Qualities
Assert-NonEmptyList -Name "PlanetSizes" -Values $PlanetSizes

Push-Location $repoRoot
try {
    $capabilities = Get-RenderPlanetCapabilities
    $samples = Select-SupportedSamples -Samples (New-RequestedSamples) -Capabilities $capabilities

    $plannedCount = $Presets.Count * $Qualities.Count * $PlanetSizes.Count * $samples.Count
    if ($Limit -gt 0 -and $Limit -lt $plannedCount) {
        Write-Host "Running $Limit of $plannedCount planned render commands."
    }
    else {
        Write-Host "Running $plannedCount planned render commands."
    }

    $runCount = 0
    :matrix foreach ($preset in $Presets) {
        foreach ($quality in $Qualities) {
            foreach ($planetSize in $PlanetSizes) {
                foreach ($sample in $samples) {
                    if ($Limit -gt 0 -and $runCount -ge $Limit) {
                        break matrix
                    }

                    $rendererArgs = @(
                        "--preset", $preset,
                        "--quality", $quality,
                        "--planet-size", $planetSize
                    )

                    if ($capabilities.SeedFlag -and $sample.Seed) {
                        $rendererArgs += @($capabilities.SeedFlag, $sample.Seed)
                    }

                    if ($capabilities.ArchetypeLabelFlag -and $sample.ArchetypeLabel) {
                        $rendererArgs += @($capabilities.ArchetypeLabelFlag, $sample.ArchetypeLabel)
                    }

                    if ($SkipMaterialMaps -and $capabilities.SkipMaterialMapsFlag) {
                        $rendererArgs += $capabilities.SkipMaterialMapsFlag
                    }

                    $outputDir = Join-Path "target\planet-render-matrix" (Join-Path (ConvertTo-SafePathSegment -Value $sample.Label) (Join-Path $preset (Join-Path $quality $planetSize)))
                    $rendererArgs += @(
                        "--output-dir", $outputDir,
                        "--emit-manifest",
                        "--progress", $Progress
                    )
                    if (-not $SkipMaterialMaps) {
                        $rendererArgs += "--emit-material-maps"
                    }
                    if ($EmitRaytracePreview) {
                        $rendererArgs += @(
                            "--emit-raytrace-preview",
                            "--trace-size", $TraceSize.ToString([Globalization.CultureInfo]::InvariantCulture)
                        )
                        if ($TraceSamples -gt 0) {
                            $rendererArgs += @("--trace-samples", $TraceSamples.ToString([Globalization.CultureInfo]::InvariantCulture))
                        }
                    }
                    if ($Threads -gt 0) {
                        $rendererArgs += @("--threads", $Threads.ToString([Globalization.CultureInfo]::InvariantCulture))
                    }

                    Write-Host "Sample: $($sample.Label); preset=$preset; quality=$quality; planet-size=$planetSize"
                    $cargoArgs = New-CargoRunArguments -RendererArguments $rendererArgs -UseRelease:$Release
                    Invoke-CargoChecked -Arguments $cargoArgs -DryRun:$DryRun
                    $runCount += 1
                }
            }
        }
    }

    if ($DryRun) {
        Write-Host "Prepared $runCount render command(s)."
    }
    else {
        Write-Host "Completed $runCount render command(s)."
    }
}
finally {
    Pop-Location
}
