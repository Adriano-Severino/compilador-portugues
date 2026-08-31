param(
    [string]$Version = "0.1.4",
    [string]$OutputDir = "dist",
    [switch]$SkipTests
)

$ErrorActionPreference = "Stop"

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$projectDir = Split-Path -Parent $scriptDir
if (-not (Test-Path -LiteralPath (Join-Path $projectDir "Cargo.toml"))) {
    $projectDir = Get-Location
}

Push-Location $projectDir
try {
    Write-Host "==============================================" -ForegroundColor Cyan
    Write-Host " Compilador Por do Sol - Build Release v$Version" -ForegroundColor Cyan
    Write-Host "==============================================" -ForegroundColor Cyan

    if (-not $SkipTests) {
        Write-Host ""
        Write-Host "[1/3] Executando testes..." -ForegroundColor Yellow
        & cargo test
        if ($LASTEXITCODE -ne 0) {
            throw "Falha nos testes unitários e de integração do compilador."
        }
        Write-Host "OK: Todos os testes passaram com sucesso!" -ForegroundColor Green
    } else {
        Write-Host ""
        Write-Host "[1/3] Testes ignorados (-SkipTests)." -ForegroundColor Gray
    }

    Write-Host ""
    Write-Host "[2/3] Compilando binarios em modo Release (opt-level=3, LTO)..." -ForegroundColor Yellow
    & cargo build --release --bin compilador --bin interpretador
    if ($LASTEXITCODE -ne 0) {
        throw "Falha na compilacao em release."
    }
    Write-Host "OK: Binarios compilados com sucesso!" -ForegroundColor Green

    Write-Host ""
    Write-Host "[3/3] Empacotando artefatos em $OutputDir..." -ForegroundColor Yellow
    $pkgName = "compilador-portugues-v$Version-windows-x64"
    $distDir = Join-Path $projectDir $OutputDir
    $pkgDir = Join-Path $distDir $pkgName
    $binDir = Join-Path $pkgDir "bin"

    if (Test-Path -LiteralPath $pkgDir) {
        Remove-Item -LiteralPath $pkgDir -Recurse -Force
    }
    New-Item -ItemType Directory -Force -Path $binDir | Out-Null

    Copy-Item "target\release\compilador.exe" -Destination (Join-Path $binDir "compilador.exe") -Force
    Copy-Item "target\release\interpretador.exe" -Destination (Join-Path $binDir "interpretador.exe") -Force

    foreach ($doc in @("README.md", "LICENSE", "agent.md")) {
        if (Test-Path -LiteralPath $doc) {
            Copy-Item -LiteralPath $doc -Destination (Join-Path $pkgDir $doc) -Force
        }
    }

    $zipFile = Join-Path $distDir "$pkgName.zip"
    if (Test-Path -LiteralPath $zipFile) {
        Remove-Item -LiteralPath $zipFile -Force
    }

    Compress-Archive -Path "$pkgDir\*" -DestinationPath $zipFile -CompressionLevel Optimal -Force
    Write-Host "OK: Pacote ZIP gerado: $zipFile" -ForegroundColor Green

    $hash = (Get-FileHash -Path $zipFile -Algorithm SHA256).Hash.ToLowerInvariant()
    $shaFile = "$zipFile.sha256"
    "$hash  $pkgName.zip" | Out-File -FilePath $shaFile -Encoding ascii
    Write-Host "OK: SHA256: $hash" -ForegroundColor Green
    Write-Host "OK: Checksum gerado: $shaFile" -ForegroundColor Green

    Write-Host ""
    Write-Host "==============================================" -ForegroundColor Cyan
    Write-Host " Release gerada com sucesso!" -ForegroundColor Cyan
    Write-Host " Arquivo: $zipFile" -ForegroundColor White
    Write-Host " Checksum: $shaFile" -ForegroundColor White
    Write-Host "==============================================" -ForegroundColor Cyan
}
finally {
    Pop-Location
}
