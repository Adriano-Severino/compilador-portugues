mod lexer;
mod ast;
mod codegen;
mod type_checker;
mod ownership;
mod module_system;
mod stdlib;
mod interpolacao; // NOVO
use lalrpop_util::lalrpop_mod;

lalrpop_mod!(pub parser);

use std::fs;
use logos::Logos;
use inkwell::context::Context;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Uso: {} <arquivo.pr>", args[0]);
        std::process::exit(1);
    }

    compilar_arquivo(&args[1])
}

fn compilar_arquivo(caminho_arquivo: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Compilando {} ===", caminho_arquivo);

    let codigo = fs::read_to_string(caminho_arquivo)?;

    // 1. Análise Léxica
    println!("1. Análise Léxica...");
    let lex = lexer::Token::lexer(&codigo);
    let tokens: Vec<_> = lex.spanned()
        .filter_map(|(tok_res, span)| {
            match tok_res {
                Ok(tok) => Some((span.start, tok, span.end)),
                Err(e) => {
                    eprintln!("Erro léxico na posição {}: {:?}", span.start, e);
                    None
                }
            }
        })
        .collect();

    if tokens.is_empty() {
        return Err("Nenhum token válido encontrado".into());
    }

    println!("   {} tokens processados", tokens.len());

    // 2. Análise Sintática
    println!("2. Análise Sintática...");
    let parser = parser::ArquivoParser::new();
    let mut ast = parser.parse(tokens.iter().cloned())
        .map_err(|e| format!("Erro sintático: {:?}", e))?;

    // 3. Adicionar biblioteca padrão
    println!("3. Carregando biblioteca padrão...");
    let mut stdlib = stdlib::criar_biblioteca_padrao();
    ast.declaracoes.append(&mut stdlib);

    // 4. Verificação de Tipos
    println!("4. Verificação de tipos...");
    let mut verificador_tipos = type_checker::VerificadorTipos::new();
    match verificador_tipos.verificar_programa(&ast) {
        Ok(()) => println!("   ✓ Tipos verificados com sucesso"),
        Err(erros) => {
            eprintln!("   ✗ Erros de tipo encontrados:");
            for erro in &erros {
                eprintln!("     - {}", erro);
            }
            // Continuar mesmo com erros de tipo para demonstração
        }
    }

    // 5. Análise de Ownership
    println!("5. Análise de ownership...");
    let mut analisador_ownership = ownership::AnalisadorOwnership::new();
    match analisador_ownership.analisar_programa(&ast) {
        Ok(warnings) => {
            println!("   ✓ Ownership verificado com sucesso");
            if !warnings.is_empty() {
                println!("   Avisos:");
                for warning in &warnings {
                    println!("     - {}", warning);
                }
            }
        },
        Err(erros) => {
            eprintln!("   ✗ Erros de ownership encontrados:");
            for erro in &erros {
                eprintln!("     - {}", erro);
            }
            // Continuar mesmo com erros para demonstração
        }
    }
    
    // 6. Geração de Código LLVM
    println!("6. Geração de código...");
    let context = Context::create();
    let gerador = codegen::GeradorCodigo::new(&context);
    
    let i32_type = context.i32_type();
    let function_type = i32_type.fn_type(&[], false);
    let function = gerador.module.add_function("main", function_type, None);
    let basic_block = context.append_basic_block(function, "entry");
    gerador.builder.position_at_end(basic_block);

    // Compilar o programa
    gerador.compilar_programa(&ast)?;
    let _ = gerador.builder.build_return(Some(&i32_type.const_int(0, false)));

    // 7. Verificação e Saída
    println!("7. Verificação final...");
    gerador.module.verify()
        .map_err(|e| format!("Erro na verificação do módulo LLVM: {}", e))?;

    let output_path = format!("{}.ll", caminho_arquivo.trim_end_matches(".pr"));
    gerador.module.print_to_file(&output_path)
        .map_err(|e| format!("Erro ao escrever arquivo: {}", e))?;

    println!("✓ Compilação concluída! Arquivo gerado: {}", output_path);
    println!("\nPara executar:");
    println!("  clang {} -o {}", output_path, caminho_arquivo.trim_end_matches(".pr"));
    println!("  ./{}", caminho_arquivo.trim_end_matches(".pr"));

    Ok(())
}