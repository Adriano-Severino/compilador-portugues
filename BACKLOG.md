# Backlog — Ecossistema Por do Sol

Backlog mestre do ecossistema Por do Sol: linguagem, compilador, biblioteca padrão, framework web, ferramentas CLI, Language Server e IDE — rumo ao objetivo de uma **linguagem de programação completa, moderna e produtiva, 100% em português brasileiro, com a ergonomia do C## e a capacidade de gerar código executável de verdade**.

## Visão do objetivo

O estado final desejado é um ecossistema coerente onde um desenvolvedor consegue:

1. Instalar o SDK (`pordosol`) e criar um projeto com `pordosol new`;
2. Escrever código idiomático em Por do Sol (OO, generics, async, exceções, LINQ);
3. Usar uma biblioteca padrão completa (`sistema-padrao`) como a BCL do .NET;
4. Construir APIs web com `net-por-do-sol` (estilo ASP.NET Core);
5. Desenvolver na IDE oficial (`pordosol-studio`) com LSP, debugger e toolchain integrados;
6. Distribuir aplicações compiladas para bytecode, LLVM nativo ou .NET/CIL.

## Legenda

| Campo | Valores |
|---|---|
| Prioridade | **P0** = bloqueia o objetivo central · **P1** = importante para paridade com C#/produtividade · **P2** = diferencial/polish |
| Esforço | **P** = pequeno · **M** = médio · **G** = grande |
| Status | `⬜` a fazer · `🔶` em andamento · `✅` feito |
| Subprojeto | `compilador` · `stdlib` · `net` · `cli` · `lsp` · `studio` · `qa` |

> **Regra de dependência:** cada épico respeita a ordem do roadmap ao final. O `net` depende de `stdlib` e `compilador`; `cli`/`lsp`/`studio` consomem os artefatos dos anteriores.

---

## Épico A — Linguagem e Compilador (`compilador-portugues`)

### A.1 Completude do comando `para`

> **Contexto:** `para` existe no lexer/parser/AST e tem codegen apenas no backend CIL (`src/codegen/cil/comandos.rs`). Os backends **LLVM IR** e **bytecode** não tratam `Comando::Para`.

| # | Tarefa | Pri | Esf | Status | Critério de aceitação |
|---|---|---|---|---|---|
| A.1.1 | Implementar codegen de `para` em LLVM IR | P0 | M | ✅ | Um `.pr` com `para (i = 0; i < n; i = i + 1)` compila com `--target=llvm-ir`, linka com clang e executa com a saída correta; testes em `tests/llvm_*` passam. |
| A.1.2 | Implementar codegen de `para` em bytecode + interpretador | P0 | M | ✅ | `--target=bytecode` gera `.pbc` executável pelo `interpretador`; o loop itera o número correto de vezes; `build/` artifacts regenerados. |
| A.1.3 | Validar `para` no type checker e ownership | P0 | P | ✅ | Variáveis de init/cond/incr têm tipos verificados; erros de tipo produzem mensagem clara com linha/coluna. |
| A.1.4 | Teste de paridade `para` nos 3 backends | P0 | P | ⬜ | Mesmo programa `.pr` produz saída idêntica em `bytecode`, `llvm-ir` e `cil-bytecode`. |

### A.2 `para cada` (foreach)

| # | Tarefa | Pri | Esf | Status | Critério de aceitação |
|---|---|---|---|---|---|
| A.2.1 | Lexer/parser/AST para `para cada (item em colecao)` | P1 | M | ⬜ | Nova sintaxe documentada em `docs/`; gramática LALRPOP sem novos conflitos. |
| A.2.2 | Codegen `para cada` nos 3 backends | P1 | M | ⬜ | Itera `Lista<T>`, arrays e `Texto` (chars) com a mesma saída nos 3 backends. |

### A.3 Exceções (`tente` / `capture` / `finalmente`)

> Sem exceções hoje não há como escrever código robusto nem a stdlib/framework tratam erros de I/O.

