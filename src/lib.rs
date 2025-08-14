// src/lib.rs

//! Compilador de Linguagem de Programação em Português
//!
//! Este projeto implementa um compilador completo para uma linguagem de programação
//! totalmente em português, com suporte a:
//! - Orientação a objetos
//! - Herança e polimorfismo
//! - Verificação de tipos
//! - Análise de ownership
//! - Geração de código LLVM

// Declarar módulos principais
pub mod ast;
pub mod lexer;
// pub mod runtime; // Comentado se não estiver em uso
pub mod codegen;
pub mod inferencia_tipos;
pub mod interpolacao;
pub mod jit;
pub mod module_system;
pub mod ownership;
pub mod stdlib;
pub mod type_checker;

// Parser usando LALRPOP
use lalrpop_util::lalrpop_mod;
lalrpop_mod!(pub parser);

// Re-exportações básicas
pub use ast::{Comando, Declaracao, DeclaracaoClasse, Expressao, MetodoClasse, Programa, Tipo};
pub use inferencia_tipos::InferenciaTipos;
pub use lexer::Token;
pub use ownership::AnalisadorOwnership;
pub use type_checker::VerificadorTipos;

// ✅ CORREÇÃO: Removida a importação do `BackendType`, que não é mais público.
pub use codegen::GeradorCodigo;
// pub use runtime::{
//     executar_programa_otimizado,
//     executar_programa_debug,
//     interpretar_com_classes,
//     validar_programa
// };

// Estrutura principal do compilador
pub struct CompiladorPortugues<'a> {
    pub verificador_tipos: VerificadorTipos<'a>,
    pub analisador_ownership: AnalisadorOwnership,
    pub inferencia_tipos: InferenciaTipos,
}

impl<'a> CompiladorPortugues<'a> {
    pub fn new() -> Self {
        Self {
            verificador_tipos: VerificadorTipos::new(),
            analisador_ownership: AnalisadorOwnership::new(),
            inferencia_tipos: InferenciaTipos::new(),
        }
    }

    pub fn compilar_codigo(&mut self, codigo: &str) -> Result<Programa, String> {
        // Precisamos de uma String mutável para possivelmente anexar chaves ausentes
        let mut codigo_fonte = codigo.to_string();
        let mut tentou_recuperar = false;
        loop {
            // Tokenização
            use logos::Logos;
            let lex = Token::lexer(&codigo_fonte);
            let tokens: Vec<_> = lex
                .spanned()
                .filter_map(|(tok_res, span)| match tok_res {
                    Ok(tok) => Some((span.start, tok, span.end)),
                    Err(_) => None,
                })
                .collect();
            if tokens.is_empty() {
                return Err("Nenhum token válido encontrado".to_string());
            }

            // Parsing
            let parser = parser::ArquivoParser::new();
            match parser.parse(tokens.iter().cloned()) {
                Ok(mut ast) => {
                    // Interpolação de strings
                    interpolacao::walk_programa(&mut ast, |e| {
                        *e = interpolacao::planificar_interpolada(e.clone());
                    });
                    return Ok(ast);
                }
                Err(err) => {
                    let err_msg = format!("{:?}", err);
                    // Se chegamos ao final do arquivo esperando '}' tentamos auto-fechar uma única vez
                    if !tentou_recuperar && err_msg.contains("UnrecognizedEof") {
                        let abre = codigo_fonte.matches('{').count();
                        let fecha = codigo_fonte.matches('}').count();
                        if abre > fecha {
                            let faltando = abre - fecha;
                            codigo_fonte.push_str(&"}".repeat(faltando));
                            tentou_recuperar = true;
                            continue; // tenta novamente
                        }
                    }
                    return Err(format!("Erro sintático: {}", err_msg));
                }
            }
        }
    }
}

// Função utilitária mantida
pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
