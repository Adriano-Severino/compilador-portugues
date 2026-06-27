// src/main.rs

use std::env;
use std::fmt;
use std::fs;
use std::path::Path;
use std::process::Command;

// Declaração dos módulos do projeto
mod ast;
mod codegen;
mod inferencia_tipos;
mod interpolacao;
mod lexer;
mod module_system;
mod ownership;
mod stdlib;
mod type_checker;

// Parser LALRPOP
use lalrpop_util::lalrpop_mod;
lalrpop_mod!(pub parser);
use logos::Logos;

//LLVM (Já funcional):
//cargo run --bin compilador -- teste.pr --target=llvm-ir
//clang teste.ll -o teste
//./teste

//CIL Bytecode:
//cargo run --bin compilador -- teste.pr --target=cil-bytecode
//# Se tiver o 'ilasm' (parte do .NET Framework ou Mono)
//ilasm teste.il /exe /output:teste-cil.exe
//# Para executar (no Windows)
//./teste-cil.exe
//# Ou com Mono
//mono teste-cil.exe

//Console .NET:
//cargo run --bin compilador -- teste.pr --target=console
//cd teste # Entra no diretório do projeto gerado
//dotnet run

//Bytecode Customizado:
//cargo run -- teste.pr --target=bytecode
//cat teste.pbc # Para ver o bytecode gerado

//para executar o bytecode:
//Gere o Bytecode:
//cargo run --bin compilador -- teste.pr --target=bytecode
//cargo run --bin interpretador -- teste.pbc

//help
//cargo run --bin compilador
//cargo run --bin compilador -- --help

// Struct de erro customizada
#[derive(Debug)]
struct CompilerError(String);

impl fmt::Display for CompilerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl std::error::Error for CompilerError {}

// Enum para os alvos de compilação
#[derive(Debug, Clone)]
enum TargetCompilacao {
    Universal,
    LlvmIr,
    CilBytecode,
    Console,
    Bytecode,
}

