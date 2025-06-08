# Meu Compilador - Por do sol

Este é o repositório do projeto "compilador-portugues", que desenvolve um compilador para uma linguagem de programação moderna escrita em português brasileiro.

## 📖 Sobre a Linguagem

Esta linguagem "por do sol" foi desenvolvida com foco acadêmico e educacional, visando democratizar o ensino de programação no Brasil através de uma sintaxe em português. No entanto, ela também é projetada para ser versátil o suficiente para desenvolvimento de aplicações desktop nativas com alta performance, graças à geração de código LLVM.

### 🎯 Objetivo Principal

Criar uma linguagem de programação que seja:

*   Acessível para estudantes brasileiros iniciantes.
*   Moderna com sintaxe inspirada em C#.
*   Performática gerando código nativo via LLVM.
*   Educacional mas capaz de projetos reais.

### 🚀 Recursos Principais

*   Tipagem estática forte para maior segurança e detecção precoce de erros.
*   Sintaxe em português inspirada em C#, adaptada para falantes nativos.
*   Geração de código LLVM eficiente para performance nativa.
*   Estruturas de controle como `se`, `então`, `senão`, `enquanto` e blocos com `{}`.
*   Suporte completo a variáveis (`inteiro`, `texto`, `booleano`) com atribuições.
*   Expressões aritméticas e de comparação (`+`, `-`, `*`, `/`, `>`, `<`, `==`, `!=`).
*   Compilação multiplataforma para código nativo executável.
*   Comentários com `//` (linha) e `/* */` (bloco).

## 📋 Pré-requisitos

Antes de começar, certifique-se de que você tem os seguintes softwares instalados:

*   **Rust (versão 1.70+):** Necessário para construir o compilador.
    *   Para instalar o Rust, use o `rustup`:
        ```bash
        curl --proto '=https' --tlsv1.2 -sSf [https://sh.rustup.rs](https://sh.rustup.rs) | sh
        ```
*   **LLVM 16:** A linguagem depende especificamente desta versão.
    *   Para Ubuntu/Debian:
        ```bash
        sudo apt-get update
        sudo apt-get install llvm-16 llvm-16-dev clang-16
        ```
    *   É crucial que a variável de ambiente `LLVM_SYS_160_PREFIX` esteja configurada corretamente, apontando para o diretório de instalação do LLVM 16. Por exemplo:
        ```bash
        export LLVM_SYS_160_PREFIX=/usr/lib/llvm-16 
        # Adicione esta linha ao seu ~/.bashrc ou ~/.zshrc para tornar a configuração permanente
        ```
*   **Clang:** Usado para compilar o código LLVM IR gerado para um executável nativo (geralmente incluído com as ferramentas de desenvolvimento do LLVM).
*   **opt:** Ferramenta de otimização do LLVM, usada para otimizar o código LLVM IR (também parte do toolchain LLVM).

## ⚙️ Instalação e Configuração

1.  **Clone o repositório:**
    ```bash
    git clone https://github.com/Adriano-Severino/compilador-portugues
    cd compilador-portugues 
    ```

2.  **Configure o ambiente LLVM:**
    Certifique-se de que o LLVM 16 está instalado e a variável `LLVM_SYS_160_PREFIX` está definida no seu ambiente, como mostrado na seção de Pré-requisitos.

3.  **Construa o compilador:**
    ```bash
    cargo build --release
    ```
    O executável do compilador estará em `target/release/compilador-portugues` (o nome pode variar dependendo do nome do seu crate no `Cargo.toml`).

## 📝 Como Usar

Os programas na sua linguagem devem ser escritos em arquivos com a extensão `.pr`.

### Estrutura Básica de um Programa

