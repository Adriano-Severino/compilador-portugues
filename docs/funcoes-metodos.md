# Funções, Métodos e Parâmetros Opcionais

## Funções de topo
```pordosol
publico função bemvindo() { imprima("Olá mundo"); }
publico função inteiro calcular() { retorne 42; }
```

## Métodos em classes
```pordosol
publico classe Pessoa {
    publico texto Nome { obter; definir; }
    publico vazio Apresentar() { imprima($"Nome: {Nome}"); }
}
```

## Parâmetros opcionais (C#-like)
```pordosol
classe Teste {
    publico vazio meuMetodo(texto msg = "valor padrao") { imprima(msg); }
}
publico função Principal() { novo Teste().meuMetodo(); }
```

## Strings interpoladas
```pordosol
imprima($"Nome: {Nome}, Idade: {Idade}");
```

## Namespaces (espaços)
```pordosol
espaco Meu.App { publico classe Pessoa { /* ... */ } }
```
