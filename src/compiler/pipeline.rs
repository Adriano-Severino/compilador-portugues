// src/compiler/pipeline.rs
use crate::ast::*;
use crate::compiler::namespace_resolver::NamespaceResolver;
use std::collections::HashMap;

pub struct CompilerPipeline {
    passes: Vec<Box<dyn CompilerPass>>,
    pub context: CompilationContext,
}

#[derive(Default)]
pub struct CompilationContext {
    pub symbols: SymbolTable,
    pub errors: Vec<CompilationError>,
    pub warnings: Vec<CompilationWarning>,
    pub optimizations: OptimizationFlags,
}

#[derive(Default)]
pub struct OptimizationFlags {
    pub enable_dead_code_elimination: bool,
    pub enable_constant_folding: bool,
}

#[derive(Debug, Clone)]
pub enum CompilationError {
    PassError { pass: String, message: String },
    TypeError { message: String },
    NameError { message: String },
}

impl std::fmt::Display for CompilationError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            CompilationError::PassError { pass, message } => {
                write!(f, "Erro no pass '{}': {}", pass, message)
            },
            CompilationError::TypeError { message } => {
                write!(f, "Erro de tipo: {}", message)
            },
            CompilationError::NameError { message } => {
                write!(f, "Erro de nome: {}", message)
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct CompilationWarning {
    pub message: String,
}

pub trait CompilerPass {
    fn name(&self) -> &str;
    fn run(&mut self, programa: &mut Programa, context: &mut CompilationContext) -> Result<(), String>;
    fn dependencies(&self) -> Vec<&str> { vec![] }
}

// Imports para tabela de símbolos
#[derive(Default, Clone)]
pub struct SymbolTable {
    classes: HashMap<String, DeclaracaoClasse>,
    functions: HashMap<String, DeclaracaoFuncao>,
}

impl SymbolTable {
    pub fn new() -> Self {
        Self::default()
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
}

// Stubs para os passes que ainda não existem
struct SymbolResolver;
impl SymbolResolver {
    fn new() -> Self { Self }
}
impl CompilerPass for SymbolResolver {
    fn name(&self) -> &str { "Symbol Resolver" }
    fn run(&mut self, _programa: &mut Programa, _context: &mut CompilationContext) -> Result<(), String> {
        Ok(())
    }
}

struct TypeChecker;
impl TypeChecker {
    fn new() -> Self { Self }
}
impl CompilerPass for TypeChecker {
    fn name(&self) -> &str { "Type Checker" }
    fn run(&mut self, _programa: &mut Programa, _context: &mut CompilationContext) -> Result<(), String> {
        Ok(())
    }
}

struct InheritanceValidator;
impl InheritanceValidator {
    fn new() -> Self { Self }
}
impl CompilerPass for InheritanceValidator {
    fn name(&self) -> &str { "Inheritance Validator" }
    fn run(&mut self, _programa: &mut Programa, _context: &mut CompilationContext) -> Result<(), String> {
        Ok(())
    }
}

struct OwnershipAnalyzer;
impl OwnershipAnalyzer {
    fn new() -> Self { Self }
}
impl CompilerPass for OwnershipAnalyzer {
    fn name(&self) -> &str { "Ownership Analyzer" }
    fn run(&mut self, _programa: &mut Programa, _context: &mut CompilationContext) -> Result<(), String> {
        Ok(())
    }
}

struct OptimizationPass;
impl OptimizationPass {
    fn new() -> Self { Self }
}
impl CompilerPass for OptimizationPass {
    fn name(&self) -> &str { "Optimization Pass" }
    fn run(&mut self, _programa: &mut Programa, _context: &mut CompilationContext) -> Result<(), String> {
        Ok(())
    }
}

impl CompilerPipeline {
    pub fn new() -> Self {
        let mut pipeline = Self {
            passes: Vec::new(),
            context: CompilationContext::default(),
        };
        
        // Ordem correta dos passes
        pipeline.add_pass(Box::new(NamespaceResolver::new()));
        pipeline.add_pass(Box::new(SymbolResolver::new()));
        pipeline.add_pass(Box::new(TypeChecker::new()));
        pipeline.add_pass(Box::new(InheritanceValidator::new()));
        pipeline.add_pass(Box::new(OwnershipAnalyzer::new()));
        pipeline.add_pass(Box::new(OptimizationPass::new()));
        
        pipeline
    }

    pub fn compile(&mut self, mut programa: Programa) -> Result<Programa, Vec<String>> {
        for pass in &mut self.passes {
            println!("Executando pass: {}", pass.name());
            
            if let Err(e) = pass.run(&mut programa, &mut self.context) {
                self.context.errors.push(CompilationError::PassError {
                    pass: pass.name().to_string(),
                    message: e,
                });
            }
        }

        if !self.context.errors.is_empty() {
            let error_messages: Vec<String> = self.context.errors
                .iter()
                .map(|e| e.to_string())
                .collect();
            return Err(error_messages);
        }

        Ok(programa)
    }

    fn add_pass(&mut self, pass: Box<dyn CompilerPass>) {
        self.passes.push(pass);
    }
}