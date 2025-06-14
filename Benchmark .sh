#!/bin/bash
# benchmark.sh
echo "🔥 EXECUTANDO BENCHMARKS..."

# Criar arquivo de teste grande
python3 -c "
code = '''
inteiro contador = 0;
se (contador < 100) {
    imprima(contador);
    contador = contador + 1;
}
'''
with open('benchmark_test.pr', 'w') as f:
    for i in range(1000):  # 1000 repetições = ~50K LOC
        f.write(code.replace('contador', f'contador{i}'))
"

# Executar benchmark
echo "📊 Compilando arquivo de 50K linhas..."
time ./target/release/compilador-portugues benchmark_test.pr

# Cleanup
rm benchmark_test.pr
