# Diretrizes e Regras do Projeto: Compilador Pôr do Sol

Este documento estabelece as regras e restrições mandatórias para desenvolvimento, refatoração, caça a bugs e revisão de código no ecossistema da linguagem **Pôr do Sol**.

---

## 1. Princípios Inegociáveis da Linguagem
1. **Português Primeiro**:
   - Palavras-chave de sintaxe, tipos, operadores textuais e mensagens de erro do compilador devem ser estritamente em português brasileiro.
   - Proibido o vazamento de termos em inglês na sintaxe da linguagem (ex: usar `classe`, `interface`, `função`, `retorne`, `se`, `senão`, `enquanto`, `para`, `novo`, `este`, `base`, `publico`, `privado`, `protegido`, `estática`, `abstrata`, `redefinível`, `sobrescreve`, `obter`, `definir`, `verdadeiro`, `falso`, `inteiro`, `texto`, `booleano`, `duplo`, `decimal`, `flutuante`, `vazio`, `espaco`, `usando`, `enumeração`, `assíncrona`, `aguarde`).
2. **Modelo Mental C#**:
   - Orientação a objetos clássica com herança simples e múltiplas interfaces.
   - Polimorfismo explícito (`redefinível` e `sobrescreve`).
   - Propriedades com acessores `obter` / `definir`.
   - Construtor base explícito (`: base(...)`).
   - Inferência de tipos local com `var`.
   - Strings interpoladas com `$"..."`.
   - Métodos curtos com arrow function `=>`.
3. **Código Real e Compilável**:
   - Nenhum exemplo `.pr` pode ser mantido no repositório como pseudocódigo; todos devem ser compiláveis e executáveis.

---

## 2. Contrato das 8 Etapas para Novas Funcionalidades
Qualquer nova feature ou alteração na linguagem deve obrigatoriamente passar pelas seguintes 8 etapas:
1. **Lexer** (`src/lexer.rs` com Logos): Adicionar tokens necessários com suporte a UTF-8.
2. **Parser** (`src/parser.lalrpop`): Atualizar a gramática LALRPOP garantindo ausência de conflitos Shift/Reduce.
3. **AST** (`src/ast.rs`): Modelar a representação dos nós na AST.
4. **Validação Semântica & Ownership** (`src/type_checker/`, `src/ownership.rs`): Verificar compatibilidade de tipos, escopos, herança e ownership.
5. **Geração de Código** (`src/codegen/`): Atualizar LLVM IR, Bytecode (.pbc) e CIL (.il) correspondentes.
6. **Exemplos** (`exemplos/*.pr`): Criar ou atualizar exemplos práticos demonstrando o recurso.
7. **Testes** (`tests/`): Adicionar testes automatizados positivos e negativos.
8. **Documentação** (`README.md`, `docs/`): Atualizar a documentação oficial.

---

## 3. Padrões de Qualidade e Segurança em Rust
- **Zero Panics em Input do Usuário**: Proibido `.unwrap()` ou `.expect()` em código de compilação. Erros do desenvolvedor `.pr` devem retornar `Result<T, ErroCompilador>`.
- **Bounds Checking**: Indexação de arrays gerada nos backends deve obrigatoriamente validar limites.
- **Validação de Bytecode**: O interpretador deve validar magic bytes, opcodes válidos e limites de saltos de arquivos `.pbc`.

---

## 4. Subagentes Especializados Disponíveis
- `bug-hunter` / `pordosol-bug-hunter`: Diagnóstico e correção de bugs silenciosos no compilador, parser, type checker, ownership e geradores de código.
- `code-reviewer` / `pordosol-code-reviewer`: Revisão detalhada de PRs e diffs avaliando o cumprimento do contrato das 8 etapas, Rust limpo e semântica C#.
- `security-auditor` / `pordosol-security-auditor`: Auditoria de segurança, DoS no parser/type checker, bounds checking, integridade da VM e supply chain.
