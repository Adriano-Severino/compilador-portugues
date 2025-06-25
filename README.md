# Meu Compilador - Por do sol

Este é o repositório do projeto "compilador-portugues", que desenvolve um compilador para uma linguagem de programação moderna escrita em português brasileiro.

## 📖 Sobre a Linguagem

Esta linguagem "por do sol" foi desenvolvida com foco acadêmico e educacional, visando democratizar o ensino de programação no Brasil através de uma sintaxe em português. No entanto, ela também é projetada para ser versátil o suficiente para desenvolvimento de aplicações desktop nativas com alta performance, graças à geração de código LLVM.

### 🎯 Objetivo Principal

Criar uma linguagem de programação que seja:

- Acessível para estudantes brasileiros iniciantes.
- Moderna com sintaxe inspirada em C#.
- Performática gerando código nativo via LLVM.
- Educacional mas capaz de projetos reais.

### 🚀 Recursos Principais

- Tipagem estática forte para maior segurança e detecção precoce de erros.
- Sintaxe em português inspirada em C#, adaptada para falantes nativos.
- Geração de código LLVM eficiente para performance nativa.
- Estruturas de controle como `se`, `senão`, `enquanto`, `para` e blocos com `{}`.
- Suporte completo a variáveis (`inteiro`, `texto`, `booleano`, `var`) com atribuições e inferência.
- Expressões aritméticas, lógicas e de comparação (`+`, `-`, `*`, `/`, `%`, `>`, `<`, `==`, `!=`, `&&`, `||`, `!`).
- Propriedades, métodos, funções, construtores com parâmetros opcionais (igual C#).
- Strings interpoladas com `$"texto {variavel}"`.
- Compilação multiplataforma para código nativo executável.
- Comentários com `//` (linha) e `/* */` (bloco).

## 📋 Pré-requisitos

Antes de começar, certifique-se de que você tem os seguintes softwares instalados:

- **Rust (versão 1.70+):** Necessário para construir o compilador.
    - Para instalar o Rust, use o `rustup`:
      ```bash
      curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
      ```
- **LLVM 16:** A linguagem depende especificamente desta versão.
    - Para Ubuntu/Debian:
      ```bash
      sudo apt-get update
      sudo apt-get install llvm-16 llvm-16-dev clang-16
      ```
    - É crucial que a variável de ambiente `LLVM_SYS_160_PREFIX` esteja configurada corretamente, apontando para o diretório de instalação do LLVM 16. Por exemplo:
      ```bash
      export LLVM_SYS_160_PREFIX=/usr/lib/llvm-16
      # Adicione esta linha ao seu ~/.bashrc ou ~/.zshrc para tornar a configuração permanente
      ```
- **Clang:** Usado para compilar o código LLVM IR gerado para um executável nativo (geralmente incluído com as ferramentas de desenvolvimento do LLVM).
- **opt:** Ferramenta de otimização do LLVM, usada para otimizar o código LLVM IR (também parte do toolchain LLVM).

## ⚙️ Instalação e Configuração

1. **Clone o repositório:**
    ```bash
    git clone https://github.com/Adriano-Severino/compilador-portugues
    cd compilador-portugues
    ```
2. **Configure o ambiente LLVM:**
    Certifique-se de que o LLVM 16 está instalado e a variável `LLVM_SYS_160_PREFIX` está definida no seu ambiente, como mostrado na seção de Pré-requisitos.
3. **Construa o compilador:**
    ```bash
    cargo build --release
    ```
    O executável do compilador estará em `target/release/compilador-portugues` (o nome pode variar dependendo do nome do seu crate no `Cargo.toml`).

## 📝 Como Usar

Os programas na sua linguagem devem ser escritos em arquivos com a extensão `.pr`.

### Estrutura Básica de um Programa

```por do sol
// Comentário de linha
// Este é um comentário de linha
/* Este é um comentário
   de múltiplas linhas */

inteiro idade = 25;
texto nome = "Maria";
booleano ativo = verdadeiro;

imprima("Olá, mundo!");
```

## Compilando e Executando (Método Recomendado)
O projeto inclui um script `build_production.sh` para facilitar o processo completo de compilação.

1. Crie um arquivo com seu código, por exemplo, `meu_programa.pr`.
2. Execute o script de compilação (não inclua a extensão .pr ao chamar o script):
    ```bash
    ./build_production.sh meu_programa
    ```
    Este script realizará os seguintes passos:
    - Compilará `meu_programa.pr` para LLVM IR (`meu_programa.ll`) usando `cargo run --release -- meu_programa.pr` (ou o executável do compilador diretamente).
    - Otimizará o código LLVM IR para `meu_programa_opt.ll` usando `opt`.
    - Compilará `meu_programa_opt.ll` para um executável nativo (`meu_programa`) usando `clang`.
    - Tentará gerar um executável estático (`meu_programa_static`).
3. Execute seu programa:
    ```bash
    ./meu_programa
    ```

## Passos Manuais de compilação: (para Entender o Processo)
Se você quiser entender o que o script `build_production.sh` faz:

1. Gerar LLVM IR:
    ```bash
    ./target/release/compilador-portugues meu_programa.pr
    ```
2. Otimizar (opcional, mas recomendado):
    ```bash
    opt -O3 -S meu_programa.ll -o meu_programa_opt.ll
    ```
3. Compilar para um executável nativo:
    ```bash
    clang meu_programa_opt.ll -o meu_programa
    ```
4. Execute seu programa:
    ```bash
    ./meu_programa
    ```


## 📚 Referência da Linguagem

### Tipos de Dados

| Tipo      | Exemplo         | Descrição                      |
|-----------|-----------------|-------------------------------|
| inteiro   | 42, -10         | Números inteiros de 64 bits   |
| texto     | "Olá"           | Strings de texto               |
| booleano  | verdadeiro, falso | Valores lógicos             |

### Operadores

| Categoria     | Operadores         | Exemplo           |
|--------------|--------------------|-------------------|
| Aritméticos  | +, -, *, /         | a + b * c         |
| Comparação   | >, <, >=, <=, ==, != | idade >= 18    |
| Lógicos      | &&, ||, !          | a > 0 && b < 10   |
| Atribuição   | =                  | idade = 25        |


## 💡 Exemplos de Código

### Olá, Mundo!
Código (`ola_mundo.pr`):

```por do sol
imprima("Olá, Mundo!");
imprima("Bem-vindo à programação em português!");
```

### Variáveis e Operações Aritméticas
Código (`variaveis.pr`):

```por do sol
inteiro a = 10;
inteiro b = 5;

imprima("=== Teste Aritmética ===");
imprima(a);
imprima(b);
imprima(a + b);
imprima(a - b);
imprima(a * b);
```

### Estruturas Condicionais
Código (`condicionais.pr`):

```por do sol
inteiro a = 10;
inteiro b = 5;

se (a > b) {
    imprima("a é maior que b");
} senão {
    imprima("a não é maior que b");
}

inteiro idade = 25;
texto nome = "João";

imprima("Olá mundo!");
imprima(nome);
imprima(idade);

se (idade > 18) {
    imprima("Maior de idade");
} senão {
    imprima("Menor de idade");
}

se (idade > 18)  {
    imprima("Maior de idade");
} 
senão se (idade == 18) {
    imprima("Tem 18 anos");
}
senão {
    imprima("Menor de idade");
}
```

### Loops e Contadores
Código (`loops.pr`):

```por do sol
inteiro contador = 0;
imprima("Iniciando contador...");

se (contador < 5) {
    imprima("Contador é menor que 5");
    contador = contador + 1;
    imprima(contador);
}
```

### Classes, Propriedades e Construtores Opcionais
Código (`exemplo_teste.pr`):

```por do sol
espaco Meu_Programa.Domain
{
    publico classe Pessoa
    {
        publico texto Nome { obter; definir; }
        publico inteiro Idade { obter; definir; }
        publico texto Sobrenome { obter; definir; }
        publico texto Endereco { obter; definir; }
        publico texto Telefone { obter; definir; }

        // Construtor com parâmetros padrão (como C#)
        publico Pessoa(texto nome, texto endereco, texto telefone, inteiro idade = 24, texto sobrenome = "Silva") {
            Nome = nome;
            Endereco = endereco;
            Telefone = telefone;
            Idade = idade;
            Sobrenome = sobrenome;
        }

        publico vazio apresentar() {
            imprima($"Nome: {Nome}, Endereço: {Endereco}, Telefone: {Telefone}, Idade: {Idade}, Sobrenome: {Sobrenome}");
        }
    }

    publico função teste_pessoa() 
    {
        Pessoa p1 = novo Pessoa("Joana", "Rua de exemplo", "123456789");
        Pessoa p2 = novo Pessoa("Maria", "Rua B", "987654321", 30);
        Pessoa p3 = novo Pessoa("Mariano", "Rua C", "123456789", 35, "Silva");
        p1.apresentar();
        p2.apresentar();
        p3.apresentar();
    }
}
```

### Funções Fora de Classe (Sintaxe Flexível)
Código (`funcoes.pr`):

```por do sol
espaco Meu_Programa.funcoes
{
    publico função bemvindo() 
    { 
        imprima("Olá mundo"); 
    }
    publico função configurar(texto nome) 
    { 
        imprima("Configurando: " + nome); 
    }
    publico função calcular() => inteiro 
    { 
        retorne 42; 
    }

    publico função multiplicar(inteiro a, inteiro b) => inteiro { retorne a * b; }

    privado função inteiro somar(inteiro a, inteiro b) 
    { 
     retorne a + b; 
    }
    publico função
    { 
        retorne a + b; 
    }
    publico função texto obter_nome() 
    { 
        retorne "João"; 
    }

    publico função booleano eh_par(inteiro numero) 
    { 
        retorne numero % 2 == 0; 
    }

    publico função vazio imprimir_linha() 
    { 
        imprima("================"); 
    }

    publico função Pessoa criar_pessoa(texto nome, inteiro idade) 
    { 
     retorne novo Pessoa(nome, idade); 
    }

    publico função processar_dados(texto nome, inteiro idade, booleano ativo, texto endereco, texto telefone) 
    { 
        imprima("Processando dados completos"); 
    }

    publico função texto gerar_relatorio(texto nome, inteiro idade, booleano ativo, texto endereco, texto telefone) 
    { 
        retorne "Relatório gerado"; 
    }

    publico função processar_completo(texto nome, inteiro idade, booleano ativo, texto endereco, texto
    telefone) => texto { retorne "Processamento completo"; }
    publico função testar_funcoes() 
    {
        processar_dados("João", 30, verdadeiro, "Rua A", "123456789");
        texto resultado = gerar_relatorio("Joana", 25, falso, "Rua B", "987654321");
        var processo = processar_completo("Mario", 35, verdadeiro, "Rua C", "123456789");
        imprima("Resultado do relatório: " + resultado);
        imprima("Resultado do processo: " + processo);
    }
}
```

### Strings Interpoladas
```por do sol
imprima($"Nome: {Nome}, Idade: {Idade}");
```

## 🏆 Exemplo Completo: Sistema de Biblioteca Digital

Abaixo um exemplo real de programa completo, mostrando classes, propriedades, construtores opcionais, métodos, funções, strings interpoladas, controle de fluxo e mais:

```pordosol
espaco BibliotecaDigital.Sistema {

publico classe Livro 
{

    // Propriedades
    publico texto   Titulo               { obter; definir; }
    publico texto   Autor                { obter; definir; }
    publico texto   ISBN                 { obter; definir; }
    publico inteiro AnoPublicacao        { obter; definir; }
    publico texto   Categoria            { obter; definir; }
    publico inteiro QuantidadeTotal      { obter; definir; }
    publico inteiro QuantidadeDisponivel { obter; definir; }
    publico booleano Disponivel          { obter; definir; }

    // Construtor com parâmetros opcionais
    publico Livro(texto titulo,
                  texto autor,
                  texto isbn,
                  inteiro ano = 2024,
                  texto categoria = "Geral",
                  inteiro quantidade = 1) {

        Titulo               = titulo;
        Autor                = autor;
        ISBN                 = isbn;
        AnoPublicacao        = ano;
        Categoria            = categoria;
        QuantidadeTotal      = quantidade;
        QuantidadeDisponivel = quantidade;
        Disponivel           = verdadeiro;
    }

    publico vazio apresentarDetalhes(booleano completo = verdadeiro) 
    {
        se (completo) 
        {
            imprima("📚 LIVRO: " + Titulo);
            imprima(" Autor: "      + Autor);
            imprima(" ISBN: "       + ISBN);
            imprima(" Ano: "        + AnoPublicacao);
            imprima(" Categoria: "  + Categoria);
            imprima(" Disponível: " +
                    QuantidadeDisponivel + "/" + QuantidadeTotal);

            se (Disponivel) 
            {
                imprima(" Status: Disponível");
            } 
            senão 
            {
                imprima(" Status: Indisponível");
            }
        } 
        senão 
        {
            se (Disponivel) 
            {
                imprima($"📚 {Titulo} - {Autor} ✅");
            } 
            senão 
            {
                imprima($"📚 {Titulo} - {Autor} ❌");
            }
        }
    }

    publico booleano emprestar() 
    {
        se (QuantidadeDisponivel > 0) 
        {
            QuantidadeDisponivel = QuantidadeDisponivel - 1;
            se (QuantidadeDisponivel == 0) 
            {
                Disponivel = falso;
            }
            retorne verdadeiro;
        }
        retorne falso;
    }

    publico vazio devolver() 
    {
        QuantidadeDisponivel = QuantidadeDisponivel + 1;
        se (QuantidadeDisponivel > 0) 
        {
            Disponivel = verdadeiro;
        }
    }
}

publico classe Usuario 
{

    publico texto   Nome            { obter; definir; }
    publico texto   Email           { obter; definir; }
    publico texto   Telefone        { obter; definir; }
    publico texto   TipoUsuario     { obter; definir; }
    publico inteiro NumeroCartao    { obter; definir; }
    publico inteiro LimiteEmprestimos { obter; definir; }
    publico inteiro LivrosEmprestados { obter; definir; }

    publico Usuario(texto nome,
                    texto email,
                    texto telefone = "",
                    texto tipo = "Comum",
                    inteiro limite = 3) 
    { 

        Nome              = nome;
        Email             = email;
        Telefone          = telefone;
        TipoUsuario       = tipo;
        LimiteEmprestimos = limite;
        LivrosEmprestados = 0;
        NumeroCartao      = 1000 + (nome.tamanho() * 7);
    }

    publico vazio apresentarPerfil() 
    {
        imprima("👤 USUÁRIO: " + Nome);
        imprima(" Email: "      + Email);
        imprima(" Cartão: #"    + NumeroCartao);
        imprima(" Tipo: "       + TipoUsuario);
        imprima(" Empréstimos: "+ LivrosEmprestados + "/" + LimiteEmprestimos);
    }

    publico booleano podeEmprestar() 
    {
        retorne LivrosEmprestados < LimiteEmprestimos;
    }
}

publico classe Biblioteca 
{

    publico texto   Nome         { obter; definir; }
    publico texto   Endereco     { obter; definir; }
    publico inteiro TotalLivros  { obter; definir; }
    publico inteiro TotalUsuarios { obter; definir; }

    publico Biblioteca(texto nome,
                       texto endereco = "Endereço não informado") 
{

        Nome          = nome;
        Endereco      = endereco;
        TotalLivros   = 0;
        TotalUsuarios = 0;
    }

    publico vazio adicionarLivro(Livro livro) 
    {
        TotalLivros = TotalLivros + 1;
        imprima("✅ Livro '" + livro.Titulo + "' adicionado à biblioteca!");
    }

    publico vazio cadastrarUsuario(Usuario usuario) 
    {
        TotalUsuarios = TotalUsuarios + 1;
        imprima("✅ Usuário '" + usuario.Nome + "' cadastrado com sucesso!");
        imprima(" Número do cartão: #" + usuario.NumeroCartao);
    }

    publico vazio realizarEmprestimo(Usuario usuario, Livro livro) 
    {
        se (usuario.podeEmprestar()) 
        {
            se (livro.emprestar()) 
            {
                usuario.LivrosEmprestados = usuario.LivrosEmprestados + 1;
                imprima("📖 EMPRÉSTIMO REALIZADO:");
                imprima(" Livro: "   + livro.Titulo);
                imprima(" Usuário: " + usuario.Nome);
                imprima(" Cartão: #" + usuario.NumeroCartao);
            } 
            senão 
            {
                imprima("❌ Livro '" + livro.Titulo + "' não está disponível!");
            }
        } 
        senão 
        {
            imprima("❌ Usuário '" + usuario.Nome + "' atingiu o limite de empréstimos!");
        }
    }

    publico vazio realizarDevolucao(Usuario usuario, Livro livro) 
    {
        livro.devolver();
        usuario.LivrosEmprestados = usuario.LivrosEmprestados - 1;
        imprima("📥 DEVOLUÇÃO REALIZADA:");
        imprima(" Livro: "   + livro.Titulo);
        imprima(" Usuário: " + usuario.Nome);
    }

    publico vazio gerarRelatorio() 
    {
        imprima("📊 ========== RELATÓRIO DA BIBLIOTECA ==========");
        imprima("🏛️ Biblioteca: " + Nome);
        imprima("📍 Endereço: "   + Endereco);
        imprima("📚 Total de Livros: " + TotalLivros);
        imprima("👥 Total de Usuários: " + TotalUsuarios);
        imprima("============================================");
    }

    publico vazio buscarLivrosPorCategoria(texto categoria) 
    {
        imprima("🔍 Buscando livros da categoria: " + categoria);
        imprima(" (Simulação - em implementação real buscaria no banco de dados)");
    }
}

// ------------------------------------------------------------------
// DEMONSTRAÇÃO DO SISTEMA
// ------------------------------------------------------------------
publico função demonstrarSistema() 
{

    imprima("🎯 ===== SISTEMA DE BIBLIOTECA DIGITAL =====");
    imprima("");

    // Criando biblioteca
    Livro livro1 = novo Livro("1984", "George Orwell", "978-85-250-4099-1", 1949, "Ficção Científica", 2);
    Livro livro2 = novo Livro("Clean Code", "Robert Martin", "978-0-13-235088-4", 2008, "Tecnologia");
    Livro livro3 = novo Livro("O Pequeno Príncipe", "Antoine de Saint-Exupéry", "978-85-325-2734-9");

    // Adicionando livros
    biblioteca.adicionarLivro(livro1);
    biblioteca.adicionarLivro(livro2);
    biblioteca.adicionarLivro(livro3);
    imprima("");

    // Criando usuários
    Usuario usuario1 = novo Usuario("Ana Silva",
                                    "ana.silva@email.com",
                                    "11987654321",
                                    "Premium",
                                    5);

    Usuario usuario2 = novo Usuario("Joana Silva",
                                    "joana@email.com",
                                    "11876543210");

    Usuario usuario3 = novo Usuario("Maria Oliveira",
                                    "maria@email.com");

    // Cadastrando usuários
    biblioteca.cadastrarUsuario(usuario1);
    biblioteca.cadastrarUsuario(usuario2);
    biblioteca.cadastrarUsuario(usuario3);
    imprima("");

    // Catálogo
    imprima("📋 CATÁLOGO DE LIVROS:");
    livro1.apresentarDetalhes();
    imprima("");
    livro2.apresentarDetalhes(falso);
    livro3.apresentarDetalhes(falso);
    livro4.apresentarDetalhes(falso);
    imprima("");

    // Perfis
    imprima("👥 USUÁRIOS CADASTRADOS:");
    usuario1.apresentarPerfil();
    imprima("");
    usuario2.apresentarPerfil();
    imprima("");

    // Empréstimos
    imprima("📖 REALIZANDO EMPRÉSTIMOS:");
    biblioteca.realizarEmprestimo(usuario1, livro1);
    biblioteca.realizarEmprestimo(usuario1, livro2);
    biblioteca.realizarEmprestimo(usuario2, livro1); // Falhará
    biblioteca.realizarEmprestimo(usuario2, livro3);
    imprima("");

    // Status após empréstimos
    imprima("📊 STATUS APÓS EMPRÉSTIMOS:");
    livro1.apresentarDetalhes(falso);
    livro2.apresentarDetalhes(falso);
    livro3.apresentarDetalhes(falso);
    imprima("");

    // Devolução
    imprima("📥 REALIZANDO DEVOLUÇÕES:");
    biblioteca.realizarDevolucao(usuario1, livro1);
    imprima("");

    // Status final
    livro1.apresentarDetalhes(falso);
    imprima("");

    // Relatório
    biblioteca.gerarRelatorio();

    // Busca
    biblioteca.buscarLivrosPorCategoria("Literatura");
    biblioteca.buscarLivrosPorCategoria("Tecnologia");
    imprima("");

    imprima("✨ Sistema funcionando perfeitamente!");

    // Estatísticas
    inteiro totalOperacoes = 0;
    imprima("🧮 CONTABILIZANDO OPERAÇÕES DO SISTEMA:");
    totalOperacoes = totalOperacoes + 4; // Livros adicionados
    totalOperacoes = totalOperacoes + 3; // Usuários cadastrados
    totalOperacoes = totalOperacoes + 4; // Empréstimos tentados
    totalOperacoes = totalOperacoes + 1; // Devoluções
    imprima("📈 Total de operações realizadas: " + totalOperacoes);

    se (totalOperacoes > 10) {
        imprima("🎯 Sistema com alta atividade!");
    } senão {
        imprima("📊 Sistema com atividade moderada.");
    }

    imprima("🏁 ===== FIM DA DEMONSTRAÇÃO =====");
}

publico função principal() 
{
    demonstrarSistema();
}

} // fim do espaco BibliotecaDigital.Sistema

// Executando o sistema
espaco principal {
    função inicio() {
        BibliotecaDigital.Sistema.demonstrarSistema();
        inteiro totalOperacoes = 0;
        imprima("🧮 CONTABILIZANDO OPERAÇÕES DO SISTEMA:");
        totalOperacoes = totalOperacoes + 4; // Livros adicionados
        totalOperacoes = totalOperacoes + 3; // Usuários cadastrados
        totalOperacoes = totalOperacoes + 4; // Empréstimos tentados
        totalOperacoes = totalOperacoes + 1; // Devoluções
        imprima($"📈 Total de operações realizadas: {totalOperacoes}");
        se (totalOperacoes > 10) {
            imprima("🎯 Sistema com alta atividade!");
        } senão {
            imprima("📊 Sistema com atividade moderada.");
        }
        imprima("🏁 ===== FIM DA DEMONSTRAÇÃO =====");
    }
}
```

## 🧩 Extensões e Ferramentas para VS Code

- [Servidor de Linguagem Por do Sol (LSP)](https://github.com/Adriano-Severino/pordosol-language-server)
- [Extensão oficial VS Code: linguagem-portugues-por-do-sol](https://github.com/Adriano-Severino/linguagem-portugues-por-do-sol)

Essas extensões fornecem realce de sintaxe, auto-complete, diagnósticos e integração moderna para desenvolvimento com a linguagem Por do Sol no VS Code.

## 🏗️ Estrutura do Projeto

```text
compilador-portugues/
├── src/
│   ├── main.rs           # Ponto de entrada do compilador
│   ├── lexer.rs          # Analisador léxico (geração de tokens)
│   ├── ast.rs            # Definições da Árvore Sintática Abstrata (AST)
│   ├── parser.lalrpop    # Gramática da linguagem para LALRPOP (ou similar)
│   └── codegen.rs        # Geração de código LLVM IR
├── build_production.sh   # Script para compilação completa de programas .pr
├── compile_fast.sh       # (Opcional) Script para compilação rápida durante desenvolvimento
├── Cargo.toml            # Manifesto do projeto Rust
├── build.rs              # (Opcional) Script de build do Cargo
└── README.md             # Este arquivo
```

## 🤝 Contribuindo

Contribuições são muito bem-vindas! Para contribuir:

1. Faça um fork do repositório
2. Clone sua fork:
    ```bash
    git clone https://github.com/Adriano-Severino/compilador-portugues
    ```
3. Crie uma branch para sua feature:
    ```bash
    git checkout -b minha-nova-feature
    ```
4. Faça suas mudanças e adicione testes, se aplicável.
5. Faça um commit das suas mudanças:
    ```bash
    git commit -m "Adiciona nova feature incrível"
    ```
6. Faça um push para sua fork:
    ```bash
    git push origin minha-nova-feature
    ```
7. Abra um Pull Request no repositório original.

## Diretrizes para Contribuição

- Mantenha a sintaxe da linguagem e dos comentários em português brasileiro.
- Adicione testes para novas funcionalidades ou correções de bugs.
- Documente quaisquer mudanças significativas no README.md ou em comentários no código.
- Siga o estilo de código existente.

## 🐛 Reportando Problemas

Encontrou um bug ou tem alguma sugestão? Abra uma Issue [neste link](https://github.com/Adriano-Severino/compilador-portugues) com:

1. Descrição detalhada do problema ou sugestão.
2. Passos para reproduzir o erro (se for um bug).
3. Informações do seu ambiente de desenvolvimento (versão do Rust, sistema operacional, etc).
4. Se possível, forneça um exemplo de código que reproduz o problema.
5. Saída esperada vs. saída atual.

## 📝 Licença

Este projeto está licenciado sob a Licença MIT - veja o arquivo LICENSE para detalhes.

## Agradecimentos

- À comunidade Rust por suas ferramentas e ecossistema incríveis.
- Ao projeto LLVM por fornecer uma infraestrutura de compilação robusta e poderosa.
- Aos educadores e estudantes brasileiros que inspiram e podem se beneficiar deste projeto.

⭐ Se este projeto foi útil, deixe uma estrela!

🌟 Ajude a democratizar a programação em português!
