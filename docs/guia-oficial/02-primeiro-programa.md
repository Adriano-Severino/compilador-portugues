# Capítulo 1: Seu Primeiro Programa

Todo grande projeto começa com um simples e amigável "Olá, Mundo". Na linguagem Pôr do Sol não é diferente.

## Estrutura Básica

Um programa Pôr do Sol é escrito em arquivos de texto plano com a extensão `.pr`. Para estruturar nosso programa, normalmente o envolvemos em um **espaço** e usamos uma função especial que atua como ponto de entrada para execução, o `principal` (semelhante ao `main` do C#, C, e Java).

Crie um arquivo chamado `meu_programa.pr` com o seguinte código:

```pordosol
espaco MeuProjeto
{
    // A função Principal() é o ponto de partida de qualquer programa executável
    publico função Principal()
    {
        imprima("Olá, Mundo! Este é o meu primeiro programa em Pôr do Sol!");
    }
}
```

## Dissecando o código

- `espaco MeuProjeto`: Espaços servem para agrupar e organizar nosso código (semelhante aos *namespaces* do C# ou *packages* do Java). Se formos criar um grande sistema, poderíamos ter `espaco MeuProjeto.Usuarios` e `espaco MeuProjeto.Vendas`.
- `publico função Principal()`: Isso declara uma função que pode ser acessada globalmente (`publico`). A função com o nome exato `Principal` (com P maiúsculo) indica pro compilador que é por onde o aplicativo deve começar a rodar.
- `imprima("...")`: Uma função intrínseca (embutida) nativa da linguagem que escreve um texto diretamente no terminal.

## Comentários

Como você deve ter notado, podemos deixar anotações que o computador vai ignorar:
```pordosol
// Comentário de uma única linha. O computador ignora tudo à frente das barras.

/* 
   Comentário de múltiplas linhas. 
   Tudo aqui dentro é ignorado. 
*/
```

## Como executar?
Caso tenha o compilador instalado localmente, no terminal, você rodaria:

```bash
cargo run --bin compilador -- meu_programa.pr --target=llvm-ir
```
Isso gera o executável nativo do seu programa ultrarrápido!

Agora que você já fez o computador dizer "Olá", avance para [Capítulo 2: Variáveis e Tipos de Dados](03-variaveis-tipos.md) para dar vida aos seus aplicativos guardando valores.
