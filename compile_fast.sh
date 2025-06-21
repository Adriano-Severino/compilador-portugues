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

echo "Compilando $PROGRAMA.pr..."

# Usar o compilador já construído
./target/release/compilador-portugues $PROGRAMA.pr

# Gerar executável diretamente
clang -O3 $PROGRAMA.ll -o $PROGRAMA

echo "Pronto! Execute com: ./$PROGRAMA"
