# Funções, Métodos e Parâmetros Opcionais

## Funções de topo
```pordosol
publico função bemvindo() { imprima("Olá mundo"); }
publico função inteiro calcular() { retorne 42; }
```

## Funções Encurtadas (Arrow Functions)
```pordosol
publico função multiplicar(inteiro a, inteiro b) => inteiro { retorne a * b; }
publico função principal() => vazio { imprima("Iniciando..."); }
```

## Funções Assíncronas (async/await)
A linguagem suporta concorrência e operações baseadas em *Tasks* utilizando o modelo `assíncrona`/`aguarde`.
```pordosol
usando Sistema;

publico assíncrona função vazio processarDados() {
    imprima("Processando...");
    var conteudo = aguarde LerArquivoAssíncrono("dados.txt");
    imprima($"Conteúdo lido: {conteudo}");
}
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

## Atributos de Metadados
É possível decorar métodos, funções e parâmetros com atributos de metadados, no formato `[NomeAtributo("Valor")]`.
```pordosol
[Rota("/api/teste")]
publico função vazio endpointTeste() {
    imprima("Acessou a rota!");
}
```

## Strings interpoladas
```pordosol
imprima($"Nome: {Nome}, Idade: {Idade}");
```

## Namespaces (espaços)
```pordosol
espaco Meu.App { publico classe Pessoa { /* ... */ } }
```
