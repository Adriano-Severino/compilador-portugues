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
# O compilador irá procurar os fontes em ../sistema-padrao/src e cuspir o resultado em ../sistema-padrao/dist/sistema.pbc
cargo run --bin compilador -- --compilar-biblioteca=$SISTEMA_PADRAO_PATH

if [ $? -eq 0 ]; then
    echo "Biblioteca padrão compilada com sucesso em $SISTEMA_PADRAO_PATH/dist/sistema.pbc"
else
    echo "Erro ao compilar a biblioteca padrão."
fi
