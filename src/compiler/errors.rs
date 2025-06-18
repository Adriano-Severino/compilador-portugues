// src/compiler/errors.rs
use std::fmt;

#[derive(Debug, Clone)]
pub enum CompilationError {
    LexicalError { position: usize, message: String },
    SyntaxError { line: usize, column: usize, message: String },
    SemanticError { message: String },
    TypeMismatch { expected: String, found: String, location: String },
    UndefinedSymbol { symbol: String, location: String },
    DuplicateDefinition { symbol: String, location: String },
    PassError { pass: String, message: String },
}

#[derive(Debug, Clone)]
pub struct CompilationWarning {
    pub message: String,
    pub location: Option<String>,
    pub severity: WarningSeverity,
}

#[derive(Debug, Clone)]
pub enum WarningSeverity {
    Info,
    Warning,
    Suggestion,
}

impl fmt::Display for CompilationError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CompilationError::TypeMismatch { expected, found, location } => {
                write!(f, "Erro de tipo em {}: esperado '{}', encontrado '{}'", location, expected, found)
            },
            CompilationError::UndefinedSymbol { symbol, location } => {
                write!(f, "Símbolo '{}' não definido em {}", symbol, location)
            },
            _ => write!(f, "{:?}", self)
        }
    }
}