#!/bin/bash
PROGRAMA=$1

if [ -z "$PROGRAMA" ]; then
    echo "Uso: $0 <arquivo_sem_extensão>"
    exit 1
fi

if [ ! -f "$PROGRAMA.pr" ]; then
    echo "Erro: Arquivo $PROGRAMA.pr não encontrado"
    exit 1
fi

echo "Compilando $PROGRAMA.pr para bytecode..."
cargo run --bin compilador -- $PROGRAMA.pr --target=bytecode

echo "Executando $PROGRAMA.pbc com o interpretador..."
cargo run --bin interpretador -- $PROGRAMA.pbc