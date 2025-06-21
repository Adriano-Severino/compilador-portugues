#!/bin/bash
echo "=== Testando Compilador Português ==="

echo
echo "📦 Teste Universal"
cargo run -- teste.pr --target=universal
if [ $? -ne 0 ]; then
    echo "❌ Falha no target universal"
    exit 1
fi

echo
echo "🖥️ Teste Console"
cargo run -- teste.pr --target=console
if [ $? -ne 0 ]; then
    echo "❌ Falha no target console"
    exit 1
fi

echo
echo "🔧 Teste LLVM IR"
cargo run -- teste.pr --target=llvm-ir
if [ $? -ne 0 ]; then
    echo "❌ Falha no target llvm-ir"
    exit 1
fi

echo
echo "⚙️ Teste CIL Bytecode"
cargo run -- teste.pr --target=cil-bytecode
if [ $? -ne 0 ]; then
    echo "❌ Falha no target cil-bytecode"
    exit 1
fi

echo
echo "💾 Teste Bytecode"
cargo run -- teste.pr --target=bytecode
if [ $? -ne 0 ]; then
    echo "❌ Falha no target bytecode"
    exit 1
fi

echo
echo "🎉 Todos os targets testados com sucesso!"
