#!/bin/bash
# test_compilador.sh

echo "=== Testando Compilador Português ==="

# 1. Teste básico
echo "1. Testando compilação básica..."
./target/release/compilador-portugues teste.pr
if [ $? -eq 0 ]; then
    echo "✓ Compilação básica funcionou"
else
    echo "✗ Erro na compilação básica"
    exit 1
fi

# 2. Teste MAUI
echo "2. Testando target MAUI..."
./target/release/compilador-portugues teste.pr --target=maui-hybrid
if [ -d "teste" ] && [ -f "teste/teste.csproj" ]; then
    echo "✓ Target MAUI funcionou"
    cd teste && dotnet build && cd ..
    if [ $? -eq 0 ]; then
        echo "✓ Projeto MAUI compila no .NET"
    else
        echo "⚠ Projeto MAUI gerado mas não compila no .NET"
    fi
else
    echo "✗ Erro no target MAUI"
fi

# 3. Teste Full Stack
echo "3. Testando target Full Stack..."
./target/release/compilador-portugues teste.pr --target=fullstack
if [ -f "teste/teste.sln" ]; then
    echo "✓ Target Full Stack funcionou"
    cd teste && dotnet build && cd ..
    if [ $? -eq 0 ]; then
        echo "✓ Solução Full Stack compila no .NET"
    else
        echo "⚠ Solução gerada mas não compila no .NET"
    fi
else
    echo "✗ Erro no target Full Stack"
fi

echo "=== Testes Concluídos ==="