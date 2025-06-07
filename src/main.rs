mod ast;
mod codegen;
mod lexer;

use inkwell::context::Context;
use lalrpop_util::lalrpop_mod;
use logos::Logos; // Importar trait Logos
use std::fs; // Importar macro lalrpop_mod

lalrpop_mod!(parser); // Mover após os imports

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Verifica argumentos
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Uso: {} <arquivo.pr>", args[0]);
        std::process::exit(1);
    }

    // Lê arquivo de entrada
    let caminho_arquivo = &args[1];
    let codigo =
        fs::read_to_string(caminho_arquivo).map_err(|e| format!("Erro ao ler arquivo: {}", e))?;

    // Fase 1: Análise Léxica (CORRIGIDO)
    let mut lex = lexer::Token::lex(&codigo).spanned();
    let tokens: Vec<_> = lex
        .filter_map(|(tok, span)| match tok {
            Ok(token) => {
                if let lexer::Token::Erro = token {
                    eprintln!("Erro léxico na posição {}:{}", span.start, span.end);
                    None
                } else {
                    Some((span.start, token, span.end))
                }
            }
            Err(_) => {
                eprintln!("Erro léxico na posição {}:{}", span.start, span.end);
                None
            }
        })
        .collect();

    // Fase 2: Análise Sintática
    let parser = parser::ComandoParser::new();
    let ast = parser
        .parse(tokens)
        .map_err(|e| format!("Erro sintático: {:?}", e))?;

    // Fase 3: Geração de Código
    let context = Context::create();
    let gerador = codegen::GeradorCodigo::new(&context);

    // Cria função principal
    let i32_type = context.i32_type();
    let fn_type = i32_type.fn_type(&[], false);
    let function = gerador.module.add_function("main", fn_type, None);
    let basic_block = context.append_basic_block(function, "entry");
    gerador.builder.position_at_end(basic_block);

    gerador
        .compilar_comando(&ast)
        .map_err(|e| format!("Erro na geração de código: {}", e))?;

    // Retorno zero (padrão para executáveis)
    gerador
        .builder
        .build_return(Some(&i32_type.const_int(0, false)));

    // Validação e saída
    gerador
        .module
        .verify()
        .map_err(|e| format!("Erro de verificação LLVM: {}", e.to_string_lossy()))?;

    // Gera arquivo .ll
    let output_path = format!("{}.ll", caminho_arquivo.trim_end_matches(".pr"));
    gerador.module.print_to_file(&output_path)?;

    println!("Compilação concluída! Arquivo gerado: {}", output_path);

    Ok(())
}
