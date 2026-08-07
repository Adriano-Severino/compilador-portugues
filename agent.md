# Agent.md - Compilador Portugues

## Objetivo do projeto

O objetivo deste projeto e criar uma linguagem de programacao totalmente em portugues brasileiro, chamada hoje de Por do Sol, com sintaxe, palavras-chave e nomes fundamentais em portugues, mas com o mesmo modelo mental de uma linguagem moderna como C#.

A meta principal nao e apenas traduzir alguns comandos. A proposta e construir uma linguagem completa, expressiva e familiar para quem pensa em portugues, mantendo os pilares que tornam C# produtivo hoje:

- paradigma orientado a objetos;
- tipagem estatica forte;
- classes, interfaces, heranca e polimorfismo;
- propriedades, metodos, construtores e modificadores de acesso;
- parametros opcionais e inferencia local com `var`;
- suporte progressivo a recursos modernos, incluindo genericos, arrays, strings interpoladas e recursos funcionais;
- compilacao para alvos reais, incluindo LLVM, .NET/CIL e bytecode proprio.

A diferenca essencial em relacao ao C# e que a sintaxe da linguagem deve ser em portugues. O programador deve escrever `classe`, `interface`, `função`, `retorne`, `se`, `senão`, `enquanto`, `para`, `novo`, `este`, `base`, `publico`, `privado`, `protegido`, `estática`, `abstrata`, `redefinível`, `sobrescreve`, `obter`, `definir`, `verdadeiro`, `falso`, `inteiro`, `texto`, `booleano`, `duplo`, `decimal` e outros termos naturais ao ecossistema da linguagem.

## Visao da linguagem

Por do Sol deve ser uma linguagem de programacao moderna em portugues, inspirada em C# na organizacao de codigo, no sistema de tipos e no estilo de orientacao a objetos. Ela deve servir tanto para ensino e inclusao de estudantes brasileiros quanto para experimentacao com compilacao real e desenvolvimento de programas mais completos.

O projeto deve preservar estes principios:

- Portugues primeiro: nomes da sintaxe, mensagens e exemplos devem priorizar portugues brasileiro.
- Familiaridade com C#: quando houver duvida de design, preferir uma semantica parecida com C# moderno, adaptada para portugues.
- Codigo real, nao pseudocodigo: arquivos `.pr` devem ser compilaveis e executaveis.
- Evolucao incremental: cada recurso novo deve passar por lexer, parser, AST, verificacao semantica, geracao de codigo e testes.
- Multiplos backends: o compilador deve continuar permitindo experimentar diferentes alvos de execucao.

## Estado atual do projeto

O compilador e escrito em Rust. A tokenizacao usa `logos`, o parser usa `lalrpop`, a AST fica em `src/ast.rs`, a verificacao semantica em `src/type_checker.rs` e os geradores de codigo ficam em `src/codegen/`.

Recursos ja documentados ou presentes no codigo:

- arquivos fonte com extensao `.pr`;
- namespaces com `espaco` e imports com `usando`;
- declaracoes top-level e funcoes;
- tipos primitivos `inteiro`, `texto`, `booleano`, `flutuante`, `duplo`, `decimal` e `vazio`;
- inferencia local com `var`;
- operadores aritmeticos, logicos e de comparacao;
- `se` / `senão`, `enquanto` e suporte sintatico a `para`;
- arrays com literais, indexacao e `tamanho` / `comprimento`;
- classes com campos, propriedades, metodos e construtores;
- modificadores `publico`, `privado`, `protegido`, `estática`, `abstrata`, `redefinível` e `sobrescreve`;
- heranca com `:` e chamada de construtor base com `: base(...)`;
- interfaces, multiplas interfaces por classe e polimorfismo;
- enumerações com `enumeração`;
- parametros opcionais em estilo C#;
- strings interpoladas com `$"texto {expressao}"`;
- genericos em classes, interfaces, metodos e funcoes em evolucao;
- verificacao de tipos, resolucao de classes, interfaces e enums;
- compilacao de multiplos arquivos em uma AST unificada;
- bytecode proprio com interpretador e modo de depuracao;
- geracao de LLVM IR, CIL `.il`, projeto console .NET e bytecode `.pbc`.

## Estrutura importante

- `README.md`: visao geral, instalacao, uso e exemplos maiores da linguagem.
- `docs/`: documentacao por recurso da linguagem.
- `exemplos/`: programas `.pr` usados como referencia pratica.
- `src/lexer.rs`: palavras-chave, operadores, literais e tokens.
- `src/parser.lalrpop`: gramatica da linguagem.
- `src/ast.rs`: representacao estrutural do programa.
- `src/type_checker.rs`: regras semanticas e compatibilidade de tipos.
- `src/codegen/llvm_ir.rs`: geracao de LLVM IR.
- `src/codegen/bytecode.rs`: geracao do bytecode proprio.
- `src/codegen/cil.rs`: geracao de CIL para .NET.
- `src/codegen/console.rs`: geracao de projeto console .NET.
- `src/bin/interpretador.rs`: maquina virtual do bytecode e depurador.
- `tests/`: testes de integracao, LLVM, bytecode e exemplos.

## Como pensar novas contribuicoes

Ao implementar ou alterar um recurso da linguagem, trate o fluxo inteiro do compilador como contrato:

1. Adicionar ou ajustar tokens no lexer.
2. Atualizar a gramatica no parser.
3. Representar o recurso na AST.
4. Validar tipos e regras semanticas.
5. Atualizar os backends afetados.
6. Adicionar exemplos `.pr`.
7. Adicionar ou ajustar testes.
8. Atualizar a documentacao em `README.md` ou `docs/`.

Sempre que o comportamento esperado tiver equivalente em C#, use C# como referencia de semantica, mas traduza a experiencia para portugues. O objetivo final e que escrever em Por do Sol pareca programar em uma linguagem brasileira completa, nao em uma camada superficial sobre outra linguagem.

## Comandos uteis

```bash
cargo build
cargo test
cargo run --bin compilador -- exemplos/teste.pr --target=bytecode
cargo run --bin interpretador -- exemplos/teste.pbc
cargo run --bin compilador -- exemplos/teste.pr --target=llvm-ir
```

Alvos aceitos pelo compilador:

- `llvm-ir`: gera `.ll` e pode compilar com `clang`;
- `cil-bytecode`: gera `.il`;
- `console`: gera um projeto console .NET;
- `bytecode`: gera `.pbc` para o interpretador proprio;
- `universal`: tenta gerar todos os alvos.

## Direcao de longo prazo

A direcao do projeto e aproximar Por do Sol do conjunto de capacidades que C# oferece hoje, mantendo a identidade em portugues. Isso inclui amadurecer orientacao a objetos, genericos, biblioteca padrao, tratamento de erros, recursos funcionais, ferramentas de debug, mensagens de erro amigaveis, interoperabilidade com backends reais e uma experiencia consistente para programas pequenos e grandes.

Toda decisao tecnica deve servir a essa visao: uma linguagem de programacao completa, moderna, em portugues, com a ergonomia de C# e a capacidade de gerar codigo executavel de verdade.

Para novas features ou refatorações, siga as boas práticas e padrões de projeto para obter performance, confiabilidade, rastreabilidade, manutenibilidade e escalabilidade. Isso facilitará a manutenção e a implementação de novas features, tornando a linguagem profissional e pronta para o mercado.
