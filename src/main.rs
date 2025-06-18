// src/main.rs

use std::fs;
use std::path::Path;
use std::fmt;

mod ast;
mod codegen;
mod inferencia_tipos;
mod interpolacao;
mod lexer;
mod module_system;
mod ownership;
mod stdlib;
mod type_checker;

use lalrpop_util::lalrpop_mod;
lalrpop_mod!(pub parser);
use logos::Logos;

//LLVM (Já funcional):
//cargo run -- teste.pr --target=llvm-ir
//clang teste.ll -o teste
//./teste

//CIL Bytecode:
//cargo run -- teste.pr --target=cil-bytecode
//# Se tiver o 'ilasm' (parte do .NET Framework ou Mono)
//ilasm teste.il /exe /output:teste-cil.exe
//# Para executar (no Windows)
//./teste-cil.exe
//# Ou com Mono
//mono teste-cil.exe

//Console .NET:
//cargo run -- teste.pr --target=console
//cd teste # Entra no diretório do projeto gerado
//dotnet run

//Bytecode Customizado:
//cargo run -- teste.pr --target=bytecode
//cat teste.pbc # Para ver o bytecode gerado


// Struct de erro customizada para resolver a ambiguidade.
#[derive(Debug)]
struct CompilerError(String);

// Implementa como o erro deve ser exibido.
impl fmt::Display for CompilerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// Implementa a trait `Error`, tornando-o um tipo de erro válido.
impl std::error::Error for CompilerError {}


#[derive(Debug, Clone)]
enum TargetCompilacao {
    Universal,
    LlvmIr,
    CilBytecode,
    Console,
    Bytecode,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Uso: {} <arquivo.pr> [--target=TARGET]", args[0]);
        eprintln!("\nTargets disponíveis:");
        eprintln!("  --target=llvm-ir       (Gera código LLVM IR)");
        eprintln!("  --target=cil-bytecode  (Gera código CIL .NET)");
        eprintln!("  --target=console       (Gera um projeto de Console .NET)");
        eprintln!("  --target=bytecode      (Gera bytecode customizado)");
        eprintln!("  --target=universal     (Gera todos os alvos)");
        std::process::exit(1);
    }

    let target = match args.get(2).map(|s| s.as_str()) {
        Some("--target=universal") => TargetCompilacao::Universal,
        Some("--target=llvm-ir") => TargetCompilacao::LlvmIr,
        Some("--target=cil-bytecode") => TargetCompilacao::CilBytecode,
        Some("--target=console") => TargetCompilacao::Console,
        Some("--target=bytecode") => TargetCompilacao::Bytecode,
        _ => TargetCompilacao::Universal,
    };

    compilar_arquivo(&args[1], target)
}

fn compilar_arquivo(
    caminho_arquivo: &str,
    target: TargetCompilacao,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Compilando {} para o alvo: {:?} ===", caminho_arquivo, target);

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

/// Processa o código, incluindo a planificação de strings interpoladas.
fn processar_codigo_dinamico(codigo: &str) -> Result<ast::Programa, Box<dyn std::error::Error>> {
    let lex = lexer::Token::lexer(codigo);
    let tokens: Vec<_> = lex.spanned().map(|(tok_res, span)| (span.start, tok_res.unwrap(), span.end)).collect();
    let parser = parser::ArquivoParser::new();
    let mut ast = parser.parse(tokens.iter().cloned()).map_err(|e| format!("Erro sintático: {:?}", e))?;
    crate::interpolacao::walk_programa(&mut ast, |e| {
        *e = interpolacao::planificar_interpolada(e.clone());
    });
    Ok(ast)
}

// --- Funções de Compilação para cada Alvo ---

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
    // ✅ CORREÇÃO: Conversão explícita para Box<dyn Error> usando Box::new.
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
    // ✅ CORREÇÃO: Conversão explícita para Box<dyn Error> usando Box::new.
    gerador.gerar_cil(ast, nome_base).map_err(|e| Box::new(CompilerError(e)))?;
    println!("  ✓ {}.il gerado.", nome_base);
    println!("  Para compilar: ilasm {0}.il /exe /output:{0}.exe", nome_base);
    Ok(())
}

fn compilar_para_console(ast: &ast::Programa, nome_base: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔧 Gerando Projeto de Console .NET...");
    let gerador = codegen::GeradorCodigo::new()?;
    // ✅ CORREÇÃO: Conversão explícita para Box<dyn Error> usando Box::new.
    gerador.gerar_console(ast, nome_base).map_err(|e| Box::new(CompilerError(e)))?;
    println!("  ✓ Projeto '{}' gerado.", nome_base);
    println!("  Para executar: cd {} && dotnet run", nome_base);
    Ok(())
}

fn compilar_para_bytecode(ast: &ast::Programa, nome_base: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔧 Gerando Bytecode Customizado...");
    let gerador = codegen::GeradorCodigo::new()?;
    // ✅ CORREÇÃO: Conversão explícita para Box<dyn Error> usando Box::new.
    gerador.gerar_bytecode(ast, nome_base).map_err(|e| Box::new(CompilerError(e)))?;
    println!("  ✓ {}.pbc gerado.", nome_base);
    Ok(())
}