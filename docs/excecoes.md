# Exceções (`tente` / `capture` / `finalmente` / `falhe`)

> **Status:** lexer, parser e AST implementados (A.3.2) — programas com `tente`/`capture`/`finalmente`/`falhe` **parseiam**. Type checker valida estrutura básica; execução nos backends está planejada em A.3.4–A.3.7.

Por do Sol adota um modelo de exceções inspirado em C#: exceções são objetos de classes que herdam de `Sistema.Erros.Erro`, podem ser lançadas com `falhe`, capturadas por tipo com `capture` e sempre têm um bloco `finalmente` executado em todos os caminhos.

## Sintaxe básica

```pordosol
usando Sistema.Erros;

tente {
    inteiro numero = inteiro.Parse("abc");
    imprima(numero);
} capture (ErroNumero e) {
    imprima("Falha ao converter: " + e.Mensagem);
} finalmente {
    imprima("Fim da operação");
}
```

## `tente` — bloco protegido

`tente { ... }` delimita o código que pode lançar exceções. Deve ser seguido de **pelo menos um** `capture` ou `finalmente`.

## `capture` — captura por tipo

`capture (TipoExcecao nome)` captura exceções do tipo declarado **ou de tipos derivados** (mesma semântica de `catch` do C#). Múltiplos `capture` são avaliados em ordem:

```pordosol
capture (ErroIo e) {
    imprima("Erro de I/O: " + e.Mensagem);
} capture (Erro e) {
    imprima("Erro genérico: " + e.Mensagem);
}
```

- **Ordem obrigatória:** do tipo mais específico para o mais geral. `capture (Erro e)` antes de `capture (ErroIo e)` é erro de compilação (o segundo seria inalcançável).
- **Sem tipo:** `capture ()` captura qualquer exceção, sem binding (para relançar com `falhe`, declare o tipo base: `capture (Erro e)` com `usando Sistema.Erros`).
- **Sem binding:** `capture (ErroIo)` captura por tipo sem nomear a exceção, útil quando só interessa o tipo.

### Filtro opcional

`capture ... quando (condicao)` adiciona um filtro à captura (equivalente ao `when` do C#):

```pordosol
capture (ErroIo e) quando (e.Codigo == 2) {
    imprima("Arquivo não encontrado");
}
```

## `finalmente` — limpeza garantida

`finalmente { ... }` executa **sempre**, em todos os caminhos:

- após o bloco `tente` terminar normalmente;
- após um `capture` executar;
- quando uma exceção **não capturada** propaga para fora;
- quando `retorne` é executado dentro de `tente` ou `capture` (o `finalmente` roda antes do retorno efetivo).

```pordosol
função inteiro LerArquivo(texto caminho) {
    Arquivo arquivo = Arquivo.Abrir(caminho);
    tente {
        retorne arquivo.LerTudo();
    } capture (ErroIo e) {
        retorne -1;
    } finalmente {
        arquivo.Fechar();   // executa mesmo com retorne
    }
}
```

## `falhe` — lançamento

`falhe` lança uma exceção. Duas formas:

```pordosol
falhe "mensagem";                       // atalho: falhe novo Erro("mensagem")
falhe novo ErroIo("falha ao abrir", 2); // lança uma instância explícita
falhe e;                                // relança a exceção capturada `e`
```

- `falhe "texto"` cria e lança `Sistema.Erros.Erro` com a mensagem.
- `falhe expressao` lança a instância produzida pela expressão; o tipo deve ser `Erro` ou derivado.
- `falhe e;` dentro de um `capture` que nomeia `e` relança a exceção capturada (equivalente a `throw;` do C#, preservando a pilha original). `falhe;` sem expressão relança a exceção em captura dentro de um `capture`/`tente` aninhado.

## Exceção não capturada

Se nenhuma `capture` corresponde ao tipo lançado, a exceção propaga para o chamador. No nível do programa, a exceção não tratada encerra a execução com mensagem de erro e pilha de chamadas:

```
erro: ErroIo: falha ao abrir
  em LerArquivo (arquivo.pr:12)
  em Principal (arquivo.pr:30)
```

## Hierarquia de exceções (`Sistema.Erros`)

Exceções são classes normais. Toda exceção herda de `Sistema.Erros.Erro`, que expõe:

| Membro | Tipo | Descrição |
|---|---|---|
| `Mensagem` | `texto` | Mensagem descritiva do erro |
| `Pilha` | `texto` | Pilha de chamadas no momento do lançamento |

Hierarquia inicial proposta (implementada em A.3.8):

```
Sistema.Erros.Erro            — erro genérico (base de todas)
├── ErroArgumento             — argumento inválido
├── ErroIndice                — índice fora dos limites
├── ErroNulo                  — acesso a membro de referência nula
├── ErroOperacao              — operação inválida (divisão por zero etc.)
├── ErroIo                    — falhas de entrada/saída (arquivo, rede)
├── ErroFormato               — falha de parsing/conversão (JSON, número)
└── ErroEstado                — estado inválido do objeto
```

## Regras e restrições

- `tente` exige ao menos um `capture` ou `finalmente`.
- `capture`/`finalmente` só aparecem como parte de um `tente`; não são válidos isoladamente.
- O tipo do `capture` deve ser `Erro` ou tipo derivado; capturar tipo não relacionado é erro de compilação.
- O binding do `capture` (ex.: `e`) tem o tipo declarado e só existe dentro do bloco.
- Exceções não são valores de primeira classe: não podem ser armazenadas em variáveis fora de `capture`, passadas como argumento ou retornadas (só relançadas com `falhe`).
- `falhe` só aceita `Erro` ou derivado; `falhe 42` é erro de compilação.
- `retorne` dentro de `capture`/`tente` executa `finalmente` antes de efetivamente retornar (semântica C#).

## Notas de implementação (design, não especificação de backend)

Referências para as tarefas seguintes:

- **A.3.2 (lexer/parser/AST) — implementado:** tokens `tente`, `capture`, `finalmente`, `falhe`, `quando`; AST `Comando::Tente { bloco, captures, bloco_finalmente }` e `Comando::Falhe(Option<Expressao>)`, com `CaptureExcecao { tipo, nome, filtro, corpo }`. Relançamento usa `falhe e;`/`falhe;` (não foi criada keyword `lance`).
- **A.3.3 (type checker):** validar ordem dos captures (tipo mais específico primeiro), tipo do binding, tipo do argumento de `falhe`, e resolução de `Sistema.Erros.*`.
- **A.3.4 (runtime):** exceção carrega tipo, mensagem, pilha de chamadas e referência de causa; `finally` executa em todos os caminhos incluindo retorno e exceção não capturada.
- **A.3.5 (bytecode):** opcodes de throw/catch ou tabela de handlers no `.pbc`; interpretador mantém pilha de exceção em modo debug.
- **A.3.6 (LLVM):** `invoke`/`landingpad`/`resume`/`cleanup`; personality function adequada.
- **A.3.7 (CIL):** regiões `.try`/`.catch`/`.finally` válidas executáveis no runtime .NET.
- **A.7 (async):** `assíncrona`/`aguarde` deve propagar exceções através de `Tarefa<T>`; exceção dentro de task é re-lançada no `aguarde`.
