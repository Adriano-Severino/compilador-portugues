//! Compilador de Linguagem de Programação em Português
//! 
//! Este projeto implementa um compilador completo para uma linguagem de programação
//! totalmente em português, com suporte a:
//! - Orientação a objetos
//! - Herança e polimorfismo  
//! - Verificação de tipos
//! - Análise de ownership
//! - Geração de código LLVM

// ✅ EXISTENTE: Módulos básicos
pub mod lexer;
pub mod ast;
pub mod codegen;
pub mod type_checker;
pub mod ownership;
pub mod inferencia_tipos;
pub mod module_system;
pub mod stdlib;
pub mod interpolacao;

// ✅ NOVO: Re-exportar tipos principais para facilitar uso como biblioteca
pub use ast::{Programa, Declaracao, DeclaracaoClasse, MetodoClasse, Comando, Expressao, Tipo};
pub use lexer::Token;
pub use type_checker::VerificadorTipos;
pub use ownership::AnalisadorOwnership;
pub use inferencia_tipos::InferenciaTipos; // ✅ NOVO
pub use codegen::GeradorCodigo;

// ✅ NOVO: Parser usando LALRPOP
use lalrpop_util::lalrpop_mod;
lalrpop_mod!(pub parser);

/// ✅ NOVO: Estrutura principal do compilador com suporte à herança
pub struct CompiladorPortugues {
    pub verificador_tipos: VerificadorTipos,
    pub analisador_ownership: AnalisadorOwnership,
    pub inferencia_tipos: InferenciaTipos, // ✅ NOVO
}

impl CompiladorPortugues {
    /// Cria uma nova instância do compilador
    pub fn new() -> Self {
        Self {
            verificador_tipos: VerificadorTipos::new(),
            analisador_ownership: AnalisadorOwnership::new(),
            inferencia_tipos: InferenciaTipos::new(), // ✅ NOVO
        }
    }

    /// ✅ NOVO: Compila código com suporte completo à herança
    pub fn compilar_codigo(&mut self, codigo: &str) -> Result<Programa, String> {
        // 1. Tokenização
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

        // 2. Parsing
        let parser = parser::ArquivoParser::new();
        let mut ast = parser.parse(tokens.iter().cloned())
            .map_err(|e| format!("Erro sintático: {:?}", e))?;

        // 3. Interpolação de strings
        interpolacao::walk_programa(&mut ast, |e| {
            *e = interpolacao::planificar_interpolada(e.clone());
        });

        // ✅ NOVO: 4. Registrar classes para herança em todos os analisadores
        for declaracao in &ast.declaracoes {
            if let Declaracao::DeclaracaoClasse(classe) = declaracao {
                // Registrar para inferência de tipos
                self.inferencia_tipos.registrar_classe(classe.clone());
                
                // Registrar para análise de ownership
                self.analisador_ownership.registrar_classe(classe.clone());
            }
        }

        // 5. Verificação de tipos com herança
        self.verificador_tipos.verificar_programa(&ast)
            .map_err(|erros| format!("Erros de tipo: {}", erros.join("; ")))?;

        // 6. Análise de ownership com polimorfismo
        match self.analisador_ownership.analisar_programa(&ast) {
            Ok(_warnings) => {
                // Warnings não impedem compilação
            }
            Err(_erros) => {
                // Erros de ownership não impedem compilação por enquanto
            }
        }

        Ok(ast)
    }

    /// ✅ NOVO: Verifica se programa usa herança
    pub fn programa_usa_heranca(&self, programa: &Programa) -> bool {
        for declaracao in &programa.declaracoes {
            if let Declaracao::DeclaracaoClasse(classe) = declaracao {
                if classe.classe_pai.is_some() {
                    return true;
                }
                
                for metodo in &classe.metodos {
                    if metodo.eh_virtual || metodo.eh_override {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// ✅ NOVO: Obtém estatísticas de herança
    pub fn estatisticas_heranca(&self, programa: &Programa) -> EstatisticasHeranca {
        let mut stats = EstatisticasHeranca::default();
        
        for declaracao in &programa.declaracoes {
            if let Declaracao::DeclaracaoClasse(classe) = declaracao {
                stats.total_classes += 1;
                
                if classe.classe_pai.is_some() {
                    stats.classes_com_heranca += 1;
                }
                
                for metodo in &classe.metodos {
                    stats.total_metodos += 1;
                    
                    if metodo.eh_virtual {
                        stats.metodos_redefinaveis += 1;
                    }
                    
                    if metodo.eh_override {
                        stats.metodos_sobrescritos += 1;
                    }
                }
            }
        }
        
        stats
    }
}

/// ✅ NOVO: Estrutura para estatísticas de herança
#[derive(Debug, Default, Clone)]
pub struct EstatisticasHeranca {
    pub total_classes: usize,
    pub classes_com_heranca: usize,
    pub total_metodos: usize,
    pub metodos_redefinaveis: usize,
    pub metodos_sobrescritos: usize,
}

impl EstatisticasHeranca {
    /// Verifica se há uso de herança
    pub fn tem_heranca(&self) -> bool {
        self.classes_com_heranca > 0 || self.metodos_redefinaveis > 0 || self.metodos_sobrescritos > 0
    }
    
    /// Obtém porcentagem de classes que usam herança
    pub fn porcentagem_heranca(&self) -> f64 {
        if self.total_classes == 0 {
            0.0
        } else {
            (self.classes_com_heranca as f64 / self.total_classes as f64) * 100.0
        }
    }
}

// ✅ EXISTENTE: Função utilitária mantida
pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

// ✅ NOVO: Função utilitária para verificar sintaxe rapidamente
pub fn verificar_sintaxe(codigo: &str) -> Result<(), String> {
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

    let parser = parser::ArquivoParser::new();
    parser.parse(tokens.iter().cloned())
        .map_err(|e| format!("Erro sintático: {:?}", e))?;

    Ok(())
}

// ✅ EXISTENTE: Testes mantidos
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }

    // ✅ NOVO: Teste de herança básica
    #[test]
    fn test_heranca_basica() {
        let codigo = r#"
            publico classe Animal {
                publico texto Nome;
                
                publico redefinível vazio som() {
                    imprima("Som genérico");
                }
            }
            
            publico classe Cachorro : Animal {
                publico sobrescreve vazio som() {
                    imprima("Au au!");
                }
            }
        "#;

        let resultado = verificar_sintaxe(codigo);
        assert!(resultado.is_ok(), "Sintaxe de herança deve ser válida");
    }

    // ✅ NOVO: Teste do compilador completo
    #[test]
    fn test_compilador_com_heranca() {
        let mut compilador = CompiladorPortugues::new();
        
        let codigo = r#"
            publico classe Veiculo {
                publico texto Marca;
                publico redefinível vazio acelerar() {
                    imprima("Acelerando...");
                }
            }
            
            publico classe Carro : Veiculo {
                publico sobrescreve vazio acelerar() {
                    imprima("Carro acelerando!");
                }
            }
        "#;

        let resultado = compilador.compilar_codigo(codigo);
        assert!(resultado.is_ok(), "Compilação com herança deve funcionar");
        
        let programa = resultado.unwrap();
        assert!(compilador.programa_usa_heranca(&programa), "Programa deve usar herança");
        
        let stats = compilador.estatisticas_heranca(&programa);
        assert_eq!(stats.total_classes, 2);
        assert_eq!(stats.classes_com_heranca, 1);
        assert!(stats.tem_heranca());
    }
}