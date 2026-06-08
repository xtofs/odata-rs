param(
    [string]$BaseUrl = "http://127.0.0.1:3000",
    [int]$StartupTimeoutSeconds = 30
)

$ErrorActionPreference = "Stop"

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$hurlFile = Join-Path $repoRoot "examples/rooms/demo.hurl"

if (-not (Get-Command hurl -ErrorAction SilentlyContinue)) {
    throw "hurl is not installed or not in PATH. Install it from https://hurl.dev/docs/installation.html"
}

$server = $null
try {
    $server = Start-Process -FilePath "cargo" `
        -ArgumentList @("run", "-p", "odata-rs", "--example", "rooms", "--features", "sqlx-sqlite") `
        -WorkingDirectory $repoRoot `
        -PassThru

    $deadline = (Get-Date).AddSeconds($StartupTimeoutSeconds)
    $ready = $false
    while ((Get-Date) -lt $deadline) {
        if ($server.HasExited) {
            throw "rooms server exited before becoming ready (exit code $($server.ExitCode))."
        }

        try {
            $response = Invoke-WebRequest -Uri "$BaseUrl/" -Method Get -TimeoutSec 2 -UseBasicParsing
            if ($response.StatusCode -ge 200 -and $response.StatusCode -lt 500) {
                $ready = $true
                break
            }
        }
        catch {
            # Not ready yet.
        }

        Start-Sleep -Milliseconds 500
    }

    if (-not $ready) {
        throw "rooms server did not become ready within $StartupTimeoutSeconds seconds at $BaseUrl"
    }

    & hurl --variable "baseUrl=$BaseUrl" "$hurlFile"
    if ($LASTEXITCODE -ne 0) {
        throw "hurl checks failed (exit code $LASTEXITCODE)."
    }

    Write-Host "Hurl scenario passed."
}
finally {
    if ($null -ne $server -and -not $server.HasExited) {
        Stop-Process -Id $server.Id -Force
    }
}