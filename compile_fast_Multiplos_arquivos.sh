#!/bin/bash
# ARQUIVOS_PR will contain all arguments passed to the script
ARQUIVOS_PR=("$@")

if [ ${#ARQUIVOS_PR[@]} -eq 0 ]; then
    echo "Uso: $0 <arquivo1.pr> [arquivo2.pr ...]"
    exit 1
fi

# Validate all input files
for file in "${ARQUIVOS_PR[@]}"; do
    if [ ! -f "$file" ]; then
        echo "Erro: Arquivo $file não encontrado"
        exit 1
    fi
    if [[ ! "$file" =~ \.pr$ ]]; then
        echo "Erro: Arquivo $file não tem a extensão .pr"
        exit 1
    fi
done

echo "Compilando ${ARQUIVOS_PR[@]}..."

# Usar o compilador já construído
# Pass all .pr files to the compiler with the bytecode target
./target/release/compilador.exe "${ARQUIVOS_PR[@]}" --target=bytecode

# Get the base name of the first file for the output bytecode
# This assumes the main program is the first one listed
FIRST_FILE_BASE_NAME=$(basename "${ARQUIVOS_PR[0]}" .pr)

echo "Pronto! Bytecode gerado em ${FIRST_FILE_BASE_NAME}.pbc"
echo "Para executar: cargo run --bin interpretador -- ${FIRST_FILE_BASE_NAME}.pbc"