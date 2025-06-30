#!/bin/bash
# Script para testar todos os arquivos .pr da pasta exemplos/ com o compilador e interpretador

set -e

EXEMPLOS_DIR="exemplos"
COMPILADOR="./target/release/compilador.exe"
INTERPRETADOR="cargo run --bin interpretador --"

# Verifica se o compilador existe
if [ ! -f "$COMPILADOR" ]; then
    echo "Compilador não encontrado em $COMPILADOR. Compile o projeto antes."
    exit 1
fi

# Para cada arquivo .pr na pasta exemplos/
for pr_file in "$EXEMPLOS_DIR"/*.pr; do
    base_name=$(basename "$pr_file" .pr)
    echo "Compilando $pr_file..."
    $COMPILADOR "$pr_file" --target=bytecode
    if [ $? -ne 0 ]; then
        echo "FALHA ao compilar $pr_file"
        exit 1
    fi
    echo "Executando bytecode ${base_name}.pbc..."
    $INTERPRETADOR "${base_name}.pbc"
    if [ $? -ne 0 ]; then
        echo "FALHA ao executar ${base_name}.pbc"
        exit 1
    fi
    echo "OK: $pr_file"
done

echo "Todos os testes foram executados com sucesso!"
