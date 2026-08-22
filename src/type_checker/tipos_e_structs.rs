use crate::ast;
use crate::ast::*;
use crate::error::{ErroCompilador, TipoErro};
use crate::library_loader::{self, LibSimbolo};
use std::collections::{HashMap, HashSet};

// Helper function to convert string to ast::Tipo

pub fn string_para_tipo(s: &str) -> ast::Tipo {
    match s {
        "inteiro" => ast::Tipo::Inteiro,
        "texto" => ast::Tipo::Texto,
        "booleano" => ast::Tipo::Booleano,
        "flutuante" => ast::Tipo::Flutuante,
        "duplo" => ast::Tipo::Duplo,
        "decimal" => ast::Tipo::Decimal,
        "vazio" => ast::Tipo::Vazio,
        "objeto" => ast::Tipo::Objeto,
        _ => ast::Tipo::Classe(s.to_string()),
    }
}

#[derive(Clone)]
pub struct VerificadorTipos<'a> {
    pub(crate) usings: Vec<String>,
    pub(crate) simbolos_namespaces: HashMap<String, &'a Declaracao>,
    pub classes: HashMap<String, &'a DeclaracaoClasse>,
    pub interfaces: HashMap<String, &'a ast::DeclaracaoInterface>,
    pub enums: HashMap<String, &'a DeclaracaoEnum>,
    pub resolved_classes: HashMap<String, ResolvedClassInfo<'a>>,
    pub(crate) erros: Vec<ErroCompilador>,
    // Biblioteca externa carregada (metadados .pbl sem conversão para AST)
    pub biblioteca_externa: Option<library_loader::Biblioteca>,
    // Storage for library-loaded AST nodes (mantido para compatibilidade com tipo 'objeto')
    pub(crate) loaded_lib_declarations: Vec<Declaracao>,
    pub generic_scope: Vec<std::collections::HashSet<String>>,
    pub(crate) stdlib_namespaces: std::collections::HashSet<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedClassInfo<'a> {
    pub name: String,
    pub parent_name: Option<String>,
    pub properties: Vec<&'a ast::PropriedadeClasse>,
    pub fields: Vec<&'a ast::CampoClasse>,
    pub methods: HashMap<String, &'a ast::MetodoClasse>,
    pub eh_estatica: bool,
    pub eh_abstrata: bool,
    pub interfaces: Vec<String>,
}