| # | Tarefa | Pri | Esf | Status | Critério de aceitação |
|---|---|---|---|---|---|
| A.3.1 | Desenhar e documentar a sintaxe de exceções em PT | P0 | P | ⬜ | Proposta aprovada em `docs/` (ex.: `tente { } capture (Erro e) { } finalmente { }`, `falhe "mensagem"`). |
| A.3.2 | Lexer + parser + AST de exceções | P0 | M | ⬜ | Programa com `tente/capture/finalmente` parseia e aparece na AST. |
| A.3.3 | Type checker de exceções | P0 | M | ⬜ | Tipos de exceção verificados; `capture` sem tipo casa com exceção base; erros de tipo reportados. |
| A.3.4 | Runtime de exceções (throw/catch) | P0 | G | ⬜ | Exceção lançada é capturada corretamente; `finally` executa em todos os caminhos (inclusive retorno). |
| A.3.5 | Codegen exceções — bytecode + interpretador | P0 | G | ⬜ | `.pbc` executa try/catch/finally; debug mode mostra a pilha de exceção. |
| A.3.6 | Codegen exceções — LLVM IR (invoke/landingpad) | P0 | G | ⬜ | LLVM IR usa `invoke`/`landingpad`/`resume`; binário nativo executa try/catch corretamente. |
| A.3.7 | Codegen exceções — CIL (`.try`/`.catch`) | P0 | M | ⬜ | `.il` gerado contém regiões protegidas válidas; executa no runtime .NET. |
| A.3.8 | `Erro`/hierarquia de exceções na stdlib | P1 | M | ⬜ | `Sistema.Erros.Erro`, `ErroIo`, `ErroArgumento` lançadas pela stdlib em falhas reais (arquivo inexistente, parse JSON inválido etc.). |

### A.4 Delegates e eventos

| # | Tarefa | Pri | Esf | Status | Critério de aceitação |
|---|---|---|---|---|---|
| A.4.1 | Sintaxe de delegado (`delegado`) e tipo funcional | P1 | M | ⬜ | `delegado Manipulador(int x)` declara tipo; método converte para delegado; chamada indireta funciona. |
| A.4.2 | Eventos (`evento`) com `adicionar`/`remover`/`acionar` | P1 | M | ⬜ | Classe com evento; assinante recebe notificação; unsubscribe funciona. |
| A.4.3 | Codegen delegates/eventos nos 3 backends | P1 | G | ⬜ | Paridade de comportamento entre bytecode, LLVM e CIL. |
| A.4.4 | Suporte a lambda/anônima além de `=>` simples | P2 | M | ⬜ | Lambdas com bloco `{ }` e captura de variáveis locais compilam nos 3 backends. |

### A.5 Generics completos

> Generics existem "em evolução" (classes, interfaces, métodos, funções). Falta paridade total com C#.

| # | Tarefa | Pri | Esf | Status | Critério de aceitação |
|---|---|---|---|---|---|
| A.5.1 | Restrições de tipo `onde T : classe/base/interface` | P1 | M | ⬜ | `Lista<inteiro>` válido, `Lista<int-distinto>` rejeitado com mensagem clara; verificado no type checker. |
| A.5.2 | Inferência de tipo genérico em chamadas | P1 | M | ⬜ | `var x = metodo(1)` infere `inteiro`; casos ambíguos geram erro explicativo. |
| A.5.3 | Genéricos em todos os backends sem fallback para `objeto` | P0 | G | ⬜ | Classes genéricas geram código especializado/eficiente em LLVM, CIL e bytecode; sem boxing desnecessário em tipos de valor. |
| A.5.4 | Covariância/contravariância (`em`/`out`/`in` ou equivalente PT) | P2 | G | ⬜ | `IEnumeravel<derivado>` atribuível a `IEnumeravel<base>` conforme regras documentadas. |

### A.6 Recursos de linguagem restantes

