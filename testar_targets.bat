@echo off
echo === Testando Compilador Portugues ===

echo.
echo Teste Universal
cargo run --bin compilador -- teste.pr --target=universal
if errorlevel 1 (
    echo Falha no target universal
    exit /b 1
)

echo.
echo Teste Console
cargo run --bin compilador -- teste.pr --target=console
if errorlevel 1 (
    echo Falha no target console
    exit /b 1
)

echo.
echo Teste LLVM IR
cargo run --bin compilador -- teste.pr --target=llvm-ir
if errorlevel 1 (
    echo Falha no target llvm-ir
    exit /b 1
)

echo.
echo Teste CIL Bytecode
cargo run --bin compilador -- teste.pr --target=cil-bytecode
if errorlevel 1 (
    echo Falha no target cil-bytecode
    exit /b 1
)

echo.
echo Teste Bytecode
cargo run --bin compilador -- teste.pr --target=bytecode
if errorlevel 1 (
    echo Falha no target bytecode
    exit /b 1
)

echo.
echo Todos os targets testados com sucesso!