//Função para exibir a ajuda
fn exibir_ajuda() {
    print!(
        "Compilador da Linguagem em Português (v0.1.0)
=============================================

Uso: compilador <arquivo.pr> [OPÇÃO]

OPÇÕES:
  --target=<alvo>    Define o formato de saída da compilação.
  --help             Exibe esta mensagem de ajuda.

ALVOS DISPONÍVEIS:
  llvm-ir            Gera código intermediário LLVM (.ll), otimizado para compilação nativa com Clang.
  cil-bytecode       Gera código CIL (.il) para a plataforma .NET.
  console            Cria um projeto de console .NET completo, pronto para ser executado com 'dotnet run'.
  bytecode           Gera um arquivo de bytecode customizado (.pbc) para ser executado pelo interpretador.
  universal          Executa a compilação para todos os alvos disponíveis (padrão).

EXEMPLOS DE USO:
  # Compilar para LLVM IR e gerar um executável nativo
  cargo run --bin compilador -- teste.pr --target=llvm-ir
  clang teste.ll -o teste_nativo

  # Criar e executar um projeto de console .NET
  cargo run --bin compilador -- teste.pr --target=console
  cd teste && dotnet run

  # Gerar bytecode e executá-lo com o interpretador
  cargo run --bin compilador -- teste.pr --target=bytecode
  cargo run --bin interpretador -- teste.pbc
"
    );
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("DEBUG: O compilador iniciou a execução.");
    let args: Vec<String> = env::args().collect();

    if args.len() <= 1 || args.contains(&"--help".to_string()) {
        exibir_ajuda();
        return Ok(());
    }

    let caminhos_arquivos: Vec<String> = args
        .iter()
        .skip(1)
        .filter(|arg| arg.trim_matches('"').ends_with(".pr"))
        .map(|arg| arg.trim_matches('"').to_string())
        .collect();

    if caminhos_arquivos.is_empty() {
        eprintln!("Erro: Nenhum arquivo de entrada (.pr) especificado.");
        exibir_ajuda();
        return Err(Box::new(CompilerError("Nenhum arquivo de entrada".into())));
    }

    let target = args
        .iter()
        .find(|arg| arg.starts_with("--target="))
        .map(|arg| arg.split('=').nth(1).unwrap_or("universal"))
        .map(|t| match t {
            "llvm-ir" => TargetCompilacao::LlvmIr,
            "cil-bytecode" => TargetCompilacao::CilBytecode,
            "console" => TargetCompilacao::Console,
            "bytecode" => TargetCompilacao::Bytecode,

            _ => TargetCompilacao::Universal,
        })
        .unwrap_or(TargetCompilacao::Universal);

    // --- Nova Lógica de Compilação em Fases ---

    // Fase 1: Ler todos os arquivos para a memória.
    // Lê e sanitiza cada arquivo, removendo comandos de shell acidentalmente colados após ponto-e-vírgula.
    // Exemplo suportado: "chamada();cargo run --bin compilador -- arquivo.pr --target=bytecode" -> "chamada();"
    fn sanitizar_codigo(orig: String) -> String {
        let mut resultado = String::with_capacity(orig.len());
        for linha in orig.lines() {
            // Procura padrões após ';' que iniciem comandos externos comuns e corta a linha neste ponto.
            let mut corte = linha.len();
            for marcador in [
                ";cargo ",
                ";dotnet ",
                ";clang ",
                ";echo ",
                ";./",
                ";interpretador ",
            ] {
                if let Some(idx) = linha.find(marcador) {
                    if idx < corte {
                        corte = idx + 1;
                    } // mantém o ';'
                }
            }
            if corte < linha.len() {
                resultado.push_str(&linha[..corte]);
            } else {
                resultado.push_str(linha);
            }
            resultado.push('\n');
        }
        resultado
    }

    let codigos: Vec<String> = caminhos_arquivos
        .iter()
        .map(|p| fs::read_to_string(p).map(sanitizar_codigo))
        .collect::<Result<_, _>>()?;

    // Fase 2: Parsear todos os arquivos para ASTs.
    let mut asts = Vec::new();
    for (caminho, codigo) in caminhos_arquivos.iter().zip(codigos.iter()) {
        let lexer = lexer::Token::lexer(codigo);
        let tokens_result: Result<Vec<_>, _> = lexer
            .spanned()
            .map(|(token, span)| token.map(|t| (span.start, t, span.end)))
            .collect();

        let tokens = match tokens_result {
            Ok(tokens) => tokens,
            Err(_) => {
                return Err(Box::new(CompilerError(format!(
                    "Erro Léxico: Token inválido encontrado em '{}'",
                    caminho
                ))))
            }
        };

        let parser = parser::ArquivoParser::new();
        let mut ast = parser.parse(tokens.iter().cloned()).map_err(|e| {
            Box::new(CompilerError(format!(
                "Erro sintático em '{}': {:?}",
                caminho, e
            )))
        })?;

        crate::interpolacao::walk_programa(&mut ast, |e| {
            *e = interpolacao::planificar_interpolada(e.clone());
        });
        asts.push(ast);
    }

    // Fase 3: Juntar ASTs para uma análise semântica unificada.
    let mut programa_final = ast::Programa {
        usings: vec![],
        namespaces: vec![],
        declaracoes: vec![],
    };
    for mut ast in asts {
        programa_final.declaracoes.extend(ast.declaracoes);
        programa_final.usings.extend(ast.usings);

        for ns_para_mesclar in ast.namespaces.drain(..) {
            if let Some(ns_existente) = programa_final
                .namespaces
                .iter_mut()
                .find(|n| n.nome == ns_para_mesclar.nome)
            {
                // Namespace já existe, mescla as declarações.
                ns_existente.declaracoes.extend(ns_para_mesclar.declaracoes);
            } else {
                // Namespace novo, adiciona à lista.
                programa_final.namespaces.push(ns_para_mesclar);
            }
        }
    }

    // Fase 4: Análise semântica no AST combinado.
    let mut type_checker = type_checker::VerificadorTipos::new();
    if let Err(erros) = type_checker.verificar_programa(&programa_final) {
        for erro in erros {
            eprintln!("Erro Semântico: {}", erro);
        }
        return Err(Box::new(CompilerError(
            "Houve erros semânticos.".to_string(),
        )));
    }

    // Fase 5: Geração de código.
    let nome_base = Path::new(&caminhos_arquivos[0])
        .file_stem()
        .unwrap_or_default()
        .to_str()
        .unwrap_or("saida");
    match target {
        TargetCompilacao::Universal => {
            compilar_universal(&programa_final, &mut type_checker, nome_base)
        }
        TargetCompilacao::LlvmIr => {
            compilar_para_llvm_ir(&programa_final, &mut type_checker, nome_base)?;
            println!("Compilando com clang...");
            let output = Command::new("clang")
                .arg(format!("{}.ll", nome_base))
                .arg("-o")
                .arg(nome_base)
                .output()
                .map_err(|e| Box::new(CompilerError(format!("Falha ao executar clang: {}", e))))?;

            if !output.status.success() {
                return Err(Box::new(CompilerError(format!(
                    "Erro ao compilar LLVM IR com clang:\nstdout: {}\nstderr: {}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                ))));
            }
            println!("Executável gerado: ./{}", nome_base);
            Ok(())
        }
        TargetCompilacao::CilBytecode => compilar_para_cil_bytecode(&programa_final, nome_base),
        TargetCompilacao::Console => compilar_para_console(&programa_final, nome_base),
        TargetCompilacao::Bytecode => {
            compilar_para_bytecode(&programa_final, &mut type_checker, nome_base)
        }
    }
}

fn compilar_universal<'a>(
    ast: &'a ast::Programa,
    type_checker: &'a mut type_checker::VerificadorTipos<'a>,
    nome_base: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🌍 Iniciando Compilação Universal...");
    compilar_para_llvm_ir(ast, &mut type_checker.clone(), nome_base)?;
    compilar_para_cil_bytecode(ast, nome_base)?;
    compilar_para_console(ast, nome_base)?;
    compilar_para_bytecode(ast, type_checker, nome_base)?;
    println!("\n🎉 Compilação Universal Concluída!");
    Ok(())
}

fn compilar_para_llvm_ir<'a>(
    programa: &'a ast::Programa,
    type_checker: &'a mut type_checker::VerificadorTipos<'a>,
    nome_base: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔧 Gerando LLVM IR...");
    let mut gerador = codegen::llvm_ir::LlvmGenerator::new(
        programa,
        type_checker,
        &type_checker.resolved_classes,
    );
    let llvm_ir = gerador.generate();
    fs::write(format!("{}.ll", nome_base), llvm_ir)?;
    println!("  ✓ {}.ll gerado.", nome_base);
    println!("  Para compilar: clang {0}.ll -o {0}", nome_base);
    println!("🎯 Pipeline LLVM: AST → LLVM IR → Código de Máquina");
    println!("Para executar: ./{}", nome_base);
    Ok(())
}

fn compilar_para_cil_bytecode<'a>(
    ast: &'a ast::Programa,
    nome_base: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔧 Gerando CIL Bytecode...");
    let gerador = codegen::GeradorCodigo::new()?;
    gerador
        .gerar_cil(ast, nome_base)
        .map_err(|e| Box::new(CompilerError(e)))?;
    println!("  ✓ {}.il gerado.", nome_base);
    println!(
        "  Para compilar: ilasm {0}.il /exe /output:{0}.exe",
        nome_base
    );
    Ok(())
}

fn compilar_para_console<'a>(
    ast: &'a ast::Programa,
    nome_base: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔧 Gerando Projeto de Console .NET...");
    let gerador = codegen::GeradorCodigo::new()?;
    gerador
        .gerar_console(ast, nome_base)
        .map_err(|e| Box::new(CompilerError(e)))?;
    println!("  ✓ Projeto '{}' gerado.", nome_base);
    println!("  Para executar: cd {} && dotnet run", nome_base);
    Ok(())
}

fn compilar_para_bytecode<'a>(
    ast: &'a ast::Programa,
    type_checker: &'a mut type_checker::VerificadorTipos,
    nome_base: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔧 Gerando Bytecode Customizado...");
    let mut gerador = codegen::GeradorCodigo::new()?;
    gerador
        .gerar_bytecode(ast, type_checker, nome_base)
        .map_err(|e| Box::new(CompilerError(e)))?;
    println!("  ✓ {}.pbc gerado.", nome_base);
    println!(" ✓ Executando o bytecode...");
    println!("Você pode executar o bytecode usando o interpretador personalizado.");
    println!(
        "Execute: cargo run --bin interpretador -- {}.pbc",
        nome_base
    );
    println!("ou use o comando:");
    println!("Para executar: interpretador {}.pbc", nome_base);
    Ok(())
}
