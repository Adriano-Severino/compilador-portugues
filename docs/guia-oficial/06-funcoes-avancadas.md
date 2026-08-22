# Capítulo 5: Funções e Recursos Avançados

Conforme os sistemas Pôr do Sol crescem, os desenvolvedores precisam de ferramentas modernas para escalar.

## Arrow Functions (Sintaxe encurtada)
Para métodos e funções que só têm um retorno de única linha, usar a palavra `retorne` com chaves `{}` toma muito espaço de tela.

Pôr do Sol introduz a sintaxe de *Arrow Functions*:
```pordosol
// Normal
publico função multiplicar(inteiro a, inteiro b) => inteiro {
    retorne a * b;
}

// Com arrow function (se você colocar as chaves com '=>', ele ainda exige o retorno mas fica alinhado)
publico função principal() => vazio {
    imprima("Testando!");
}
```

## Programação Assíncrona (`assíncrona` e `aguarde`)
Ler de um banco de dados, acessar uma API web ou ler um disco rígido físico é um processo absurdamente demorado pros processadores atuais. Para o programa não ficar travado esperando (e deixar a janela congelada), usamos a Programação Assíncrona (Async/Await).

No Pôr do Sol, o suporte já está built-in.
```pordosol
usando Sistema;

publico assíncrona função processarLongo() {
    imprima("Iniciando carregamento do disco local...");
    
    // A palavra 'aguarde' pausa ESSA função, devolvendo o controle da thread para o SO
    var conteudoDoArquivo = aguarde LerArquivoAssíncrono("arquivo_gigante.txt");
    
    // Quando o arquivo estiver pronto, a função recomeça daqui
    imprima($"O arquivo tem as informações: {conteudoDoArquivo}");
}
```

## Atributos de Metadados
Se você estiver criando um site na Web ou testando um framework, pode rotular funções e classes usando metadados. São as etiquetas entre chaves anguladas, que não alteram o código, mas permitem que bibliotecas o descubram.

```pordosol
[Rota("/api/clientes/listar")]
publico função listarClientes() => vazio {
    imprima("Devolvendo JSON para a Web!");
}

[Autor("Adriano Severino")]
publico classe ControleDeVendas {
    // ...
}
```

## Próximos Passos
Você acaba de concluir a leitura do Guia Oficial Pôr do Sol!
Você viu toda a escalabilidade e o poder nativo para construir do básico ao sistema web complexo. 
Recomendamos a prática! Vá direto para os repositórios, baixe o compilador `compilador-portugues`, crie os seus arquivos `.pr`, rode no Interpretador de Depuração para ver a abstração agindo, e rode pelo target do compilador `llvm-ir` para ver o código de máquina nativo acelerado ser ejetado para produção!

O mundo é o seu limite. Feliz codificação!
