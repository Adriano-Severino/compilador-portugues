#!/bin/bash
# Script para compilar a biblioteca padrão 'sistema-padrao'

# Garante que o script está sendo executado do diretório 'compilador-portugues'
if [ ! -f "Cargo.toml" ]; then
    echo "Erro: Este script deve ser executado a partir do diretório 'compilador-portugues'"
    exit 1
fi

# ecoar "Compilando a biblioteca padrão..."

# O caminho para a biblioteca padrão é relativo ao diretório do compilador
SISTEMA_PADRAO_PATH="../sistema-padrao"

# Executa o compilador com a flag para compilar a biblioteca
# O compilador irá procurar os fontes em ../sistema-padrao/src e gerar:
# - ../sistema-padrao/dist/sistema.pbl (formato moderno)
# - ../sistema-padrao/dist/sistema.ll (LLVM IR)
cargo run --bin compilador -- --compilar-biblioteca=$SISTEMA_PADRAO_PATH

if [ $? -eq 0 ]; then
    echo "Biblioteca padrão compilada com sucesso em $SISTEMA_PADRAO_PATH/dist/"
    echo "  - sistema.pbl (formato moderno)"
    echo "  - sistema.ll (LLVM IR)"
else
    echo "Erro ao compilar a biblioteca padrão."
fi
