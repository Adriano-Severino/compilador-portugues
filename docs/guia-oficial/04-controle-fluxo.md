# Capítulo 3: Controle de Fluxo

Para um programa não ser apenas uma lista de ações burras tocadas de cima pra baixo, ele precisa tomar decisões.

## Tomando Decisões (`se` e `senão`)

Podemos instruir o código a agir diferente com base em condições lógicas, usando `se` (if) e `senão` (else).

```pordosol
inteiro idade = 25;

se (idade >= 18) {
    imprima("Você é maior de idade!");
} senão {
    imprima("Você é menor de idade!");
}
```

Se tivermos múltiplas possibilidades, podemos encadear `senão se` (else if):

```pordosol
var hora = 15;

se (hora < 12) {
    imprima("Bom dia!");
} senão se (hora < 18) {
    imprima("Boa tarde!");
} senão {
    imprima("Boa noite!");
}
```

## Operadores Relacionais
Você usará esses operadores o tempo todo para montar as lógicas do `se`:
- `==` (Igual a)
- `!=` (Diferente de)
- `>` (Maior que)
- `<` (Menor que)
- `>=` (Maior ou igual a)
- `<=` (Menor ou igual a)

## Repetições (Loops)

Imagine que você quer mostrar os números de 1 até 100 na tela. Fazer isso manualmente com 100 linhas de comando seria terrível!
Para isso usamos os laços de repetição.

### O laço `enquanto` (while)
O laço `enquanto` executa o bloco de código "enquanto a condição for verdadeira":

```pordosol
inteiro contador = 1;

enquanto (contador <= 5) {
    imprima($"Passo número {contador}");
    contador = contador + 1; // Nunca esqueça de alterar a variável de controle, senão o programa trava em um loop infinito!
}
```

### O laço `para` (for)
Pôr do Sol também suporta o clássico `para`, que concentra a declaração, a condição e o passo numa única linha.

```pordosol
para (inteiro i = 1; i <= 5; i = i + 1) {
    imprima($"Iteração número {i}");
}
```

Com decisões lógicas e repetições, você consegue montar qualquer algoritmo matemático imaginável. Siga para o [Capítulo 4: Orientação a Objetos](05-classes-objetos.md) para organizar seus grandes sistemas profissionais.
