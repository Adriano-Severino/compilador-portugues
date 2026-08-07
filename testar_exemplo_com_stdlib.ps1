<#
 .SYNOPSIS
    Testa a compilação e execução de um exemplo que depende da biblioteca padrão (sistema-padrao).

 .DESCRIPTION
    Este script automatiza o processo de compilação de um arquivo de exemplo que utiliza
    a biblioteca padrão 'sistema-padrao'. Ele localiza todos os arquivos-fonte da biblioteca,
    os combina com o arquivo de exemplo e os passa para o compilador. Em seguida, executa
    o bytecode resultante com o interpretador.

    Isso simula como um projeto real usaria a biblioteca padrão, garantindo que o compilador
    consiga resolver os 'usando' e as chamadas para as classes da biblioteca.
#>

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Info($m) { Write-Host "[INFO] $m" -ForegroundColor Cyan }
function Ok($m) { Write-Host "[OK]   $m" -ForegroundColor Green }
function Fail($m) { Write-Host "[FAIL] $m" -ForegroundColor Red; exit 1 }

Info "Iniciando teste de integração com a biblioteca padrão..."

# --- Configuração de Caminhos ---
$RaizCompilador = Split-Path -Parent $PSCommandPath
$RaizProjeto = Split-Path -Parent $RaizCompilador

$CompiladorPath = Join-Path $RaizCompilador "target\debug\compilador.exe"
$InterpretadorPath = Join-Path $RaizCompilador "target\debug\interpretador.exe"
$Exemplo = Join-Path $RaizCompilador "exemplos\Utilizado_biblioteca_sistema_padrao.pr"
$StdLibProjetoDir = Join-Path $RaizProjeto "sistema-padrao"
$BuildDir = Join-Path $RaizCompilador "temp_build"
$StdLibBytecode = Join-Path $StdLibProjetoDir "dist\sistema.pbc"

if (!(Test-Path $CompiladorPath)) { Fail "Compilador não encontrado em '$CompiladorPath'. Execute 'cargo build' primeiro." }
if (!(Test-Path $InterpretadorPath)) { Fail "Interpretador não encontrado em '$InterpretadorPath'. Execute 'cargo build' primeiro." }
if (!(Test-Path $Exemplo)) { Fail "Arquivo de exemplo não encontrado em '$Exemplo'." }
if (!(Test-Path $StdLibProjetoDir)) { Fail "Diretório do projeto da biblioteca padrão não encontrado em '$StdLibProjetoDir'." }

# --- Limpeza e Preparação ---
New-Item -ItemType Directory -Force -Path $BuildDir | Out-Null
if (Test-Path $StdLibBytecode) { Remove-Item $StdLibBytecode }

# --- Etapa 1: Compilar a Biblioteca Padrão ---
Info "Compilando a biblioteca padrão 'sistema-padrao'..."
& $CompiladorPath --compilar-biblioteca=$StdLibProjetoDir
if ($LASTEXITCODE -ne 0) { Fail "Falha ao compilar a biblioteca padrão." }
if (!(Test-Path $StdLibBytecode)) { Fail "Bytecode da biblioteca padrão ('$StdLibBytecode') não foi gerado." }
Ok "Biblioteca padrão compilada com sucesso."

# --- Etapa 2: Compilar o Exemplo usando a Biblioteca ---
Push-Location $BuildDir

Info "Compilando o exemplo '$($Exemplo)' com a biblioteca padrão..."
# Passamos o caminho do exemplo e a referência para a biblioteca compilada
& $CompiladorPath $Exemplo --target bytecode --sistema-lib=$StdLibBytecode
if ($LASTEXITCODE -ne 0) { Pop-Location; Fail "Falha na compilação." }
Ok "Compilação concluída com sucesso."

# --- Etapa 3: Execução ---
$bytecodeFile = "Utilizado_biblioteca_sistema_padrao.pbc"
if (!(Test-Path $bytecodeFile)) { Pop-Location; Fail "Arquivo de bytecode '$bytecodeFile' não foi gerado." }

Info "Executando o bytecode com o interpretador..."
& $InterpretadorPath $bytecodeFile
if ($LASTEXITCODE -ne 0) { Pop-Location; Fail "Falha na execução do interpretador." }
Ok "Execução concluída com sucesso."

Pop-Location
Remove-Item -Recurse -Force $BuildDir

Ok "Teste de integração com a biblioteca padrão passou!"