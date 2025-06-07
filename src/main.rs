mod lexer;
mod ast;
mod codegen;
// ...
lalrpop_mod!(parser);

use std::fs;
use logos::Logos;
use inkwell::context::Context;
use lalrpop_util::lalrpop_mod;


fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Uso: {} <arquivo.pr>", args[0]);
        std::process::exit(1);
    }

    let caminho_arquivo = &args[1];
    let codigo = fs::read_to_string(caminho_arquivo)?;

    // Análise Léxica
    let lex = lexer::Token::lexer(&codigo);
    let tokens: Vec<_> = lex.spanned()
        .filter_map(|(tok_res, span)| tok_res.ok().map(|tok| (span.start, tok, span.end)))
        .collect();

    // Análise Sintática
    let parser = parser::ComandoParser::new();
    let ast = parser.parse(tokens.iter().cloned())
        .map_err(|e| format!("Erro sintático: {:?}", e))?;

    // Geração de Código
    let context = Context::create();
    let gerador = codegen::GeradorCodigo::new(&context);
    
    let i32_type = context.i32_type();
    let function = context.i32_type().fn_type(&[], false);
    let function = gerador.module.add_function("main", function, None);
    let basic_block = context.append_basic_block(function, "entry");
    gerador.builder.position_at_end(basic_block);

    gerador.compilar_comando(&ast)?;
    let _ = gerador.builder.build_return(Some(&i32_type.const_int(0, false)));

    gerador.module.verify()?;
    let output_path = format!("{}.ll", caminho_arquivo.trim_end_matches(".pr"));
    gerador.module.print_to_file(&output_path)?;

    println!("Compilação concluída! Arquivo gerado: {}", output_path);
    
    Ok(())
}