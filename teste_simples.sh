#!/bin/bash
echo "=== Teste Simplificado ==="

# Criar arquivo de teste básico
cat > teste_basico.pr << 'EOF'
publico classe Principal {
    publico vazio Main() {
        imprima("Olá, compilador português!");
    }
}
EOF

echo "📦 Testando target console..."
cargo run --bin compilador teste_basico.pr --target=console

echo "💾 Testando target bytecode..."
cargo run --bin compilador teste_basico.pr --target=bytecode

echo "🎯 Testando target universal..."
cargo run --bin compilador teste_basico.pr --target=universal

echo "✅ Teste simplificado concluído!"
