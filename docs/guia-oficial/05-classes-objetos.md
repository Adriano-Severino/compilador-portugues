# Capítulo 4: Orientação a Objetos

Orientação a Objetos (OO) é a forma que os humanos e os computadores encontraram para traduzir o mundo real para dentro do software. O Pôr do Sol suporta fortemente este paradigma, muito parecido com a forma feita em C#.

## Classes e Objetos
Uma `classe` é como se fosse um molde, uma planta-baixa (um blueprint).
A partir desse molde, você pode criar dezenas de `objetos` independentes que estarão na memória.

```pordosol
publico classe Carro {
    // Propriedades são as "características" do objeto
    publico texto Marca { obter; definir; }
    publico texto Modelo { obter; definir; }
    publico inteiro Ano { obter; definir; }

    // Métodos são as "ações" do objeto
    publico vazio buzinar() {
        imprima("Bibi!");
    }
}
```

Para instanciar o objeto na vida real e usá-lo na sua função principal, você usa a palavra `novo`:
```pordosol
publico função Principal() {
    var meuCarro = novo Carro();
    meuCarro.Marca = "Fiat";
    meuCarro.Modelo = "Uno";
    meuCarro.Ano = 2010;
    
    meuCarro.buzinar();
}
```

## Construtores
Podemos forçar que os dados já sejam preenchidos na hora que chamamos a palavra `novo` usando Construtores:

```pordosol
publico classe Carro {
    publico texto Modelo { obter; definir; }
    publico inteiro Ano { obter; definir; }

    // O Construtor é uma função especial que tem o exato mesmo nome da classe
    publico Carro(texto modeloParam, inteiro anoParam) {
        Modelo = modeloParam;
        Ano = anoParam;
    }
}

// Criando com construtor
var c1 = novo Carro("Ferrari", 2024);
```

## Herança
Sistemas robustos não se repetem. Em Pôr do Sol, você pode criar hierarquias de classes usando o sinal de `:`.

```pordosol
publico classe Animal {
    publico texto Nome { obter; definir; }
    publico redefinível vazio emitirSom() {
        imprima("Som genérico de animal...");
    }
}

// O Cachorro herda (é um) Animal
publico classe Cachorro : Animal {
    publico sobrescreve vazio emitirSom() {
        imprima("Au Au!");
    }
}
```
Na linguagem Pôr do Sol, por padrão a segurança é máxima: métodos que podem ser reescritos nos "filhos" (subclasses) precisam ser marcados explicitamente como `redefinível` (virtual no C#), e o filho que os modifica precisa dizer explicitamente `sobrescreve` (override no C#).

Vá para o último capítulo [Capítulo 5: Funções Avançadas](06-funcoes-avancadas.md) para explorar concorrência, lambdas, e os metadados!
