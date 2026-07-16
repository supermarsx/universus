param(
    [string]$EnvFile = '.env.example',
    [switch]$Lifecycle
)

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$envPath = (Resolve-Path (Join-Path $repoRoot $EnvFile)).Path
$composeBase = @('compose', '--env-file', $envPath)

function Invoke-Docker {
    param([Parameter(ValueFromRemainingArguments = $true)][string[]]$DockerArgs)
    & docker @DockerArgs
    if ($LASTEXITCODE -ne 0) {
        throw "docker command failed with exit code $LASTEXITCODE"
    }
}

function Invoke-Compose {
    param(
        [string]$Project,
        [Parameter(ValueFromRemainingArguments = $true)][string[]]$ComposeArgs
    )
    & docker @composeBase -p $Project @ComposeArgs
    if ($LASTEXITCODE -ne 0) {
        throw "docker compose $($ComposeArgs -join ' ') failed with exit code $LASTEXITCODE"
    }
}

Push-Location $repoRoot
try {
    $configText = (& docker @composeBase config --format json) -join "`n"
    if ($LASTEXITCODE -ne 0) {
        throw 'docker compose config failed'
    }
    $config = $configText | ConvertFrom-Json -AsHashtable
    $services = $config.services

    $databaseConsumers = @()
    foreach ($entry in $services.GetEnumerator()) {
        $environment = $entry.Value.environment
        if ($environment -and $environment.ContainsKey('DATABASE_URL')) {
            $databaseConsumers += $entry.Key
            $dependencies = $entry.Value.depends_on
            if (-not $dependencies -or
                -not $dependencies.ContainsKey('database') -or
                $dependencies.database.condition -ne 'service_healthy') {
                throw "$($entry.Key) declares DATABASE_URL without a healthy database dependency"
            }
            if (-not $dependencies.ContainsKey('database-migrate') -or
                $dependencies['database-migrate'].condition -ne 'service_completed_successfully') {
                throw "$($entry.Key) declares DATABASE_URL without a successful database-migrate dependency"
            }
        }
    }
    if ($databaseConsumers.Count -eq 0) {
        throw 'no DATABASE_URL consumers were found in Compose'
    }

    $states = @{}
    function Visit-Service {
        param([string]$Name, [string[]]$Path)
        if ($states[$Name] -eq 1) {
            throw "Compose dependency cycle detected: $(($Path + $Name) -join ' -> ')"
        }
        if ($states[$Name] -eq 2) {
            return
        }
        $states[$Name] = 1
        $dependencies = $services[$Name].depends_on
        if ($dependencies) {
            foreach ($dependency in $dependencies.Keys) {
                if ($services.ContainsKey($dependency)) {
                    Visit-Service -Name $dependency -Path ($Path + $Name)
                }
            }
        }
        $states[$Name] = 2
    }
    foreach ($serviceName in $services.Keys) {
        Visit-Service -Name $serviceName -Path @()
    }

    $migrationService = $services['database-migrate']
    if (-not $migrationService -or
        $migrationService.entrypoint[0] -ne '/opt/universus/database/scripts/migrate-db.sh') {
        throw 'database-migrate does not execute the canonical runner'
    }
    Write-Host "Compose migration contract passed ($($databaseConsumers.Count) DATABASE_URL consumers, no dependency cycles)."

    if (-not $Lifecycle) {
        return
    }

    $expectedMigrations = @(Get-ChildItem (Join-Path $repoRoot 'database\sql\steps') -Filter '*.sql' -File).Count
    $databaseEnvironment = $services.database.environment
    $postgresDb = [string]$databaseEnvironment.POSTGRES_DB
    $postgresUser = [string]$databaseEnvironment.POSTGRES_USER
    $postgresPassword = [string]$databaseEnvironment.POSTGRES_PASSWORD
    $freshProject = "universus-compose-fresh-$PID".ToLowerInvariant()
    $legacyProject = "universus-compose-legacy-$PID".ToLowerInvariant()
    $seedContainer = "${legacyProject}-seed"
    $legacyVolume = "${legacyProject}_postgres_data"

    function Wait-Migration {
        param([string]$Project)
        for ($attempt = 0; $attempt -lt 90; $attempt++) {
            $state = (& docker inspect -f '{{.State.Status}}:{{.State.ExitCode}}' universus_database_migrate 2>$null) -join ''
            if ($LASTEXITCODE -eq 0 -and $state -eq 'exited:0') {
                return
            }
            if ($LASTEXITCODE -eq 0 -and $state.StartsWith('exited:')) {
                & docker @composeBase -p $Project logs database-migrate
                throw "database-migrate failed with $state"
            }
            Start-Sleep -Seconds 2
        }
        throw 'database-migrate did not complete within 180 seconds'
    }

    function Query-ComposeDatabase {
        param([string]$Project, [string]$Statement)
        $value = (& docker @composeBase -p $Project exec -T database `
            psql -U $postgresUser -d $postgresDb -Atc $Statement) -join ''
        if ($LASTEXITCODE -ne 0) {
            throw "Compose database query failed: $Statement"
        }
        return $value.Trim()
    }

    foreach ($fixedName in @('universus_database', 'universus_database_migrate')) {
        $existing = (& docker ps -a --filter "name=^/${fixedName}$" --format '{{.Names}}') -join ''
        if ($existing.Trim() -eq $fixedName) {
            throw "refusing lifecycle test because container $fixedName already exists"
        }
    }

    try {
        Invoke-Compose -Project $freshProject -ComposeArgs @('up', '-d', '--build', 'database', 'database-migrate')
        Wait-Migration -Project $freshProject
        $freshCount = Query-ComposeDatabase -Project $freshProject -Statement 'SELECT count(*) FROM universus_schema_migrations;'
        if ([int]$freshCount -ne $expectedMigrations) {
            throw "fresh Compose volume recorded $freshCount/$expectedMigrations migrations"
        }
        $firstRunCount = [int](Query-ComposeDatabase -Project $freshProject -Statement "SELECT count(*) FROM universus_schema_migration_runs WHERE status = 'applied';")

        Invoke-Compose -Project $freshProject -ComposeArgs @('rm', '-sf', 'database-migrate')
        Invoke-Compose -Project $freshProject -ComposeArgs @('up', '-d', 'database-migrate')
        Wait-Migration -Project $freshProject
        $repeatCount = Query-ComposeDatabase -Project $freshProject -Statement 'SELECT count(*) FROM universus_schema_migrations;'
        $secondRunCount = [int](Query-ComposeDatabase -Project $freshProject -Statement "SELECT count(*) FROM universus_schema_migration_runs WHERE status = 'applied';")
        if ([int]$repeatCount -ne $expectedMigrations -or $secondRunCount -le $firstRunCount) {
            throw 'same-volume Compose startup was not repeat-safe or observable'
        }
        Write-Host "Fresh and repeated Compose startup passed ($expectedMigrations migrations)."
    }
    finally {
        & docker @composeBase -p $freshProject down -v --remove-orphans 2>$null | Out-Null
    }

    try {
        Invoke-Docker -DockerArgs @(
            'volume', 'create',
            '--label', "com.docker.compose.project=$legacyProject",
            '--label', 'com.docker.compose.volume=postgres_data',
            $legacyVolume
        ) | Out-Null
        Invoke-Docker -DockerArgs @(
            'run', '-d', '--name', $seedContainer,
            '-e', "POSTGRES_DB=$postgresDb",
            '-e', "POSTGRES_USER=$postgresUser",
            '-e', "POSTGRES_PASSWORD=$postgresPassword",
            '-v', "${legacyVolume}:/var/lib/postgresql/data",
            'postgres:16-alpine'
        ) | Out-Null
        $ready = $false
        for ($attempt = 0; $attempt -lt 60; $attempt++) {
            & docker exec $seedContainer pg_isready -U $postgresUser -d $postgresDb *> $null
            if ($LASTEXITCODE -eq 0) {
                $ready = $true
                break
            }
            Start-Sleep -Seconds 1
        }
        if (-not $ready) {
            throw 'legacy seed PostgreSQL did not become ready'
        }
        Get-Content -Raw (Join-Path $repoRoot 'database\sql\steps\01_core_schema.sql') |
            & docker exec -i $seedContainer psql -v ON_ERROR_STOP=1 -U $postgresUser -d $postgresDb | Out-Null
        if ($LASTEXITCODE -ne 0) {
            throw 'failed to create the no-history legacy schema'
        }
        Invoke-Docker -DockerArgs @(
            'exec', $seedContainer, 'psql', '-v', 'ON_ERROR_STOP=1',
            '-U', $postgresUser, '-d', $postgresDb, '-c',
            "INSERT INTO users (username, email, password_hash) VALUES ('compose_legacy_keeper', 'compose-legacy@example.test', 'legacy-hash');"
        ) | Out-Null
        Invoke-Docker -DockerArgs @('rm', '-f', $seedContainer) | Out-Null

        Invoke-Compose -Project $legacyProject -ComposeArgs @('up', '-d', '--build', 'database', 'database-migrate')
        Wait-Migration -Project $legacyProject
        $legacyCount = Query-ComposeDatabase -Project $legacyProject -Statement 'SELECT count(*) FROM universus_schema_migrations;'
        $sentinelCount = Query-ComposeDatabase -Project $legacyProject -Statement "SELECT count(*) FROM users WHERE username = 'compose_legacy_keeper';"
        if ([int]$legacyCount -ne $expectedMigrations -or [int]$sentinelCount -ne 1) {
            throw 'pre-history Compose volume was not upgraded with its data preserved'
        }
        Write-Host "Pre-history Compose volume upgrade passed ($expectedMigrations migrations, sentinel preserved)."
    }
    finally {
        & docker rm -f $seedContainer 2>$null | Out-Null
        & docker @composeBase -p $legacyProject down -v --remove-orphans 2>$null | Out-Null
        & docker volume rm -f $legacyVolume 2>$null | Out-Null
    }
}
finally {
    Pop-Location
}
