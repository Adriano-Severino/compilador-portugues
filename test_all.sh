#!/bin/bash

echo "=== Testando Compilador Modificado ==="

# Targets válidos
targets=("universal" "llvm-ir" "cil-bytecode" "console" "bytecode")

for target in "${targets[@]}"; do
    echo "Testando target: $target"
    if cargo run -- app.pr --target=$target; then
        echo "✅ $target: SUCESSO"
    else
        echo "❌ $target: FALHA"
    fi
    echo "---"
done

echo "=== Verificando targets removidos ==="
removed_targets=("maui-hybrid" "blazor-web" "api" "fullstack")

for target in "${removed_targets[@]}"; do
    echo "Verificando target removido: $target"
    if cargo run -- ./tests/basicos/app.pr --target=$target 2>/dev/null; then
        echo "❌ $target: DEVERIA TER SIDO REMOVIDO"
    else
        echo "✅ $target: CORRETAMENTE REMOVIDO"
    fi
done