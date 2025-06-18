// src/compiler/namespace_resolver.rs
use crate::ast::*;
use crate::compiler::pipeline::*;

pub struct NamespaceResolver;

impl NamespaceResolver {
    pub fn new() -> Self {
        Self
    }
}

impl CompilerPass for NamespaceResolver {
    fn name(&self) -> &str {
        "Namespace Resolver"
    }
    
    fn run(&mut self, programa: &mut Programa, context: &mut CompilationContext) -> Result<(), String> {
        // Achatar namespaces para o nível raiz
        let mut declaracoes_achatadas = Vec::new();
        
        // Processar declarações existentes no nível raiz
        declaracoes_achatadas.extend(programa.declaracoes.clone());
        
        // Processar declarações dentro de namespaces
        for namespace in &programa.namespaces {
            println!("  Processando namespace: {}", namespace.nome);
            
            for declaracao in &namespace.declaracoes {
                // Adicionar informação de namespace às declarações
                let mut declaracao_qualificada = declaracao.clone();
                self.qualificar_declaracao(&mut declaracao_qualificada, &namespace.nome);
                declaracoes_achatadas.push(declaracao_qualificada);
            }
        }
        
        // Atualizar programa com declarações achatadas
        programa.declaracoes = declaracoes_achatadas;
        
        // Registrar símbolos no contexto
        for declaracao in &programa.declaracoes {
            self.registrar_simbolo(declaracao, context)?;
        }
        
        Ok(())
    }
}

impl NamespaceResolver {
    fn qualificar_declaracao(&self, declaracao: &mut Declaracao, namespace: &str) {
        match declaracao {
            Declaracao::DeclaracaoClasse(classe) => {
                if !classe.nome.contains("::") {
                    classe.nome = format!("{}::{}", namespace, classe.nome);
                }
            },
            Declaracao::DeclaracaoFuncao(funcao) => {
                if !funcao.nome.contains("::") && funcao.nome != "principal" {
                    funcao.nome = format!("{}::{}", namespace, funcao.nome);
                }
            },
            _ => {}
        }
    }
    
    fn registrar_simbolo(&self, declaracao: &Declaracao, context: &mut CompilationContext) -> Result<(), String> {
        match declaracao {
            Declaracao::DeclaracaoClasse(classe) => {
                context.symbols.register_class(&classe.nome, classe.clone())?;
            },
            Declaracao::DeclaracaoFuncao(funcao) => {
                context.symbols.register_function(&funcao.nome, funcao.clone())?;
            },
            _ => {}
        }
        Ok(())
    }
}