| # | Tarefa | Pri | Esf | Status | Critério de aceitação |
|---|---|---|---|---|---|
| A.6.1 | Indexadores `this[i]` em classes | P1 | M | ⬜ | Classe com `obter`/`definir` indexador; `lista[0]` funciona; paridade nos backends. |
| A.6.2 | Tuplas e destructuring | P2 | M | ⬜ | `(inteiro, texto)` como tipo; `var (a, b) = par()` funciona. |
| A.6.3 | Pattern matching (`se x é texto s`, `quando`) | P2 | G | ⬜ | Switch por tipo/valor documentado e testado nos 3 backends. |
| A.6.4 | Sobrecarga de operadores (`+`, `==` customizados) | P2 | G | ⬜ | Classe `Vetor` com `+`/`==` funciona; conversões implícitas/explicitas. |
| A.6.5 | Tipos nullable (`inteiro?`) e `nulo` consistente | P1 | M | ⬜ | `inteiro? x = nulo` compila; acesso sem checagem gera warning/erro; `x.tem_valor`/`x.valor`. |
| A.6.6 | `sealed`/`readonly`/`static class`/`const` | P2 | P | ⬜ | Modificadores documentados e enforced pelo type checker. |
| A.6.7 | Propriedades auto-implementadas `{ obter; definir; }` | P2 | P | ⬜ | `propriedade Nome { obter; definir; }` gera backing field; serialização futura enxerga. |

### A.7 Async/await maduro

| # | Tarefa | Pri | Esf | Status | Critério de aceitação |
|---|---|---|---|---|---|
| A.7.1 | `Tarefa<T>` e `aguarde` com resultados | P1 | G | ⬜ | `assíncrona` retorna `Tarefa<inteiro>`; `aguarde` obtém valor; exceções dentro de task propagam. |
| A.7.2 | Cancelamento (`Cancelamento`) e timeout | P2 | G | ⬜ | `aguarde tarefa` com token cancela corretamente; `Tarefa.QuandoQualquer`/`QuandoTudo`. |
| A.7.3 | Async streams (`assíncrona` iteradores) | P2 | G | ⬜ | `para cada` sobre stream async funciona. |
| A.7.4 | Paridade async nos 3 backends | P1 | G | ⬜ | Mesmo programa async produz resultado idêntico em bytecode, LLVM e CIL. |

### A.8 Qualidade do compilador

| # | Tarefa | Pri | Esf | Status | Critério de aceitação |
|---|---|---|---|---|---|
| A.8.1 | Mensagens de erro com span (linha/coluna) e sugestão | P0 | G | ⬜ | Erro de tipo/sintaxe mostra trecho do código, `^` na coluna e dica ("você quis dizer X?"); formato estável para o LSP consumir. |
| A.8.2 | Modo `--diagnostico` estruturado (JSON) | P1 | M | ⬜ | `compilador --diagnostico=arquivo.json` emite erros parseáveis por ferramentas. |
| A.8.3 | JIT Cranelift: cobertura de features | P1 | G | ⬜ | JIT executa classes, generics, strings interpoladas e async com testes; benchmark vs interpretador documentado. |
| A.8.4 | Debug info (arquivo/linha) no bytecode e nativo | P1 | M | ⬜ | Debugger do `pordosol-studio` mostra fonte correta; `.pbc` carrega tabela de linhas. |
| A.8.5 | Sistema de módulos/projetos (`.prj`, referências) | P1 | G | ⬜ | `compilador --projeto app.prj` resolve dependências locais; multi-arquivo com namespaces circulares detectado. |
| A.8.6 | Fuzzing/property tests do parser | P2 | M | ⬜ | Suite de fuzz não produz panic; erros sempre estruturados (sem crash do compilador). |
| A.8.7 | Reduzir `unwrap`/`panic!` em código Rust do compilador | P1 | M | ⬜ | `cargo clippy` sem panics em caminhos de erro; toda falha vira `ErroCompilacao`. |

---

## Épico B — Biblioteca Padrão (`sistema-padrao`)

### B.1 Fundação de coleções e iteração

