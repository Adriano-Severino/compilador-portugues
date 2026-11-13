# Biblioteca Padrão (sistema-padrao)

Guia de como a biblioteca padrão do Por do Sol é compilada, carregada e executada em cada backend do compilador.

## Visão geral

A `sistema-padrao` é um projeto irmão do compilador, escrito inteiramente em `.pr`, organizado por namespaces inspirados na BCL do .NET (`Sistema.Console`, `Sistema.Colecoes`, `Sistema.Texto`, `Sistema.IO`, `Sistema.Rede`, `Sistema.Data`, `Sistema.Threading`, etc.). O código-fonte fica em `../sistema-padrao/src/sistema/` e o manifesto do projeto em `../sistema-padrao/Sistema.toml`.

O compilador interage com ela em três níveis:

1. **Type checker** — precisa conhecer classes, métodos e namespaces da stdlib para validar chamadas como `Texto.ParaMaiusculas("ola")`.
2. **Backend bytecode/LLVM** — precisa do bytecode/LLVM IR das implementações para executar chamadas reais.
3. **Backend CIL** — não usa as implementações `.pr`; mapeia um subconjunto das APIs diretamente para a BCL do .NET.

## Compilando a biblioteca

```bash
# a partir do diretório compilador-portugues
cargo run --bin compilador -- --compilar-biblioteca=../sistema-padrao
```

Isso produz em `../sistema-padrao/dist/`:

| Artefato | Conteúdo | Consumido por |
|---|---|---|
| `sistema.pbl` | Manifesto de API + bytecode da biblioteca | Type checker e backend bytecode |
| `sistema.ll` | LLVM IR da biblioteca | Backend `llvm-ir` (linkagem automática) |

`--compilar-biblioteca` ignora `--target`: sempre gera `.pbl` + `.ll`.

## Como um programa de usuário carrega a stdlib

A resolução da biblioteca segue esta ordem (`src/main.rs`):

1. `--stdlib-src-path=<dir>` — usa os fontes `.pr` da stdlib diretamente (útil em desenvolvimento);
2. `PORDOSOL_HOME/tools/stdlib` — instalação padrão, se existir;
3. Para alvos compilados (não-`bytecode`): tenta `dist/sistema.pbl` pré-compilado; se não existir, cai para os fontes;
4. Se nada for encontrado, o compilador tenta localizar `sistema-padrao` relativo ao executável.

Na prática, após rodar `--compilar-biblioteca` uma vez, programas `.pr` que usam `usando Sistema;` compilam sem flags extras.

Scripts de referência:

- `testar_exemplo_com_stdlib.ps1` — fluxo completo bytecode (compila `.pbl`, compila exemplo, executa `.pbc`);
- `build_stdlib.sh` — atalho para `--compilar-biblioteca=../sistema-padrao`.

## Paridade por backend

### Bytecode (`.pbc` + interpretador)

Cobertura completa: todos os nativos são despachados por `src/bin/interpretador/nativos.rs`. É o backend de referência semântica.

### LLVM (`llvm-ir` → clang)

As chamadas da stdlib viram chamadas a `porsol_native_call` (`src/runtime/porsol_runtime.c`), um dispatcher por chave `"Classe::Metodo"` com objetos opacos (`PorsolObj`). O link final inclui:

```bash
clang programa.ll src/runtime/async_runtime.c src/runtime/porsol_runtime.c \
      ../sistema-padrao/dist/sistema.ll -o programa.exe \
      -llegacy_stdio_definitions -lws2_32 -lshell32   # Windows
# não-Windows: -pthread no lugar das libs Win32
```

- `-lws2_32`: Winsock (SocketTcp, ServidorTcp, ClienteHttp, Dns);
- `-lshell32`: `CommandLineToArgvW` (`Ambiente.Argumentos`);
- `compilar_llvm_ir_com_runtime` (`src/codegen/mod.rs`) já adiciona tudo isso automaticamente ao alvo `llvm-ir`.

#### HTTPS no LLVM (mbedTLS vendored)

`https://` usa mbedTLS 3.6.7 vendored em `src/runtime/tls/` (ver `src/runtime/tls/VENDORING.md`). O compilador pré-compila `target/tls-cache/libporsol_tls.a` uma única vez e linka a biblioteca somente quando o `.ll` contém chamadas reais a `ClienteHttp` — programas sem `ClienteHttp` não carregam TLS. Para compilação manual de um `.ll` que usa `ClienteHttp`:

```bash
clang programa.ll src/runtime/async_runtime.c src/runtime/porsol_runtime.c \
      ../sistema-padrao/dist/sistema.ll target/tls-cache/libporsol_tls.a \
      -o programa.exe -DPOR_DO_SOL_HAS_TLS \
      -llegacy_stdio_definitions -lws2_32 -lshell32 -lbcrypt -Wl,/opt:ref \
      -ffunction-sections -fdata-sections   # Windows
# não-Windows: -pthread -Wl,--gc-sections (sem libs Win32/bcrypt)
```

Use o mesmo clang que o compilador usa (`clang_executable()` em `src/codegen/mod.rs`) — a biblioteca TLS é compilada com o target dele.

`ClienteHttp.*Assincrono` executa o cliente HTTP síncrono do `porsol_runtime.c` em uma thread do pool do `async_runtime.c` via `task_create_http` — `aguarde` em método de instância stdlib é suportado pelo codegen (`await_http_async`), que lê `BaseUrl` na thread principal antes de criar a task.

Cobertura verificada: todos os nativos exercitados por `exemplos/teste_stdlib_fase0,2,3,4,5,6.pr` (a fase 1 não existe — numeração histórica) mais `ClienteHttp.*Assincrono` (teste `llvm_stdlib_cliente_http_assincrono` com servidor HTTP local). Ver `tests/llvm_stdlib_test.rs`.

