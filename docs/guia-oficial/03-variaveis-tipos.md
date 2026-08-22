# Capítulo 2: Variáveis e Tipos de Dados

Variáveis são espaços na memória do computador onde armazenamos dados que vamos usar depois. Como Pôr do Sol possui **Tipagem Estática Forte**, o computador sempre quer saber exatamente o que está sendo guardado em cada gaveta.

## Tipos Primitivos

Os tipos mais comuns já estão embutidos na linguagem:

- `inteiro`: Guarda números sem casas decimais (Ex: `-1`, `0`, `42`).
- `flutuante`, `duplo`, `decimal`: Guarda números com casas decimais (Ex: `3.14`). O decimal é ótimo para valores financeiros.
- `booleano`: Guarda valores verdadeiros ou falsos (`verdadeiro` ou `falso`).
- `texto`: Guarda letras, palavras e frases (sempre entre aspas duplas `"..."`).

### Declarando variáveis
```pordosol
inteiro idade = 25;
texto nome = "Maria";
booleano contaAtiva = verdadeiro;
decimal saldoEmConta = 1500.50;
```

## Inferência de Tipos (var)

Às vezes é chato e repetitivo escrever o tipo da variável quando está na cara qual valor ela vai receber. Para isso existe a palavra-chave `var`.
```pordosol
var cidade = "São Paulo"; // O compilador sabe que é texto!
var populacao = 12000000; // O compilador sabe que é inteiro!
```
> [!NOTE]
> Quando usamos `var`, nós ainda estamos usando Tipagem Forte. Você não poderá atribuir um booleano `falso` para a variável `cidade` no futuro. O `var` apenas pede pro compilador descobrir qual é o tipo pela primeira atribuição que você fez!

## Operadores Matemáticos
Você tem as quatro operações matemáticas básicas e mais algumas à sua disposição:
```pordosol
inteiro soma = 10 + 5;        // 15
inteiro subtracao = 10 - 5;   // 5
inteiro multiplicacao = 10 * 5; // 50
inteiro divisao = 10 / 5;     // 2
inteiro restoDaDivisao = 10 % 3; // 1
```

## Operadores Lógicos
Você pode fazer perguntas pro computador (se uma coisa for isso E aquilo).
```pordosol
// "E" lógico
booleano eMaiorIdade = verdadeiro && verdadeiro; 

// "OU" lógico
booleano podeEntrar = verdadeiro || falso;

// "NÃO" (inversão)
booleano naoPode = !verdadeiro; // Fica falso!
```

## Strings Interpoladas

Pôr do Sol torna unir texto e variáveis super fácil através da *interpolação*. Colocando um `$` na frente da string e usando chaves `{}` pra injetar variáveis:
```pordosol
var nome = "João";
var idade = 30;

imprima($"Olá, me chamo {nome} e tenho {idade} anos."); 
// Saída: Olá, me chamo João e tenho 30 anos.
```

Com isso em mãos, você já pode salvar valores e fazer cálculos complexos. Pule para o [Capítulo 3: Controle de Fluxo](04-controle-fluxo.md) e dê inteligência e capacidade de decisão para o seu programa!
