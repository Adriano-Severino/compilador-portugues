#!/bin/bash
set -e

echo "🚀 === Teste Completo do Compilador Português ==="

echo "📋 1. Compilando projeto..."
cargo build

echo "📋 2. Executando testes unitários..."
cargo test --lib

echo "📋 3. Executando testes de integração..."
cargo test --test integration_tests

echo "📋 4. Testando todos os targets..."
./testar_targets.sh

echo "📋 5. Verificando arquivos gerados..."
if [ -f "teste.ll" ]; then
    echo "✓ LLVM IR gerado"
fi
if [ -f "teste.il" ]; then
    echo "✓ CIL gerado"
fi
if [ -f "teste.js" ]; then
    echo "✓ JavaScript gerado"
fi
if [ -d "teste_Console" ]; then
    echo "✓ Projeto Console gerado"
fi

echo "📋 6. Testando execução C# (se dotnet disponível)..."
if command -v dotnet &> /dev/null; then
    if [ -d "teste_Console" ]; then
        cd teste_Console
        dotnet run
        cd ..
    fi
fi

echo "📋 7. Testando execução JavaScript (se node disponível)..."
if command -v node &> /dev/null; then
    if [ -f "teste.js" ]; then
        node teste.js
    fi
fi

echo "🎉 Teste completo finalizado com sucesso!"