```Por do sol
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
O projeto inclui um script build_production.sh para facilitar o processo completo de compilação.

1. Crie um arquivo com seu código, por exemplo, meu_programa.pr.
2. Execute o script de compilação (não inclua a extensão .pr ao chamar o script):
    ```bash
    ./build_production.sh meu_programa
    ```
Este script realizará os seguintes passos:
    *   Compilará meu_programa.pr para LLVM IR (meu_programa.ll) usando cargo run --release -- meu_programa.pr (ou o executável do compilador diretamente).
    *   Otimizará o código LLVM IR para meu_programa_opt.ll usando opt.
    *   Compilará meu_programa_opt.ll para um executável nativo (meu_programa) usando clang.
    *   Tentará gerar um executável estático (meu_programa_static).

3. Execute seu programa:
    ```bash
    ./meu_programa
    ```
## Passos Manuais de compilação: (para Entender o Processo)
Se você quiser entender o que o script build_production.sh faz:

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
💡 Exemplos de Código

Olá, Mundo!
Código (ola_mundo.pr):

```bash
imprima("Olá, Mundo!");
imprima("Bem-vindo à programação em português!");
```
Compilar e executar:
```bash
./build_production.sh ola_mundo
./ola_mundo
```
Saída esperada:
```bash
Olá, Mundo!
Bem-vindo à programação em português!
```
Variáveis
Código (variaveis.pr):
```bash
// Declaração e inicialização de variáveis
inteiro idade = 21;
texto nome = "João Silva";
booleano estudante = verdadeiro;

// Exibindo valores
imprima("=== Informações Pessoais ===");
imprima(nome);
imprima(idade);

// Modificando variáveis
idade = idade + 1;
nome = "João Santos";

imprima("=== Após Mudanças ===");
imprima(nome);
imprima(idade);
```
Estruturas Condicionais
Código (condicionais.pr):
```bash
// Declaração e inicialização de variáveis
inteiro nota = 85;
texto nome = "Ana";

imprima("=== Sistema de Avaliação ===");
imprima(nome);
imprima(nota);

se nota >= 90 então {
    imprima("Excelente! Parabéns!");
} senão {
    se nota >= 70 então {
        imprima("Bom trabalho!");
    } senão {
        imprima("Precisa melhorar.");
    }
}

// Comparações múltiplas
se nota > 60 && nota < 100 então {
    imprima("Nota válida aprovada");
}
```
Operações Aritméticas
Código (operacoes_aritmeticas.pr):
```bash
inteiro a = 15;
inteiro b = 4;

imprima("=== Calculadora Básica ===");
imprima("Número A:");
imprima(a);
imprima("Número B:");
imprima(b);

imprima("Soma:");
imprima(a + b);

imprima("Subtração:");
imprima(a - b);

imprima("Multiplicação:");
imprima(a * b);

imprima("Divisão:");
imprima(a / b); // Divisão inteira

// Operações compostas
inteiro resultado = (a + b) * 2;
imprima("(A + B) * 2 =");
imprima(resultado);
```
Loops e Contadores
Código (loops.pr):
```bash
inteiro contador = 1;
inteiro limite = 5;

imprima("=== Contagem de 1 a 5 ===");

enquanto contador <= limite {
    imprima("Contador:");
    imprima(contador);
    contador = contador + 1;
}

imprima("Contagem finalizada!");

// Exemplo com condição mais complexa
inteiro numero = 2;
imprima("=== Números Pares até (menor que) 20 ===");
enquanto numero < 20 {
    // Assumindo que '%' é o operador módulo para verificar paridade.
    // Se sua linguagem não tiver '%', a lógica de paridade precisará ser adaptada.
    // Exemplo de verificação de paridade sem '%': se (numero / 2) * 2 == numero então { ... }
    se numero % 2 == 0 então { 
        imprima("Par:");
        imprima(numero);
    }
    numero = numero + 2;
}
```
Programa Completo - Sistema de Notas
Código (sistema_notas.pr):
```bash
// Sistema simples de avaliação de estudantes
texto estudante = "Joana";
inteiro nota1 = 85;
inteiro nota2 = 92;
inteiro nota3 = 78;