| # | Tarefa | Pri | Esf | Status | Critério de aceitação |
|---|---|---|---|---|---|
| B.1.1 | `IEnumeravel<T>`/`IEnumerador<T>` e integração com `para cada` | P0 | M | ⬜ | `Lista`, `Dicionario`, arrays e `Texto` implementam a interface; `para cada` funciona sobre todos. |
| B.1.2 | `IComparable`/`IEqualityComparer`/`IComparer` | P1 | M | ⬜ | `Ordenar`/`Conjunto` usam comparadores customizados; paridade com C#. |
| B.1.3 | `Nullable<T>` e tipos utilitários (`TipoDe`, `Conversao`) | P1 | M | ⬜ | `TipoDe(5).Nome == "inteiro"`; conversões seguras com `Tentar*`. |

### B.2 LINQ-to-objects

| # | Tarefa | Pri | Esf | Status | Critério de aceitação |
|---|---|---|---|---|---|
| B.2.1 | `Sistema.Linq`: `Onde`, `Selecione`, `OrdenarPor`, `Agrupe`, `Primeiro`, `Conte` | P1 | G | ⬜ | Operadores sobre `IEnumeravel<T>` com lazy evaluation; testes cobrem casos vazios e nulos. |
| B.2.2 | Sintaxe fluente + (se disponível) query syntax | P2 | M | ⬜ | `lista.Onde(x => x > 0).Selecione(x => x * 2)` funciona. |

### B.3 Texto e serialização

| # | Tarefa | Pri | Esf | Status | Critério de aceitação |
|---|---|---|---|---|---|
| B.3.1 | `Json`: serialização de objetos/classe → JSON e parse reverso | P0 | G | ⬜ | Round-trip de classe com propriedades; erros de parse lançam `Erro` de A.3.8; unicode/escapes corretos. |
| B.3.2 | `Codificacao` completa (UTF-8/16/Latin1) e streams | P1 | M | ⬜ | Ler/escrever arquivo em cada codificação com BOM correto. |
| B.3.3 | `Texto` avançado: split/join/pad/span-like, `Format` | P1 | M | ⬜ | Paridade com `System.String` do .NET para os métodos listados. |
| B.3.4 | XML/HTML encoding helpers | P2 | P | ⬜ | `CodificarHtml`/`DecodificarHtml` cobertos por testes. |

### B.4 I/O, rede e sistema

| # | Tarefa | Pri | Esf | Status | Critério de aceitação |
|---|---|---|---|---|---|
| B.4.1 | `Processo`/`Process` (start, args, stdout/stderr, exit code) | P1 | M | ⬜ | Lançar processo externo e capturar saída em teste automatizado. |
| B.4.2 | `Ambiente` completo (variáveis, diretório corrente, SO, args) | P1 | P | ⬜ | API estável nos 3 SOs. |
| B.4.3 | `ClienteHttp` completo: headers, POST/PUT/DELETE, timeout, SSL | P0 | G | ⬜ | Request/response HTTPS real contra servidor de teste; erros de rede viram exceção. |
| B.4.4 | `SocketTcp`/`ServidorTcp` robustos (async, múltiplas conexões) | P1 | G | ⬜ | Servidor aceita N conexões concorrentes; base do `net-por-do-sol`. |
| B.4.5 | `Seguranca`: hash (SHA-256), HMAC, Base64 | P1 | M | ⬜ | Vetores de teste conhecidos passam. |
| B.4.6 | `Data`/`Horario` paridade com .NET (fuso, formatos, parsing) | P2 | M | ⬜ | Round-trip ISO-8601 e formatos BR. |

### B.5 Concorrência e async

| # | Tarefa | Pri | Esf | Status | Critério de aceitação |
|---|---|---|---|---|---|
| B.5.1 | `Tarefa`/`Tarefa<T>` sobre o async do compilador | P1 | G | ⬜ | `Tarefa.Delas`/`aguardar todas` funciona; resultados preservados. |
| B.5.2 | `SemaphoreSlim`/`Monitor`/`Interlocked` e `Bloqueio` | P1 | M | ⬜ | Contador concorrente sem race em teste com N threads. |
| B.5.3 | `Temporizador`, `PoolTarefas` com fila e cancelamento | P2 | M | ⬜ | Pool reutiliza threads; cancelamento drena fila. |

