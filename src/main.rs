mod lexer;
mod parser;
use std::io;
mod ast;
mod codegen;

use inkwell::context::Context;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let codigo = r#"
        se x > 10 então
            imprima("Olá mundo")
    "#;

    // Lexer
    let mut lex = lexer::Token::lexer(codigo);
    let tokens: Vec<_> = lex.collect();

    // Parser
    let parser = parser::ComandoParser::new();
    let ast = parser.parse(tokens)?;

    // Codegen
    let context = Context::create();
    let gerador = codegen::GeradorCodigo::new(&context);
    gerador.compilar_comando(&ast);

    // Otimizações LLVM
    // Corrigido: unwrap() foi substituído por um tratamento de erro apropriado.
    // Se a verificação do módulo LLVM falhar, um erro será criado e propagado.
    gerador.module.verify().map_err(|llvm_err_string| {
        io::Error::new(io::ErrorKind::Other, format!("Falha na verificação do módulo LLVM: {}", llvm_err_string.to_string_lossy()))
    })?;
    gerador.module.print_to_file("output.ll")?;

    Ok(())
}
