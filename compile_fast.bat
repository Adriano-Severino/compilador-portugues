@echo off
if "%1"=="" (
    echo Uso: %0 ^<arquivo_sem_extensao^>
    exit /b 1
)

set PROGRAMA=%1
echo Compilando %PROGRAMA%.pr...

REM Usar o compilador já construído
.\target\release\compilador-portugues.exe "%PROGRAMA%.pr"

REM Gerar executável diretamente
clang -O3 "%PROGRAMA%.ll" -o "%PROGRAMA%.exe"

echo Pronto! Execute com: %PROGRAMA%.exe
