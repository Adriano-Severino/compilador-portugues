# Interfaces e Polimorfismo

Este documento cobre suporte a interfaces, implementação em classes e polimorfismo.

## Sintaxe
- Declaração:
```pordosol
publico interface IAnimal {
    publico vazio Falar();
}
```
- Implementação (estilo C# via dois-pontos, sem `implementa`):
```pordosol
publico classe Cachorro : IAnimal { /* ... */ }
publico classe Carro : Veiculo, IMovel, IApresentavel { /* ... */ }
```

## Regras
- Classes podem implementar múltiplas interfaces e herdar de uma única classe base.
- Métodos de interface devem ser implementados como `publico` na classe concreta.
- Palavras-chave:
  - `abstrata` em classes/métodos: requer sobrescrita em derivadas.
  - `redefinível` em métodos de classe base: permite `sobrescreve` em derivadas.
  - `sobrescreve` em métodos: marca implementação que substitui um método `redefinível`.

## Polimorfismo
- Chamada dinâmica via vtable em classes com métodos `redefinível/sobrescreve`.
- Conversão implícita classe→interface quando a classe implementa a interface.
- Arrays de interfaces suportados (ver Arrays).

## Exemplo básico
```pordosol
publico interface IPessoa { publico vazio Falar(); }
publico classe Pessoa : IPessoa { publico vazio Falar() { imprima("oi"); } }

publico função vazio Principal() {
    var p = novo Pessoa();
    p.Falar();
}
```

## Exemplo avançado
Trecho adaptado de `exemplos/interfaces_avancado.pr` mostrando múltiplas interfaces, herança abstrata e polimorfismo em arrays.

```pordosol
publico abstrata classe AnimalBase : IAnimal {
    publico abstrata vazio Falar();
    publico texto Categoria() { retorne "Animal"; }
}

publico classe Cachorro : AnimalBase, IMovel, IApresentavel, IComedor {
    publico sobrescreve vazio Falar() { imprima("Au au!"); }
    publico inteiro Mover(inteiro d) { retorne d; }
    publico texto Apresentar(texto nome) { retorne "Cão"; }
    publico vazio Comer(texto comida) { }
}

publico função vazio Principal() {
    var falantes = [novo Cachorro(), /* ... */]; // IAnimal[]
    inteiro i = 0;
    enquanto (i < falantes.tamanho) { falantes[i].Falar(); i = i + 1; }
}
```

## Backends
- LLVM: classes com ponteiro de vtable; despacho virtual; conversão classe→interface via ponteiro de vtable compatível.
- Bytecode: tabela de métodos na VM; chamadas resolvidas em tempo de execução.

## Erros comuns
- "método X não implementado para interface Y": faltou implementar na classe.
- "sobrescreve sem redefinível": marque o método base como `redefinível`.
