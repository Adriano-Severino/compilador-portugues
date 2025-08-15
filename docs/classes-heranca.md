# Classes, Herança e Modificadores

## Declarações
```pordosol
publico classe Veiculo { /* campos, propriedades, métodos */ }
publico classe Carro : Veiculo { /* herda de Veiculo */ }
publico abstrata classe Forma { publico abstrata duplo Area(); }
publico classe Retangulo : Forma { publico sobrescreve duplo Area() { retorne 10.0; } }
```

## Modificadores
- `publico`, `privado`, `protegido` em membros de classe.
- `estática` para membros/ classes estáticas.
- `abstrata` para classes e métodos abstratos.
- `redefinível` e `sobrescreve` para polimorfismo virtual.

## Propriedades
```pordosol
publico texto Nome { obter; definir; }
publico inteiro Idade { obter; privado definir; }
```

## Construtores
- Suportam parâmetros opcionais e chamada ao construtor base com `: base(...)`.
```pordosol
publico classe Carro : Veiculo {
    publico Carro(texto marca) : base(marca) { }
}
```

## Uso
```pordosol
var c = novo Carro("Toyota");
c.Info();
```

## Backends
- LLVM: estruturas com ponteiro de vtable + campos; construtores inicializam vptr e campos.
- Bytecode: objetos com mapa de campos e métodos.
