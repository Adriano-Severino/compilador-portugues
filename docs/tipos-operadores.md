# Tipos, Variáveis e Operadores

## Tipos primitivos
- `inteiro` (i64)
- `duplo` (f64)
- `decimal` (decimal com sufixo `m`)
- `texto` (string)
- `booleano` (verdadeiro/falso)
- `var` (inferência de tipo)

## Declaração e atribuição
```pordosol
inteiro idade = 25;
texto nome = "Maria";
booleano ativo = verdadeiro;
var total = 10 + 5;
```

## Operadores
- Aritméticos: `+ - * / %`
- Comparação: `> < >= <= == !=`
- Lógicos: `&& || !`

## Conversões e impressão
- Conversões implícitas entre tipos numéricos podem não ser permitidas; use tipos consistentes.
- `imprima(expr)` imprime números, texto e bool; enums são mostrados como inteiro.