### B.6 Qualidade da stdlib

| # | Tarefa | Pri | Esf | Status | Critério de aceitação |
|---|---|---|---|---|---|
| B.6.1 | Matriz de testes `.pr` por backend | P0 | G | ⬜ | Cada módulo tem teste executado em bytecode, LLVM e CIL; relatório de paridade gerado. |
| B.6.2 | Cobertura de testes ≥ 80% dos métodos públicos | P1 | G | ⬜ | Relatório de cobertura por módulo publicado no CI. |
| B.6.3 | Docs de API geradas (`docs/api/`) | P2 | M | ⬜ | Cada namespace público tem página com assinaturas e exemplos. |
| B.6.4 | Performance: benchmarks de `Lista`, `Texto`, `Json`, `ClienteHttp` | P2 | M | ⬜ | `bench/` com resultados comparáveis entre releases. |

---

## Épico C — Framework Web (`net-por-do-sol`)

> **Estado atual:** todos os arquivos de `src/` estão vazios (0 linhas), exceto `api_controlador.pr` (19 linhas). Este épico implementa o framework do zero sobre `sistema-padrao`.

### C.1 HTTP e núcleo

| # | Tarefa | Pri | Esf | Status | Critério de aceitação |
|---|---|---|---|---|---|
| C.1.1 | `Http/Requisicao` + `Http/Resposta`: parsing e serialização HTTP/1.1 | P0 | G | ⬜ | Parser de request real (método, path, headers, query, body) e resposta com status/headers/body; testes com requests crus. |
| C.1.2 | `Http/Contexto`: abstração por request (Request/Response/RouteData/Services) | P0 | M | ⬜ | Contexto imutável por request; acessível em middlewares e controladores. |
| C.1.3 | `Nucleo/Servidor`: loop accept sobre `SocketTcp` com pool de conexões | P0 | G | ⬜ | Servidor responde `GET /` com 200; mantém keep-alive; erro em handler não derruba o servidor. |
| C.1.4 | `Nucleo/Rota`: tabela de rotas com parâmetros `{id}` e constraints | P0 | M | ⬜ | `/users/{id}` casa `/users/42`; rota não encontrada retorna 404 JSON. |
| C.1.5 | `Nucleo/Middleware`: pipeline `Usar`/`UsarProximo`, ordem, curto-circuito | P0 | M | ⬜ | Middlewares executam em ordem registrada; um middleware pode responder sem chamar o próximo. |
| C.1.6 | `Nucleo/Aplicacao`: builder `WebApplication` (rotas, middlewares, DI, run) | P0 | G | ⬜ | `Aplicacao.Criar().MapaGet("/", ...).Iniciar(5000)` sobe servidor funcional. |

### C.2 Injeção de dependência e controladores

| # | Tarefa | Pri | Esf | Status | Critério de aceitação |
|---|---|---|---|---|---|
| C.2.1 | `Dependencias/Container`: registro `AdicionarSingleton`/`AdicionarTransiente`/`AdicionarEscopo` | P0 | G | ⬜ | Resolução por tipo; construtores com dependências resolvidos; ciclo detectado com erro claro. |
| C.2.2 | `Controladores/ControladorBase` + convenção de rotas (`[ApiController]` ou equivalente) | P0 | G | ⬜ | `class UsuariosControlador` com `Obter(int id)` responde `GET /usuarios/42`. |
| C.2.3 | Model binding: route/query/body JSON → parâmetros | P0 | G | ⬜ | POST JSON `{ "nome": "x" }` liga em classe; erro de validação retorna 400. |
| C.2.4 | Filtros/validação e códigos de status (`Ok`, `Criado`, `NaoEncontrado`, `Erro`) | P1 | M | ⬜ | Action results tipados; serialização JSON automática da resposta. |
| C.2.5 | Autenticação/autorização mínima (API key / bearer) | P2 | G | ⬜ | Middleware de auth rejeita 401 sem token; claims disponíveis no contexto. |

