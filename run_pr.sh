#!/bin/bash

# Verifica se um nome de arquivo foi passado como argumento
if [ -z "$1" ]; then
  echo "Uso: ./run_pr.sh <caminho/para/arquivo.pr>"
  exit 1
fi

INPUT_FILE=$1
# Determina o diretório e o nome base do arquivo de entrada sem a extensão .pr
DIR_OF_INPUT=$(dirname "$INPUT_FILE")
BASENAME_WITHOUT_PR_EXT=$(basename "$INPUT_FILE" .pr)

# Constrói o caminho para o arquivo .ll esperado
# Se DIR_OF_INPUT for '.', significa que o arquivo está no diretório atual
if [ "$DIR_OF_INPUT" == "." ]; then
  LL_FILE_PATH="${BASENAME_WITHOUT_PR_EXT}.ll"
else
  LL_FILE_PATH="$DIR_OF_INPUT/${BASENAME_WITHOUT_PR_EXT}.ll"
fi

# Caminho para o seu compilador (relativo à raiz do projeto)
MEU_COMPILADOR_EXE="./target/debug/meu_compilador"

echo "Compilando $INPUT_FILE com meu_compilador..."
$MEU_COMPILADOR_EXE "$INPUT_FILE"

# Verifica se a compilação do .pr para .ll foi bem-sucedida
if [ $? -ne 0 ]; then
  echo "Erro ao compilar $INPUT_FILE com meu_compilador."
  exit 1
fi

if [ ! -f "$LL_FILE_PATH" ]; then
    echo "Arquivo .ll ($LL_FILE_PATH) não encontrado após compilação. Verifique o nome gerado."
    exit 1
fi

echo "Executando $LL_FILE_PATH com lli..."
lli "$LL_FILE_PATH"