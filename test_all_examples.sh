#!/bin/bash

# Diretório onde os exemplos estão localizados
EXEMPLOS_DIR="exemplos"

# Diretório de saída para o bytecode
OUTPUT_DIR="bytecode_output"
mkdir -p $OUTPUT_DIR

# Limpar o diretório de saída
rm -f $OUTPUT_DIR/*

# Iterar sobre todos os arquivos .pr no diretório de exemplos
for file in $EXEMPLOS_DIR/*.pr; do
    filename=$(basename -- "$file")
    filename_no_ext="${filename%.*}"
    bytecode_file="$OUTPUT_DIR/$filename_no_ext.pbc"
    output_file="$OUTPUT_DIR/$filename_no_ext.txt"

    echo "Compilando $file para bytecode..."
    cargo run --bin compilador -- "$file" --target=bytecode -o "$bytecode_file"
    
    if [ -f "$bytecode_file" ]; then
        echo "Executando $bytecode_file..."
        cargo run --bin interpretador -- "$bytecode_file" > "$output_file"
        echo "Saída salva em $output_file"
    else
        echo "Falha na compilação de $file"
    fi
    echo "----------------------------------------"
done

echo "Testes concluídos."