### C.3 Integração e distribuição

| # | Tarefa | Pri | Esf | Status | Critério de aceitação |
|---|---|---|---|---|---|
| C.3.1 | Projeto exemplo `exemplos/api.pr` compilando em bytecode e LLVM | P0 | M | ⬜ | API CRUD completa roda com `pordosol run`. |
| C.3.2 | Template `web` da CLI usando `net-por-do-sol` | P1 | M | ⬜ | `pordosol new web -n app` gera projeto que sobe servidor no primeiro `pordosol run`. |
| C.3.3 | Testes E2E: sobe app, faz requests HTTP reais, valida respostas | P0 | M | ⬜ | Suite automatizada cobre rotas, middlewares e DI. |
| C.3.4 | Documentação do framework (`docs/`) com tutorial | P1 | M | ⬜ | Tutorial "primeira API em 15 min" + referência de middlewares/DI. |

---

## Épico D — Ferramentas CLI (`ferramentas-cli` / `pordosol`)

> Fases 1–7 do `PLANO-CLI-SDK.md` concluídas (new/build/run/doctor/templates/CI). Faltam capacidades de um SDK maduro.

| # | Tarefa | Pri | Esf | Status | Critério de aceitação |
|---|---|---|---|---|---|
| D.1 | `pordosol test`: runner de testes `.pr` com asserts | P0 | G | ⬜ | `pordosol test` descobre `testes/*.pr`, executa e reporta falhas com nome do teste e linha; exit code != 0 em falha. |
| D.2 | Mini-framework de testes na stdlib (`Sistema.Testes`: `Teste`, `Afirme`, `AfirmeIgual`) | P0 | M | ⬜ | Testes escritos com a própria linguagem; integração com D.1. |
| D.3 | `pordosol publish`/`pack`: empacotar app/biblioteca para distribuição | P1 | G | ⬜ | Gera artefato único (`.pbd`/binário) com dependências resolvidas; `pordosol run app.pbd` funciona. |
| D.4 | Gerenciador de pacotes: `Sistema.toml` com fontes/versões, `restore`, lock file | P1 | G | ⬜ | Dependência remota baixa, verifica checksum, trava versão em `Sistema.lock`. |
| D.5 | Comando `format` (formatter oficial) | P2 | M | ⬜ | `pordosol format` normaliza estilo; idempotente; não quebra código. |
| D.6 | Comando `doc` (gera documentação do projeto) | P2 | M | ⬜ | Extrai doc-comments e gera HTML/Markdown. |
| D.7 | `pordosol debug` integrado ao interpretador (breakpoints via CLI) | P2 | M | ⬜ | `pordosol debug app.pr` para em breakpoints; inspeção de variáveis. |
| D.8 | Autocomplete/ajuda contextual (`pordosol --help` por comando, man pages) | P2 | P | ⬜ | Toda flag documentada; exemplos em `pordosol help <cmd>`. |
| D.9 | Migração de projetos antigos e depreciação suave de aliases | P2 | P | ⬜ | Aliases `novo/compilar/rodar` emitem aviso de depreciação com data-alvo. |

---

## Épico E — Language Server (`pordosol-language-server`)

> Hoje: `server/src/server.ts` único (~3.2k linhas) baseado em **regex/texto** — completion/hover/diagnostics/definition existem mas sem integração com o compilador real.

