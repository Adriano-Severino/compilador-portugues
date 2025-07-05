# testar_exemplos.ps1
$compilador = "target\debug\compilador.exe"
$exemplos = Get-ChildItem -Path "exemplos" -Filter *.pr

if (!(Test-Path $compilador)) {
    Write-Host "Compilador não encontrado em $compilador"
    exit 1
}

$interpretador = "target\debug\interpretador.exe"
$resultados = @()

if (!(Test-Path $interpretador)) {
    Write-Host "Interpretador não encontrado em $interpretador"
    exit 1
}

foreach ($exemplo in $exemplos) {
    Write-Host "Testando $($exemplo.Name)..."
    & $compilador $exemplo.FullName --target bytecode
    $compilou = $LASTEXITCODE -eq 0

    if ($compilou) {
        Write-Host "Compilou OK. Testando interpretador..."
        $bytecode = [System.IO.Path]::ChangeExtension($exemplo.Name, ".pbc")
        & $interpretador $bytecode
        $executou = $LASTEXITCODE -eq 0
        if ($executou) {
            Write-Host "Interpretador OK.`n"
            $resultados += [PSCustomObject]@{Arquivo=$exemplo.Name; Status="OK"; Detalhe="Compilou e interpretador OK"}
        } else {
            Write-Host "ERRO: Falha no interpretador.`n"
            $resultados += [PSCustomObject]@{Arquivo=$exemplo.Name; Status="FALHOU"; Detalhe="Falha no interpretador"}
        }
    } else {
        Write-Host "ERRO: Falha na compilação.`n"
        $resultados += [PSCustomObject]@{Arquivo=$exemplo.Name; Status="FALHOU"; Detalhe="Falha na compilação"}
    }
}

Write-Host "Resumo dos testes:"
foreach ($res in $resultados) {
    if ($res.Status -eq "OK") {
        Write-Host ("[OK]     " + $res.Arquivo + " - " + $res.Detalhe) -ForegroundColor Green
    } elseif ($res.Detalhe -eq "Falha na compilação") {
        Write-Host ("[FALHOU] " + $res.Arquivo + " - Falha na compilação") -ForegroundColor Red
    } elseif ($res.Detalhe -eq "Falha no interpretador") {
        Write-Host ("[FALHOU] " + $res.Arquivo + " - Compilou, mas falhou no interpretador") -ForegroundColor Yellow
    }
}

# Resumo quantitativo
$total = $resultados.Count
$ok = ($resultados | Where-Object { $_.Status -eq "OK" }).Count
$falha_compilacao = ($resultados | Where-Object { $_.Detalhe -eq "Falha na compilação" }).Count

$falha_interpretador = ($resultados | Where-Object { $_.Detalhe -eq "Falha no interpretador" }).Count

Write-Host "\nResumo final:"
Write-Host ("Total: $total")
Write-Host ("Passaram compilação e interpretador: $ok") -ForegroundColor Green
Write-Host ("Falharam na compilação: $falha_compilacao") -ForegroundColor Red
Write-Host ("Compilaram, mas falharam no interpretador: $falha_interpretador") -ForegroundColor Yellow
