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
pub mod lexer;
pub mod ast;
pub mod runtime;
pub mod codegen;
pub mod type_checker;
pub mod ownership;
pub mod inferencia_tipos;
pub mod module_system;
pub mod stdlib;
pub mod interpolacao;

// Parser usando LALRPOP
use lalrpop_util::lalrpop_mod;
lalrpop_mod!(pub parser);

// Re-exportações básicas (remover as problemáticas)
pub use ast::{Programa, Declaracao, DeclaracaoClasse, MetodoClasse, Comando, Expressao, Tipo};
pub use lexer::Token;
pub use type_checker::VerificadorTipos;
pub use ownership::AnalisadorOwnership;
pub use inferencia_tipos::InferenciaTipos;

// Re-exportações do codegen (funcionais)
pub use codegen::{GeradorCodigo, BackendType};
pub use runtime::{
    executar_programa_otimizado,
    executar_programa_debug,
    interpretar_com_classes,
    validar_programa
};

// Estrutura principal do compilador
pub struct CompiladorPortugues {
    pub verificador_tipos: VerificadorTipos,
    pub analisador_ownership: AnalisadorOwnership,
    pub inferencia_tipos: InferenciaTipos,
}

impl CompiladorPortugues {
    pub fn new() -> Self {
        Self {
            verificador_tipos: VerificadorTipos::new(),
            analisador_ownership: AnalisadorOwnership::new(),
            inferencia_tipos: InferenciaTipos::new(),
        }
    }

    pub fn compilar_codigo(&mut self, codigo: &str) -> Result<Programa, String> {
        // Tokenização
        use logos::Logos;
        let lex = Token::lexer(codigo);
        let tokens: Vec<_> = lex.spanned()
            .filter_map(|(tok_res, span)| {
                match tok_res {
                    Ok(tok) => Some((span.start, tok, span.end)),
                    Err(_) => None,
                }
            })
            .collect();

        if tokens.is_empty() {
            return Err("Nenhum token válido encontrado".to_string());
        }

        // Parsing
        let parser = parser::ArquivoParser::new();
        let mut ast = parser.parse(tokens.iter().cloned())
            .map_err(|e| format!("Erro sintático: {:?}", e))?;

        // Interpolação de strings
        interpolacao::walk_programa(&mut ast, |e| {
            *e = interpolacao::planificar_interpolada(e.clone());
        });

        Ok(ast)
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