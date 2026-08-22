# Arquitetura do Interpretador Pôr do Sol

O Interpretador da linguagem Pôr do Sol (localizado em `src/bin/interpretador/`) foi projetado de forma modular para garantir alta manutenibilidade, facilitar a adição de novas funcionalidades e separar as responsabilidades do ciclo de execução.

## Estrutura de Módulos

A lógica central do interpretador, que outrora residia em um arquivo monolítico, está dividida nos seguintes submódulos principais:

### 1. `main.rs`
Ponto de entrada do interpretador executável. Responsável por:
- Ler e inicializar o bytecode (arquivos `.pbc`).
- Iniciar as instâncias iniciais do ambiente e estado de `debug`.
- Configurar o runtime assíncrono (via Tokio) para suporte nativo a `async`/`aguarde`.
- Gerenciar as flags de execução de linha de comando.

### 2. `tipos.rs`
Define as estruturas de dados centrais fundamentais, incluindo:
- `VM`: A própria máquina virtual, contendo pilhas, variáveis, escopos e cache de execução.
- `Valor`: A enumeração primária representando todos os tipos de dados da linguagem (Inteiros, Textos, Arrays, Objetos instanciados, etc.).
- `FuncInfo` e `ClasseInfo`: Metadados armazenados na inicialização sobre as funções e classes importadas.

### 3. `vm/` (Módulo de Máquina Virtual)
O coração da execução. Recentemente refatorado para um subdiretório visando melhor organização, contendo:
- `mod.rs`: Ponto de entrada, definindo a `struct VM` base e métodos de inicialização.
- `execucao.rs`: O loop principal (`run`) que lê e despacha instruções base do bytecode.
- `instrucoes.rs`: Lógica detalhada da execução das operações matemáticas, lógicas e primitivas (OpCodes).
- `util.rs`: Ferramentas de manipulação de memória, variáveis globais, cache e controle de fluxo avançado.

### 4. `carregador.rs`
Lida com a fase de inicialização ("Bootstraping"):
- Faz o *parsing* de definições de funções e classes do bytecode antes que o loop principal comece.
- Mapeia funções com o prefixo genérico.
- Prepara o dicionário (`HashMap`) de declarações prontas para execução.

### 5. `objetos.rs`
Módulo isolado para o sistema de Orientação a Objetos:
- Implementa a criação/instanciação de novos objetos (`criar_objeto`).
- Despacha chamadas de métodos de instância (identificando parâmetros e `este`).
- Despacha chamadas de métodos estáticos sem necessidade de contexto.

### 6. `nativos.rs`
Fornece integração (binds) com funções de baixo nível do sistema operacional / biblioteca padrão (Rust -> Pôr do Sol):
- Intercepta chamadas para classes nativas embutidas (ex: `Sistema.Console`, `Sistema.Arquivos`, `Sistema.Threads`).
- Mantém o despachante estático (`despachar_nativo_estatico`) e assíncrono (`despachar_nativo_assincrono`).

### 7. `debug.rs`
Ferramental de suporte ao desenvolvedor, incluindo a infraestrutura de:
- Protocolo de pontos de parada (Breakpoints).
- Avaliação e visualização do escopo de variáveis (var dump local e json dump).
- Rastreamento da pilha de chamadas (Call stack trancing).

## Fluxo de Vida (Lifecycle) da Máquina Virtual

1. **Boot**: `main.rs` instancia a struct `VM` passando o bytecode alvo.
2. **Setup**: `carregador.rs` faz varredura montando a tabela de métodos/classes em memória.
3. **Init**: A VM roda apenas os escopos globais e atribuições estáticas pré-inicializando o ambiente.
4. **Execution**: A função Principal (`Principal()`) é acionada invocando o `.run()` contínuo em `vm.rs`.
5. **Runtime Dynamics**: Instruções OO chamam `objetos.rs`, funções primitivas da linguagem delegam para `nativos.rs` de forma agnóstica de bloqueio (*non-blocking* em I/O).
