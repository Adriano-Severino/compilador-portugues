# Arquitetura do Compilador Pôr do Sol

O Compilador da linguagem Pôr do Sol (localizado em `src/`) processa o código-fonte em várias etapas (Pipeline). Visando a manutenibilidade e a clareza, os subsistemas mais complexos do compilador foram modularizados em diretórios com responsabilidades bem definidas.

## Pipeline de Compilação

O fluxo de dados no compilador segue a seguinte ordem lógica:
1. **Lexer** (`src/lexer.rs`): Lê o texto e o converte em Tokens.
2. **Parser** (`src/parser/`): Converte Tokens em uma Árvore de Sintaxe Abstrata (AST).
3. **Análise Semântica (Type Checker + Ownership)**: Valida a corretude dos tipos, aridade de genéricos e regras de empréstimo (borrowing/ownership).
4. **Geração de Código (Code Gen)**: Transforma a AST validada no produto final, seja LLVM IR (para binários nativos) ou Bytecode Pôr do Sol (para execução no interpretador).

## Estrutura dos Módulos Principais

Os três maiores pilares do compilador foram segmentados nos seguintes módulos estruturais:

### 1. `src/type_checker/` (Analisador Semântico)
Responsável por inferir e verificar tipos antes da geração de código, evitando crashes e comportamentos indefinidos.
- `mod.rs`: Ponto de entrada, definindo a struct `VerificadorTipos` e a injeção da biblioteca padrão.
- `resolucao.rs`: Busca e validação do escopo, namespaces e nomes no projeto.
- `tipos_e_structs.rs`: Definições isoladas de tipos resolvidos em interface / classe.
- `hierarquia.rs`: Validação de overrides, checagem de implementações de interfaces nas subclasses.
- `declaracoes.rs`: Verificação semântica em nível de blocos e estruturas globais.
- `comandos.rs` e `expressoes.rs`: Rotinas aprofundadas que garantem consistência semântica de comandos iterativos/condicionais e inferência recursiva em expressões. É aqui que ocorre a normalização de tipos para garantir a aridade correta de parâmetros genéricos (ex: `IPar<A, B>`).
- `genericos.rs`: Trato particular com restrições e `Tipo::Aplicado`.

### 2. `src/codegen/llvm_ir/` (Geração de Código LLVM)
Responsável por compilar a AST verificada usando a C-API do LLVM para gerar código de máquina ultra rápido e nativo.
- `mod.rs`: Contexto e configuração inicial do gerador LLVM (builder, contextos, init e ponteiros opacos).
- `comandos.rs`: Compilação de comandos como *if*, *while* e atribuições, manipulando *Basic Blocks* do LLVM.
- `expressoes.rs`: Tradução das expressões (soma, chamadas de métodos) produzindo valores concretos e tipos de LLVM.
- `declaracoes.rs`: Geração da base estática: classes, funções de inicialização, tabelas de métodos virtuais (VTable) e interfaces estruturais.
- `async_gen.rs`: Processamento especializado de funções `assíncrona` criando continuações/Tasks e manipulação do Runtime do C.
- `conversoes.rs`: Conversores utilitários que traduzem dados abstratos (Strings do Rust, Tipos Pôr do Sol) para ponteiros legíveis em C.
- `resolucao.rs`: Rotinas para efetuar lookups diretos de funções exportadas, literais e variáveis de ambiente no módulo LLVM.

### 3. `src/codegen/bytecode/` (Geração de Bytecode Customizado)
Responsável por emitir o arquivo `.pbc` utilizado pelo *Interpretador* embarcado na linguagem.
- `mod.rs`: Struct construtora do bytecode (`BytecodeBuilder`).
- `comandos.rs`: Lógica de compilação de ramificações iterativas e condicionais, calculando desvios (labels) curtos/longos.
- `expressoes.rs`: Geração das instruções baseadas em pilha para cálculo de expressões (empilhar variáveis, invocar métodos virtuais).
- `declaracoes.rs`: Empacotamento de funções genéricas e comuns para as tabelas globais do `.pbc`.
- `resolucao.rs`: Funções para descobrir IDs únicos e referências pré-computadas na inicialização.
- `util.rs`: Ferramentas auxiliares, dicionários de strings (String Pool) para economizar tamanho de arquivo, mapeamento de saltos e tabelas literais.

## Integração e Segurança

A separação garante que, caso o desenvolvedor mude a checagem de tipos (adicionando novas *features* de OOP ou Memory Safety), ele não afete a máquina de geração de baixo nível. O fluxo atual garante que **somente o código semanticamente correto** e aprovado pelo TypeChecker e Analisador de Ownership (em `ownership.rs`) consiga atingir a etapa de `codegen`.
