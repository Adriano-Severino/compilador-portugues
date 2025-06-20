#!/bin/bash
set -e  # Para no primeiro erro

PROGRAMA=$1

if [ -z "$PROGRAMA" ]; then
    echo "Uso: $0 <nome_do_programa>"
    echo "Exemplo: $0 exemplo  (para exemplo.pr)"
    exit 1
fi

if [ ! -f "$PROGRAMA.pr" ]; then
    echo "Erro: Arquivo $PROGRAMA.pr não encontrado!"
    exit 1
fi

echo "=== Compilando $PROGRAMA.pr ==="

# 1. Compilar com seu compilador
echo "1. Gerando LLVM IR..."
cargo run --release --bin compilador -- $PROGRAMA.pr
if [ $? -ne 0 ]; then
    echo "Erro: Falha na compilação do arquivo .pr"
    exit 1
fi

# 2. Verificar se .ll foi gerado
if [ ! -f "$PROGRAMA.ll" ]; then
    echo "Erro: Arquivo $PROGRAMA.ll não foi gerado!"
    exit 1
fi

# 3. Otimizar
echo "2. Otimizando código..."
opt -O3 -S $PROGRAMA.ll -o ${PROGRAMA}_opt.ll

# 4. Compilar para executável
echo "3. Gerando executáveis..."
clang -O3 -march=native ${PROGRAMA}_opt.ll -o $PROGRAMA

# 5. Executável estático
echo "4. Gerando executável estático..."
clang -static -O3 ${PROGRAMA}_opt.ll -o ${PROGRAMA}_static 2>/dev/null || {
    echo "Aviso: Não foi possível criar executável estático"
}

echo "=== Compilação concluída! ==="
echo "Arquivos gerados:"
ls -lh $PROGRAMA $PROGRAMA.ll ${PROGRAMA}_opt.ll 2>/dev/null
[ -f "${PROGRAMA}_static" ] && echo "- ${PROGRAMA}_static (estático)"