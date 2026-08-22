pub mod util;
pub mod declaracoes;
pub mod comandos;
pub mod expressoes;

use crate::ast;
use std::collections::{HashMap, HashSet};
use crate::library_loader::LibSimbolo;

/// O gerador de código para o alvo Bytecode.
pub struct BytecodeGenerator<'a> {
    programa: &'a ast::Programa,
    type_checker: &'a crate::type_checker::VerificadorTipos<'a>,
    namespace_path: String,
    bytecode_instructions: Vec<String>,
    props_por_classe: HashMap<String, Vec<String>>,
    construtor_params_por_classe: HashMap<String, Vec<String>>,
    current_class_name: Option<String>,
    // Parâmetros locais do método/construtor atual (para desambiguar nome igual a propriedade)
    current_params: Option<HashSet<String>>,
}

impl<'a> BytecodeGenerator<'a> {
    pub fn new(
        programa: &'a ast::Programa,
        type_checker: &'a crate::type_checker::VerificadorTipos,
    ) -> Self {
        Self {
            programa,
            type_checker,
            namespace_path: String::new(),
            bytecode_instructions: Vec::new(),
            props_por_classe: HashMap::new(),
            construtor_params_por_classe: HashMap::new(),
            current_class_name: None,
            current_params: None,
        }
    }

    pub fn generate(&mut self) -> Vec<String> {
        // Itera sobre as declarações no nível raiz do programa
        for declaracao in &self.programa.declaracoes {
            self.generate_declaracao(declaracao);
        }

        // Também processa namespaces de primeiro nível
        for namespace in &self.programa.namespaces {
            // Cria gerador dedicado com o caminho do namespace
            let mut sub = BytecodeGenerator {
                programa: &ast::Programa {
                    usings: vec![],
                    namespaces: vec![],
                    declaracoes: namespace.declaracoes.clone(),
                },
                type_checker: self.type_checker,
                namespace_path: namespace.nome.clone(),
                bytecode_instructions: Vec::new(),
                props_por_classe: self.props_por_classe.clone(),
                construtor_params_por_classe: self.construtor_params_por_classe.clone(),
                current_class_name: None,
                current_params: None,
            };
            self.bytecode_instructions.extend(sub.generate());
        }

        std::mem::take(&mut self.bytecode_instructions)
    }

    pub fn generate_for_library(&mut self) -> Vec<String> {
        for declaracao in &self.programa.declaracoes {
            self.generate_declaracao(declaracao);
        }

        for namespace in &self.programa.namespaces {
            let mut sub = BytecodeGenerator {
                programa: &ast::Programa {
                    usings: vec![],
                    namespaces: vec![],
                    declaracoes: namespace.declaracoes.clone(),
                },
                type_checker: self.type_checker,
                namespace_path: namespace.nome.clone(),
                bytecode_instructions: Vec::new(),
                props_por_classe: self.props_por_classe.clone(),
                construtor_params_por_classe: self.construtor_params_por_classe.clone(),
                current_class_name: None,
                current_params: None,
            };
            self.bytecode_instructions
                .extend(sub.generate_for_library());
        }

        std::mem::take(&mut self.bytecode_instructions)
    }

    // Altera a assinatura para `&mut self` e remove o retorno Vec<String>
}
