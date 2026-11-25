# Controle de Fluxo

Estruturas de controle disponíveis atualmente.

## Condicionais: `se` / `senão`
```pordosol
inteiro a = 10;
inteiro b = 5;

se (a > b) {
    imprima("a é maior que b");
} senão {
    imprima("a não é maior que b");
}

inteiro idade = 25;
se (idade > 18)  {
    imprima("Maior de idade");
} 
senão se (idade == 18) {
    imprima("Tem 18 anos");
}
senão {
    imprima("Menor de idade");
}
```

## Laço: `enquanto`
```pordosol
inteiro contador = 0;
imprima("Iniciando contador...");

se (contador < 5) {
    imprima("Contador é menor que 5");
    contador = contador + 1;
    imprima(contador);
}
```

## Laço: `para`
```pordosol
para (inteiro i = 0; i < 5; i = i + 1) {
    imprima(i);
}
```

A inicialização, a condição e o incremento ficam no cabeçalho do `para`. A condição é opcional; sem ela, o laço continua até uma saída explícita do programa.

## Laço: `para cada`

Itera sobre cada elemento de uma coleção (`lista<T>`, array ou `texto`) sem precisar de índice manual:

```pordosol
var numeros = [10, 20, 30, 40];
var soma = 0;

para cada (n em numeros) {
    soma = soma + n;
}

imprima(soma); // 100
```

A variável `n` recebe o tipo do elemento da coleção e tem escopo limitado ao corpo do laço. `para cada` também pode ser aninhado:

```pordosol
var grades = [[1, 2], [3, 4]];
para cada (linha em grades) {
    para cada (valor em linha) {
        imprima(valor);
    }
}
```

Exemplo completo executável: `exemplos/para_cada.pr`.
