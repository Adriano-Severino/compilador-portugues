// src/compiler/symbol_table.rs
use crate::ast::*;
use std::collections::HashMap;

#[derive(Default)]
pub struct SymbolTable {
    classes: HashMap<String, DeclaracaoClasse>,
    functions: HashMap<String, DeclaracaoFuncao>,
    scopes: Vec<HashMap<String, SymbolInfo>>,
}

#[derive(Clone, Debug)]
pub struct SymbolInfo {
    pub name: String,
    pub symbol_type: SymbolType,
    pub scope_level: usize,
    pub is_mutable: bool,
}

#[derive(Clone, Debug)]
pub enum SymbolType {
    Variable(Tipo),
    Parameter(Tipo),
    Field(Tipo),
    Method(Vec<Tipo>, Option<Tipo>),
    Class(String),
}

impl SymbolTable {
    pub fn new() -> Self {
        Self {
            classes: HashMap::new(),
            functions: HashMap::new(),
            scopes: vec![HashMap::new()], // Escopo global
        }
    }
    
    pub fn register_class(&mut self, name: &str, classe: DeclaracaoClasse) -> Result<(), String> {
        if self.classes.contains_key(name) {
            return Err(format!("Classe '{}' já está definida", name));
        }
        
        self.classes.insert(name.to_string(), classe);
        Ok(())
    }
    
    pub fn register_function(&mut self, name: &str, funcao: DeclaracaoFuncao) -> Result<(), String> {
        if self.functions.contains_key(name) {
            return Err(format!("Função '{}' já está definida", name));
        }
        
        self.functions.insert(name.to_string(), funcao);
        Ok(())
    }
    
    pub fn find_class(&self, name: &str) -> Option<&DeclaracaoClasse> {
        self.classes.get(name)
    }
    
    pub fn find_function(&self, name: &str) -> Option<&DeclaracaoFuncao> {
        self.functions.get(name)
    }
    
    pub fn enter_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }
    
    pub fn exit_scope(&mut self) {
        self.scopes.pop();
    }
    
    pub fn define_variable(&mut self, name: &str, tipo: Tipo) -> Result<(), String> {
        let scope_level = self.scopes.len() - 1;
        
        if let Some(current_scope) = self.scopes.last_mut() {
            if current_scope.contains_key(name) {
                return Err(format!("Variável '{}' já está definida neste escopo", name));
            }
            
            current_scope.insert(name.to_string(), SymbolInfo {
                name: name.to_string(),
                symbol_type: SymbolType::Variable(tipo),
                scope_level,
                is_mutable: true,
            });
        }
        
        Ok(())
    }
    
    pub fn lookup_variable(&self, name: &str) -> Option<&SymbolInfo> {
        for scope in self.scopes.iter().rev() {
            if let Some(symbol) = scope.get(name) {
                return Some(symbol);
            }
        }
        None
    }
}