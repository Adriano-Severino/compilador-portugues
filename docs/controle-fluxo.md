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

Obs.: a construção `para` poderá ser adicionada futuramente; use `enquanto` por ora.
