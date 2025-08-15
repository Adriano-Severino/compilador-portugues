# Depuração no Interpretador (Bytecode)

Este guia explica como habilitar o modo de depuração do interpretador de bytecode, usar os comandos interativos, e como funcionam os identificadores de código (code_id) para definir breakpoints.

Observações importantes:
- A depuração é suportada no interpretador de bytecode (binário `interpretador`).
- Os breakpoints são definidos por endereço de instrução (IP) dentro de um `code_id` específico.
- O modo de passo é StepInto: ao avançar passo a passo, chamadas entram em funções/métodos automaticamente.

## Início rápido

1) Compile seu programa `.pr` para bytecode `.pbc`:

```powershell
cargo run --bin compilador -- .\meu_programa.pr --target=bytecode
```

2) Execute o interpretador em modo depuração:

```powershell
cargo run --bin interpretador -- .\meu_programa.pbc --debug
```

3) (Opcional) Execute diretamente uma função específica (nome completo se aplicável):

```powershell
cargo run --bin interpretador -- .\meu_programa.pbc --executar-funcao Principal --debug
```

Quando um breakpoint é atingido ou o passo-a-passo está ativo, será exibido um prompt interativo `dbg>`.

## Comandos do depurador

- Execução
  - `c` | `cont` | `continue` — Continua a execução (desativa o step até o próximo breakpoint).
  - `s` | `step` | `next` | `n` — Executa uma instrução e pausa novamente (StepInto).
- Inspeção
  - `p` | `pilha` — Mostra o conteúdo da pilha da VM.
  - `vars` — Lista todas as variáveis visíveis (inclui `este` em métodos).
  - `v <nome>` — Mostra o valor de uma variável específica.
  - `dis [n]` — Exibe as próximas `n` instruções a partir da atual (padrão: 8). Mostra o IP, útil para escolher onde colocar breakpoints.
  - `where` — Mostra a posição atual: `code_id`, `ip` e a instrução corrente.
- Breakpoints
  - `bp add <ip>` — Adiciona breakpoint no `ip` atual do `code_id` ativo.
  - `bp add <code_id> <ip>` — Adiciona breakpoint em outro `code_id`.
  - `bp del <ip>` — Remove breakpoint no `ip` do `code_id` ativo.
  - `bp del <code_id> <ip>` — Remove breakpoint em outro `code_id`.
  - `bp list [code_id]` — Lista breakpoints do `code_id` informado ou do atual.
- Ajuda e saída
  - `help` | `?` — Lista os comandos.
  - `q` | `quit` | `exit` — Aborta a execução.

Dicas:
- Use `dis` para descobrir os IPs e definir breakpoints com precisão.
- Após usar `s` (step), o depurador volta a parar a cada instrução. Use `c` para retomar até o próximo breakpoint.

## Mapeamento de code_id

Cada unidade de código executável tem um identificador (`code_id`) que aparece nas mensagens do depurador e no comando `where`. Os principais formatos são:

- `global` — Código global do módulo de bytecode.
- `func:<Nome>` — Execução de uma função.
- `method:<Classe>::<Metodo>` — Execução de um método de instância.
- `static:<Classe>::<Metodo>` — Execução de um método estático.
- `ctor:<Classe>` — Execução de um construtor.

Outros identificadores que você pode ver durante a inicialização/execução:
- `global:init` — Bloco de inicialização (ex.: propriedades estáticas).
- `base_ctor:<ClasseBase>` — Chamada de construtor da classe base durante a construção.
- `main:<NomeFuncao>` — Função de entrada selecionada para execução direta.

Breakpoints são específicos ao `code_id`. Por exemplo, um breakpoint em `method:Carro::acelerar` não dispara em `func:acelerar` (se existir uma função livre homônima).

## Exemplos rápidos

- Listar as próximas instruções e anotar IPs:
  - `dis 16`
- Adicionar breakpoint no IP 12 do contexto atual:
  - `bp add 12`
- Adicionar breakpoint no IP 5 de um método específico:
  - `bp add method:Carro::acelerar 5`
- Inspecionar variáveis:
  - `vars` e `v este`
- Ver posição atual:
  - `where`

## Limitações atuais

- Depuração funciona no backend de bytecode (interpretador). Não há depuração no LLVM IR.
- Breakpoints são por IP (nível de instrução), não por linha de código fonte.
- O modo de passo é apenas StepInto.

## Solução de problemas

- Não para em breakpoint: confirme o `code_id` correto com `where` e use `bp list` para verificar se o IP está cadastrado naquele `code_id`.
- Não encontro o IP certo: use `dis` com um número maior (ex.: `dis 50`) e procure a instrução alvo.
- Quer iniciar direto dentro de uma função: passe `--executar-funcao <Nome>` ao `interpretador`.
