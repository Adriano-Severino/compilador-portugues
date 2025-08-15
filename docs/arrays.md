# Arrays

Este documento descreve o suporte atual a arrays na linguagem Por do Sol.

## Visão geral
- Declaração: `tipo[] nome` ou `var nome = [itens]` (inferência)
- Literal: `[expr1, expr2, ...]`
- Indexador: `arr[i]` para ler; `arr[i] = valor` para escrever
- Tamanho:
  - Propriedade: `arr.tamanho` ou `arr.comprimento`
  - Método (alias): `arr.tamanho()` ou `arr.comprimento()`
- Verificação de limites: acessos fora do intervalo são verificados em tempo de execução (erro)

## Exemplos
```pordosol
função Principal() {
    inteiro[] numeros = [2, 3, 1];
    imprima(numeros.tamanho);     // 3
    imprima(numeros[0]);          // 2
    numeros[2] = 5;
    imprima(numeros[2]);          // 5
}
```

### Inferência por interface/base comum
É possível criar arrays com objetos de tipos diferentes que compartilham uma interface ou classe base. O tipo do array será inferido para a interface/base comum.

```pordosol
publico interface IAnimal { publico vazio Falar(); }
publico classe Cachorro : IAnimal { publico vazio Falar() { imprima("Au!"); } }
publico classe Papagaio : IAnimal { publico void Falar() { imprima("Currr!"); } }

função Principal() {
    var animais = [novo Cachorro(), novo Papagaio()]; // Tipo: IAnimal[]
    inteiro i = 0;
    enquanto (i < animais.tamanho) {
        animais[i].Falar();
        i = i + 1;
    }
}
```

## Regras de tipo
- Todos os elementos do literal precisam ser atribuíveis a um mesmo tipo de elemento.
  - Preferência: tipo idêntico → classe base comum → interface(s) comum(ns).
- `arr[i]` tem tipo do elemento; `arr.tamanho`/`arr.comprimento` tem tipo `inteiro`.

## Backends
- LLVM: arrays possuem cabeçalho com tamanho e dados; o código gera checagem de limites.
- Bytecode: indexador leitura/escrita e tamanho são suportados no interpretador.

## Erros comuns
- "Tipos incompatíveis nos elementos do array": ajuste os elementos para um tipo comum (classe base ou interface).
- "Índice fora dos limites": garanta `0 <= i < arr.tamanho`.
