// src/main.rs

use std::env;
use std::fs;
use std::path::Path;
use std::fmt;

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

// ✅ NOVO: Função para exibir a ajuda
fn exibir_ajuda() {
    println!("Compilador da Linguagem em Português (v0.1.0)");
    println!("=============================================\n");
    println!("Uso: compilador <arquivo.pr> [OPÇÃO]");
    println!("\nOPÇÕES:");
    println!("  --target=<alvo>    Define o formato de saída da compilação.");
    println!("  --help             Exibe esta mensagem de ajuda.\n");
    println!("ALVOS DISPONÍVEIS:");
    println!("  llvm-ir            Gera código intermediário LLVM (.ll), otimizado para compilação nativa com Clang.");
    println!("  cil-bytecode       Gera código CIL (.il) para a plataforma .NET.");
    println!("  console            Cria um projeto de console .NET completo, pronto para ser executado com 'dotnet run'.");
    println!("  bytecode           Gera um arquivo de bytecode customizado (.pbc) para ser executado pelo interpretador.");
    println!("  universal          Executa a compilação para todos os alvos disponíveis (padrão).\n");
    println!("EXEMPLOS DE USO:");
    println!("  # Compilar para LLVM IR e gerar um executável nativo");
    println!("  cargo run --bin compilador -- teste.pr --target=llvm-ir");
    println!("  clang teste.ll -o teste_nativo\n");
    println!("  # Criar e executar um projeto de console .NET");
    println!("  cargo run --bin compilador -- teste.pr --target=console");
    println!("  cd teste && dotnet run\n");
    println!("  # Gerar bytecode e executá-lo com o interpretador");
    println!("  cargo run --bin compilador -- teste.pr --target=bytecode");
    println!("  cargo run --bin interpretador -- teste.pbc");
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    
    // ✅ CORREÇÃO: Tratar o caso de nenhum argumento ou --help
    if args.len() <= 1 || args.contains(&"--help".to_string()) {
        exibir_ajuda();
        return Ok(());
    }

    let arquivo_pr = &args[1];
    
    // Análise do argumento --target
    let target = args.iter()
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

    compilar_arquivo(arquivo_pr, target)
}

fn compilar_arquivo(
    caminho_arquivo: &str,
    target: TargetCompilacao,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Compilando \"{}\" para o alvo: {:?} ===", caminho_arquivo, target);

    let codigo = fs::read_to_string(caminho_arquivo)?;
    let ast = processar_codigo_dinamico(&codigo)?;
    
    let nome_base = Path::new(caminho_arquivo).file_stem().unwrap_or_default().to_str().unwrap_or("saida");

    match target {
        TargetCompilacao::Universal => compilar_universal(&ast, nome_base),
        TargetCompilacao::LlvmIr => compilar_para_llvm_ir(&ast, nome_base),
        TargetCompilacao::CilBytecode => compilar_para_cil_bytecode(&ast, nome_base),
        TargetCompilacao::Console => compilar_para_console(&ast, nome_base),
        TargetCompilacao::Bytecode => compilar_para_bytecode(&ast, nome_base),
    }
}

// (O restante do arquivo main.rs permanece o mesmo, com as funções processar_codigo_dinamico,
// compilar_universal, compilar_para_llvm_ir, etc.)

fn processar_codigo_dinamico(codigo: &str) -> Result<ast::Programa, Box<dyn std::error::Error>> {
    let lex = lexer::Token::lexer(codigo);
    let tokens: Vec<_> = lex.spanned().map(|(tok_res, span)| (span.start, tok_res.unwrap(), span.end)).collect();
    let parser = parser::ArquivoParser::new();
    let mut ast = parser.parse(tokens.iter().cloned()).map_err(|e| Box::new(CompilerError(format!("Erro sintático: {:?}", e))))?;
    crate::interpolacao::walk_programa(&mut ast, |e| {
        *e = interpolacao::planificar_interpolada(e.clone());
    });
    Ok(ast)
}

fn compilar_universal(ast: &ast::Programa, nome_base: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🌍 Iniciando Compilação Universal...");
    compilar_para_llvm_ir(ast, nome_base)?;
    compilar_para_cil_bytecode(ast, nome_base)?;
    compilar_para_console(ast, nome_base)?;
    compilar_para_bytecode(ast, nome_base)?;
    println!("\n🎉 Compilação Universal Concluída!");
    Ok(())
}

fn compilar_para_llvm_ir(programa: &ast::Programa, nome_base: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔧 Gerando LLVM IR...");
    let gerador = codegen::GeradorCodigo::new()?;
    gerador.gerar_llvm_ir(programa, nome_base).map_err(|e| Box::new(CompilerError(e)))?;
    println!("  ✓ {}.ll gerado.", nome_base);
    println!("  Para compilar: clang {0}.ll -o {0}", nome_base);
    println!("🎯 Pipeline LLVM: AST → LLVM IR → Código de Máquina");
    println!("Para executar: ./{}", nome_base);
    Ok(())
}

fn compilar_para_cil_bytecode(ast: &ast::Programa, nome_base: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔧 Gerando CIL Bytecode...");
    let gerador = codegen::GeradorCodigo::new()?;
    gerador.gerar_cil(ast, nome_base).map_err(|e| Box::new(CompilerError(e)))?;
    println!("  ✓ {}.il gerado.", nome_base);
    println!("  Para compilar: ilasm {0}.il /exe /output:{0}.exe", nome_base);
    Ok(())
}

fn compilar_para_console(ast: &ast::Programa, nome_base: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔧 Gerando Projeto de Console .NET...");
    let gerador = codegen::GeradorCodigo::new()?;
    gerador.gerar_console(ast, nome_base).map_err(|e| Box::new(CompilerError(e)))?;
    println!("  ✓ Projeto '{}' gerado.", nome_base);
    println!("  Para executar: cd {} && dotnet run", nome_base);
    Ok(())
}

fn compilar_para_bytecode(ast: &ast::Programa, nome_base: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔧 Gerando Bytecode Customizado...");
    let mut gerador = codegen::GeradorCodigo::new()?;
    gerador.gerar_bytecode(ast, nome_base).map_err(|e| Box::new(CompilerError(e)))?;
    println!("  ✓ {}.pbc gerado.", nome_base);
    println!(" ✓ Executando o bytecode...");
    println!("Você pode executar o bytecode usando o interpretador personalizado.");
    println!("Execute: cargo run --bin interpretador -- ./{}.pbc", nome_base);
    println!("ou use o comando:");
    println!("Para executar: ./interpretador ./{}.pbc", nome_base);
    Ok(())
}