| # | Tarefa | Pri | Esf | Status | Critério de aceitação |
|---|---|---|---|---|---|
| E.1 | Backend de análise via compilador (`--diagnostico` JSON de A.8.2) | P0 | G | ⬜ | Diagnósticos do LSP vêm do compilador real (tipos, semântica), não só de regex; latência < 500ms em projeto médio. |
| E.2 | Autocomplete da stdlib (`sistema.pbl`) com assinaturas e docs | P0 | M | ⬜ | Digitar `Console.` lista métodos reais da stdlib com documentação no hover. |
| E.3 | Hover com informação de tipo e documentação | P1 | M | ⬜ | Hover em variável mostra tipo inferido; em método mostra assinatura + doc-comment. |
| E.4 | Go-to-definition multi-arquivo e multi-projeto | P1 | G | ⬜ | `F12` navega entre `.pr` do projeto e da stdlib; funciona com `usando`/namespaces. |
| E.5 | Find references / rename symbol | P1 | G | ⬜ | Rename de método/classe atualiza todos os usos sem quebrar strings/comentários. |
| E.6 | Snippets e completion de código (`classe`, `função`, `para`, `tente`) | P1 | P | ⬜ | Snippets oficiais geram esqueleto correto com tabstops. |
| E.7 | Formatação on-type / document formatting via `pordosol format` | P2 | M | ⬜ | `Shift+Alt+F` formata documento usando o formatter da CLI. |
| E.8 | Code actions: "extrair método", "criar função", "importar namespace" | P2 | G | ⬜ | Ações aparecem por contexto e aplicam edição correta. |
| E.9 | Testes do LSP (unit + integration) | P1 | M | ⬜ | Suite cobre providers com fixtures `.pr`; CI roda `npm test`. |
| E.10 | Refatorar `server.ts` em módulos (lexer próprio, providers separados) | P1 | M | ⬜ | Arquivo único dividido em `server/` por responsabilidade; sem regressão funcional. |

---

## Épico F — IDE (`pordosol-studio`)

> Fork do Code-OSS com toolchain embutida (`resources/bin/`) e extensão `extensions/pordosol`. Scripts `build-ide.ps1/.sh` existem.

| # | Tarefa | Pri | Esf | Status | Critério de aceitação |
|---|---|---|---|---|---|
| F.1 | Build reproduzível da IDE documentado e verificado em máquina limpa | P0 | M | ⬜ | Seguir README gera instalador/portable; versões de node/electron fixadas. |
| F.2 | Extensão `pordosol`: paridade com `pordosol-language-server` (reusar server) | P0 | M | ⬜ | Features do Épico E disponíveis na IDE sem servidor externo. |
| F.3 | Debugger: breakpoints/step/inspect via DAP contra interpretador | P0 | G | ⬜ | `F5` para em breakpoint; painel de variáveis mostra valores reais; `F10`/`F11` funcionam. |
| F.4 | Comandos IDE: build (`Ctrl+Shift+B`), run, new project via GUI | P1 | M | ⬜ | Paleta de comandos executa toolchain embutida; erros vão para Problems. |
| F.5 | Templates/snippets de projeto na IDE (`Arquivo > Novo > Projeto Por do Sol`) | P1 | M | ⬜ | Wizard cria console/web/biblioteca com `Sistema.toml` correto. |
| F.6 | Walkthrough/tutorial interativo na primeira execução | P2 | M | ⬜ | Novo usuário completa "olá mundo" guiado dentro da IDE. |
| F.7 | CI de build da IDE (Windows/Linux/macOS) e publicação de releases | P1 | G | ⬜ | Workflow gera artefatos por SO; versão sincronizada com toolchain embutida. |
| F.8 | Marketplace/extensões: temas, snippets da comunidade, sync de settings | P2 | M | ⬜ | Extensões OpenVSX relevantes funcionam; settings sync opcional. |
| F.9 | Auto-update da IDE e do toolchain embutido | P2 | G | ⬜ | Update baixa nova versão assinada; toolchain atualiza sem quebrar projetos. |
| F.10 | Acessibilidade e localização PT-BR da UI | P2 | M | ⬜ | Strings da IDE em PT-BR onde customizadas; contraste/a11y verificados. |

---

## Épico G — Qualidade, Release e Comunidade (transversal)

