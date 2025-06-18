// src/main.rs

use std::fs;
// Os imports de `Path` e `Command` não são mais necessários aqui.
// use std::path::Path;
// use std::process::Command;

mod ast;
mod codegen;
mod inferencia_tipos;
mod interpolacao;
mod lexer;
mod module_system;
mod ownership;
// mod runtime; // Comentado se não estiver em uso
mod stdlib;
mod type_checker;

use lalrpop_util::lalrpop_mod;
lalrpop_mod!(pub parser);
use logos::Logos;

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
    println!("=== Compilando {} ===", caminho_arquivo);
    println!("🎯 Target: {:?}", target);

    let codigo = fs::read_to_string(caminho_arquivo)?;
    let ast = processar_codigo_dinamico(&codigo)?;

    match target {
        TargetCompilacao::Universal => compilar_universal(&ast, caminho_arquivo),
        TargetCompilacao::LlvmIr => compilar_para_llvm_ir(&ast, caminho_arquivo),
        TargetCompilacao::CilBytecode => compilar_para_cil_bytecode(&ast, caminho_arquivo),
        TargetCompilacao::Console => compilar_para_console(&ast, caminho_arquivo),
        TargetCompilacao::Bytecode => compilar_para_bytecode(&ast),
    }
}

// ✅ CORREÇÃO: Reintroduzida a etapa de planificação da interpolação.
fn processar_codigo_dinamico(codigo: &str) -> Result<ast::Programa, Box<dyn std::error::Error>> {
    let lex = lexer::Token::lexer(&codigo);
    let tokens: Vec<_> = lex
        .spanned()
        .map(|(tok_res, span)| (span.start, tok_res.unwrap(), span.end))
        .collect();

    let parser = parser::ArquivoParser::new();
    let mut ast = parser
        .parse(tokens.iter().cloned())
        .map_err(|e| format!("Erro sintático: {:?}", e))?;

    // ✅ ESTA É A LINHA CRUCIAL:
    //    Ela percorre a AST e converte todas as `Expressao::StringInterpolada`
    //    em uma árvore de `Expressao::Aritmetica` com o operador de Soma.
    crate::interpolacao::walk_programa(&mut ast, |e| {
        *e = interpolacao::planificar_interpolada(e.clone());
    });

    Ok(ast)
}

fn compilar_universal(
    ast: &ast::Programa,
    caminho: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🌍 Compilação Universal - Gerando LLVM e CIL...");
    let nome_base = caminho.trim_end_matches(".pr");

    println!("1. Gerando LLVM IR (Dinâmico)...");
    let gerador_llvm = codegen::GeradorCodigo::new_llvm()?;
    gerador_llvm.gerar_llvm_ir_dinamico(ast, nome_base)?;
    println!("  ✓ {}.ll (LLVM IR)", nome_base);

    println!("2. Gerando bytecode específico para CIL...");
    let gerador_cil = codegen::GeradorCodigo::new_cil_bytecode()?;
    gerador_cil.gerar_programa(ast)?;
    let bytecode_cil = gerador_cil.obter_bytecode_cil();
    println!("  ✓ Bytecode para CIL gerado na memória");

    println!("3. Gerando CIL a partir do bytecode específico...");
    gerador_cil.gerar_cil_do_bytecode_cil(&bytecode_cil, nome_base)?;
    println!("  ✓ {}.il (CIL)", nome_base);

    println!("\n🎉 Compilação Universal Concluída!");
    println!("📦 Formatos gerados:");
    println!("  • {}.ll - LLVM IR (compile com: clang {}.ll -o {})", nome_base, nome_base, nome_base);
    println!("  • {}.il - CIL (compile com: ilasm {}.il /exe)", nome_base, nome_base);

    Ok(())
}

fn compilar_para_llvm_ir(
    programa: &ast::Programa,
    caminho: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔧 Gerando LLVM IR dinamicamente a partir da AST...");
    let nome_base = caminho.trim_end_matches(".pr");

    let gerador = codegen::GeradorCodigo::new_llvm()?;
    gerador
        .gerar_llvm_ir_dinamico(programa, nome_base)
        .map_err(|e| e.to_string())?;

    println!("  ✓ {}.ll gerado com sucesso.", nome_base);
    println!("🎯 Pipeline LLVM: AST → LLVM IR → Código de Máquina");
    println!("Para compilar, execute: clang {}.ll -o {}", nome_base, nome_base);
    println!("Para executar: ./{}", nome_base);
    Ok(())
}

// Funções stub com avisos de "unused variable" corrigidos
fn compilar_para_cil_bytecode(_ast: &ast::Programa, _caminho: &str) -> Result<(), Box<dyn std::error::Error>> { Ok(()) }
fn compilar_para_console(_ast: &ast::Programa, _caminho: &str) -> Result<(), Box<dyn std::error::Error>> { Ok(()) }
fn compilar_para_bytecode(_ast: &ast::Programa) -> Result<(), Box<dyn std::error::Error>> { Ok(()) }
fn criar_projeto_console(_projeto: &str, _nome_projeto: &str) -> Result<(), Box<dyn std::error::Error>> { Ok(()) }
fn verificar_compatibilidade_dinamica(_ast: &ast::Programa) -> Result<(), Box<dyn std::error::Error>> { Ok(()) }