### CIL (`cil-bytecode` / `console` → .NET)

O backend CIL **não executa** as implementações `.pr` da stdlib: `tentar_stdlib_bcl` em `src/codegen/cil/expressoes.rs` reescreve chamadas conhecidas para métodos da BCL. Mapeamentos atuais:

| Por do Sol | .NET BCL |
|---|---|
| `Matematica.*` | `System.Math` (Abs, Max, Min, Pow, Sqrt, Round, Floor, Ceiling, Sin, Cos, Tan, Log, Log10, Exp, PI, E) |
| `Matematica.Pi`/`E` | `ldc.r8` das constantes |
| `Texto.*` | `System.String` — ParaMaiusculas→`ToUpper`, ParaMinusculas→`ToLower`, Tamanho→`Length`, Subtexto→`Substring`, IndiceDe/UltimoIndiceDe→`IndexOf`/`LastIndexOf`, ComecaCom/TerminaCom→`StartsWith`/`EndsWith`, Contem→`Contains`, PadEsquerda/Direita→`PadLeft`/`PadRight`, Comparar→`Compare`, IgualIgnoreCase→`Equals(..., OrdinalIgnoreCase)`, Substituir→`Replace`, RemoverEspacos→`Trim` |
| `Guid.Novo`/`Vazio`/`EhValido` | `System.Guid` (`NewGuid`, `Empty` via `ldsfld`, `TryParse`) |
| `Ambiente.NomeComputador` | `System.Environment.MachineName` |
| `Ambiente.VariavelAmbiente`/`DefinirVariavelAmbiente` | `System.Environment.GetEnvironmentVariable`/`SetEnvironmentVariable` |
| `Conversao.ParaInteiro`/`ParaFlutuante`/`ParaDuplo`/`ParaBooleano` | `System.Convert`/`Int64.Parse` etc. com **`CultureInfo.InvariantCulture`** (sem isso, `ParaDuplo("3.5")` retorna 35 em cultura pt-BR) |
| `Console.*` | `System.Console` |

Regras de emissão importantes:

- Tipos de valor (Guid) exigem `box` antes de `callvirt Object::ToString`;
- `inteiro` (int64) precisa de `conv.r8` antes de métodos `System.Math` (float64);
- Campos estáticos (`Guid.Empty`) usam `ldsfld`, não `call get_*`.

**Não mapeado ainda**: coleções, IO, rede, threading, `Regex`, `Json`, `Codificacao`, `Cronometro`, `Data`/`Horario` — chamadas a esses APIs no alvo CIL não têm equivalente BCL hoje. O teste `cil_stdlib_bcl_mapeado` em `tests/cil_run_examples_test.rs` cobre o subconjunto mapeado.

## Limitações conhecidas

- **LLVM Regex**: engine própria em C (literais, `.`, `*`, `+`, `?`, `{n,m}`, `[...]`, `(...)`, `|`, escapes e classes predefinidas); não é 100% compatível com a do interpretador em padrões exóticos.
- **ClienteHttp HTTP/HTTPS**: HTTP/1.1 com `http://` e `https://` nos backends bytecode e LLVM. No bytecode, TLS delega ao SO via `native-tls` (Schannel no Windows, Secure Transport no macOS, OpenSSL no Linux). No LLVM, TLS usa **mbedTLS 3.6.7 vendored** em `src/runtime/tls/mbedtls/` com validação de cadeia contra o CA bundle Mozilla embutido (`src/runtime/tls/porsol_cacert.c`, gerado de `cacert.pem`). O runtime TLS é pré-compilado uma vez em `target/tls-cache/libporsol_tls.a` e só é linkado quando o programa usa `ClienteHttp`; programas sem `ClienteHttp` não carregam nada de TLS. Não há suporte a certificado de cliente (mTLS), keep-alive ou HTTPS no alvo CIL (`Sistema.Rede` não é mapeado). Os métodos `*Assincrono` rodam a mesma implementação síncrona em threads do pool de tarefas (não é I/O verdadeiramente não-bloqueante, mas não bloqueia a thread principal).
- **LLVM tipos genéricos**: valores `T`/`TValor` retornados como `nulo` podem aparecer como `0`/vazio quando o tipo inferido é de valor.
- **CIL**: apenas o subconjunto da tabela acima; demais APIs não têm mapeamento.
- **PoolTarefas.DefinirMaximoParalelismo** é no-op no runtime C (o máximo exibido não muda).

## Exemplos de teste

| Exemplo | Cobre |
|---|---|
| `exemplos/Utilizado_biblioteca_sistema_padrao.pr` | Uso canônico + `Sistema.Rede` |
| `exemplos/teste_stdlib_fase0.pr` | Console, Matemática, Json, Arquivo, Data, Tarefa |
| `exemplos/teste_stdlib_fase2.pr` | Texto, Codificação, Regex |
| `exemplos/teste_stdlib_fase3.pr` | IO (Arquivo, Diretorio, Leitor, Caminho, InformacaoArquivo) |
| `exemplos/teste_stdlib_fase4.pr` | Threading (Tarefa, Temporizador, PoolTarefas) |
| `exemplos/teste_stdlib_fase5.pr` | Guid, Aleatorio |
| `exemplos/teste_stdlib_fase6.pr` | Conversao, Horario, Data, Dns, Ambiente, coleções |

Testes de integração: `tests/llvm_stdlib_test.rs` (LLVM), `tests/cil_run_examples_test.rs` (CIL), `testar_exemplo_com_stdlib.ps1` (bytecode).