| # | Tarefa | Pri | Esf | Status | Critério de aceitação |
|---|---|---|---|---|---|
| G.1 | CI verde em Windows/Linux/macOS para todos os repos | P0 | M | ⬜ | Badge verde; testes rodam nos 3 SOs; builds de release reproduzíveis. |
| G.2 | E2E "máquina limpa": instalar SDK → `new` → `build` → `run` | P0 | M | ⬜ | Script em VM/container limpo passa sem intervenção manual. |
| G.3 | Matriz de compatibilidade versionada (`MATRIZ-COMPATIBILIDADE.md`) atualizada a cada release | P1 | P | ⬜ | Versões de cli/compilador/stdlib/lsp/studio declaradas e testadas. |
| G.4 | Guia oficial completo (`docs/guia-oficial/` — hoje só 6 arquivos) | P1 | G | ⬜ | Cobre: instalação, tipos, classes, generics, exceções, async, stdlib, framework web, testes. |
| G.5 | Tutorial progressivo: olá mundo → CLI → API web → publicar | P1 | M | ⬜ | Trilha documentada com código copiável e verificado em CI. |
| G.6 | Versionamento semântico + CHANGELOG por repo | P1 | P | ⬜ | Tags `vX.Y.Z` geram release notes automáticas; breaking changes documentadas. |
| G.7 | Benchmarks públicos (compilador, runtime, stdlib) | P2 | M | ⬜ | `bench/` com resultados vs C#/Go em cenários equivalentes. |
| G.8 | Política de contribuição, código de conduta e templates de issue/PR | P2 | P | ⬜ | `CONTRIBUTING.md` e templates em cada repo. |
| G.9 | Segurança: auditoria de supply chain, SBOM, assinatura de binários | P1 | M | ⬜ | Releases assinados; `cargo audit`/`npm audit` limpos; segredos nunca commitados. |
| G.10 | Playgound/web: "try Por do Sol" no navegador | P2 | G | ⬜ | Página compila e executa `.pr` client-side (WASM) ou via backend efêmero. |

---

## Roadmap sugerido (ordem de execução)

| Fase | Foco | Por quê |
|---|---|---|
| **1 — Fundação** | A.1 (`para`), A.3 (exceções), A.8.1 (erros) | Sem exceções e `para` completo não há linguagem robusta nem framework confiável. |
| **2 — Stdlib base** | B.1, B.3.1 (JSON), B.4.3 (HTTP), B.5.1 (tasks) | Desbloqueia `net-por-do-sol` e apps reais. |
| **3 — Framework web** | Épico C inteiro | Depende de B (socket, JSON, DI precisa de generics/exceções estáveis). |
| **4 — Ergonomia** | A.4 (delegates), A.5 (generics), B.2 (LINQ), D.1–D.2 (testes) | Produtividade do dia a dia. |
| **5 — Tooling/IDE** | Épico E, Épico F | Consome diagnósticos do compilador; quanto mais maduro o compilador, melhor o LSP. |
| **6 — Release** | Épico G contínuo; G.1–G.6 ao final de cada fase | Qualidade transversal desde o início. |

## Dependências críticas

- `net-por-do-sol` (C) **depende de**: `sistema-padrao` B.4.3/B.4.4/B.3.1 e de exceções A.3 (erro de I/O, parse JSON).
- LSP com diagnósticos reais (E.1) **depende de** A.8.2 (`--diagnostico` JSON).
- `pordosol test` (D.1) **depende de** mini-framework B-adjacente `Sistema.Testes`.
- Template `web` da CLI **depende de** Épico C mínimo (C.1.6 + C.3.1).
- Debugger da IDE (F.3) **depende de** A.8.4 (debug info) e do modo debug do interpretador.

## Como usar este backlog

1. Cada tarefa é independentemente entregável quando seus pré-requisitos estão ✅.
2. Ao iniciar uma tarefa, marcar `🔶` e referenciar a issue/PR no repo correspondente.
3. Critério de aceitação é o "Definition of Done": sem ele, a tarefa não fecha.
4. Revisar prioridades a cada fase do roadmap — P0 de fases futuras podem subir se bloquearem o objetivo.