imprima("=== Sistema de Avaliação Acadêmica ===");
imprima("Estudante:");
imprima(estudante);

imprima("Notas individuais:");
imprima(nota1);
imprima(nota2);
imprima(nota3);

// Calcular média
inteiro soma = nota1 + nota2 + nota3;
inteiro media = soma / 3; // Divisão inteira

imprima("Média:");
imprima(media);

// Determinar situação
se media >= 90 então {
    imprima("Situação: EXCELENTE");
} senão {
    se media >= 80 então {
        imprima("Situação: BOM");
    } senão {
        se media >= 70 então {
            imprima("Situação: REGULAR");
        } senão {
            imprima("Situação: INSUFICIENTE");
        }
    }
}

// Verificar se passou
se media >= 70 então {
    imprima("Status: APROVADO");
} senão {
    imprima("Status: REPROVADO");
}
```
📚 Referência da Linguagem
Tipos de Dados

| Tipo | Exemplo | Descrição | |:----------|:-----------------|:----------------------------| | inteiro | 42, -10 | Números inteiros de 64 bits | | texto | "Olá" | Strings de texto | | booleano| verdadeiro, falso | Valores lógicos |

Operadores

| Categoria | Operadores | Exemplo | |:------------|:------------------|:--------------------| | Aritméticos | +, -, *, / | a + b * c | | Comparação | >, <, >=, <=, ==, != | idade >= 18 | | Lógicos | &&, ||, ! | a > 0 && b < 10 | | Atribuição | = | idade = 25 |

Estruturas de Controle
```bash
se condicao então comando;
```

Condicional com Bloco:
```bash
se condicao então {
    // comandos se verdadeiro
}
```
Condicional Completa:
```bash
se condicao então {
    // comandos se verdadeiro
} senão {
    // comandos se falso
}
```

Loop enquanto:
```bash
enquanto condicao {
    // comandos
}
```
🏗️ Estrutura do Projeto

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

🤝 Contribuindo
Contribuições são muito bem-vindas! Para contribuir:

1. Faça um fork do repositório
2. Clone sua fork:
    ```bash
   git clone https://github.com/Adriano-Severino/compilador-portugues
    ```
3. Crie uma branch para sua feature:
    ```bash
    git checkout -b minha-nova-feature.
    ```
4. Faça suas mudanças e adicione testes, se aplicável.
5. Faça um commit das suas mudanças:
    ```bash
    git commit -m "Adiciona nova feature incrível".
    ```
6. Faça um push para sua fork:
    ```bash
    Faça o push para a branch: git push origin minha-nova-feature.
    ```
7. Abra um Pull Request no repositório original.

## Diretrizes para Contribuição
 Mantenha a sintaxe da linguagem e dos comentários em português brasileiro.
 Adicione testes para novas funcionalidades ou correções de bugs.
 Documente quaisquer mudanças significativas no README.md ou em comentários no código.
 Siga o estilo de código existente.

## 🐛 Reportando Problemas
 Encontrou um bug ou tem alguma sugestão? Abra uma Issue https://github.com/Adriano-Severino/compilador-portugues
 com:

1. Descrição detalhada do problema ou sugestão.
2. Passos para reproduzir o erro (se for um bug).
3. Informações do seu ambiente de desenvolvimento (versão do Rust, sistema operacional, etc).
4. Se possível, forneça um exemplo de código que reproduz o problema.
5. Saída esperada vs. saída atual.

## 📝 Licença
Este projeto está licenciado sob a Licença MIT - veja o arquivo LICENSE para detalhes.

## Agradecimentos

À comunidade Rust por suas ferramentas e ecossistema incríveis.
Ao projeto LLVM por fornecer uma infraestrutura de compilação robusta e poderosa.
Aos educadores e estudantes brasileiros que inspiram e podem se beneficiar deste projeto.

⭐ Se este projeto foi útil, deixe uma estrela!

🌟 Ajude a democratizar a programação em português!