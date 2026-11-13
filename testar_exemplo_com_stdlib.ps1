<#
 .SYNOPSIS
    Testa a compilação e execução de um exemplo que depende da biblioteca padrão (sistema-padrao).

 .DESCRIPTION
    Fluxo moderno da biblioteca padrão:
      1. Compila a biblioteca com --compilar-biblioteca, gerando dist\sistema.pbl (+ .ll).
      2. Compila o exemplo com --target bytecode; o compilador carrega o .pbl
         automaticamente via --stdlib-src-path (equivalente a Reference Assemblies).
      3. Executa o .pbc resultante com o interpretador.

    O formato legado dist\sistema.pbc não é mais gerado nem aceito pelo compilador.
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
$Exemplo = Join-Path $RaizCompilador "exemplos\teste_stdlib_fase6.pr"
$StdLibProjetoDir = Join-Path $RaizProjeto "sistema-padrao"
$BuildDir = Join-Path $RaizCompilador "temp_build"
$StdLibPbl = Join-Path $StdLibProjetoDir "dist\sistema.pbl"

if (!(Test-Path $CompiladorPath)) { Fail "Compilador não encontrado em '$CompiladorPath'. Execute 'cargo build' primeiro." }
if (!(Test-Path $InterpretadorPath)) { Fail "Interpretador não encontrado em '$InterpretadorPath'. Execute 'cargo build' primeiro." }
if (!(Test-Path $Exemplo)) { Fail "Arquivo de exemplo não encontrado em '$Exemplo'." }
if (!(Test-Path $StdLibProjetoDir)) { Fail "Diretório do projeto da biblioteca padrão não encontrado em '$StdLibProjetoDir'." }

# --- Limpeza e Preparação ---
New-Item -ItemType Directory -Force -Path $BuildDir | Out-Null

# --- Etapa 1: Compilar a Biblioteca Padrão (.pbl) ---
Info "Compilando a biblioteca padrão 'sistema-padrao'..."
& $CompiladorPath --compilar-biblioteca=$StdLibProjetoDir
if ($LASTEXITCODE -ne 0) { Fail "Falha ao compilar a biblioteca padrão." }
if (!(Test-Path $StdLibPbl)) { Fail "Biblioteca padrão ('$StdLibPbl') não foi gerada." }
Ok "Biblioteca padrão compilada com sucesso (.pbl)."

# --- Etapa 2: Compilar o Exemplo usando a Biblioteca ---
Info "Compilando o exemplo '$($Exemplo)' com a biblioteca padrão..."
& $CompiladorPath $Exemplo --target bytecode --output-dir=$BuildDir --stdlib-src-path=$StdLibProjetoDir
if ($LASTEXITCODE -ne 0) { Fail "Falha na compilação." }
Ok "Compilação concluída com sucesso."

# --- Etapa 3: Execução ---
$bytecodeFile = Join-Path $BuildDir "teste_stdlib_fase6.pbc"
if (!(Test-Path $bytecodeFile)) { Fail "Arquivo de bytecode '$bytecodeFile' não foi gerado." }

Info "Executando o bytecode com o interpretador..."
& $InterpretadorPath $bytecodeFile
if ($LASTEXITCODE -ne 0) { Fail "Falha na execução do interpretador." }
Ok "Execução concluída com sucesso."

Remove-Item -Recurse -Force $BuildDir

Ok "Teste de integração com a biblioteca padrão passou!"
