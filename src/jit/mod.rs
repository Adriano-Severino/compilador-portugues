// JIT engine (feature-gated) usando Cranelift.
// Fase 1: esqueleto mínimo que permite compilar uma função "soma" para nativo como prova de conceito.

#[cfg(feature = "jit")]
pub mod cranelift_engine;

#[derive(Debug)]
pub enum JitError {
    NaoSuportado(&'static str),
    Interno(String),
}

#[cfg(feature = "jit")]
pub use cranelift_engine::{CraneliftJit, JitHandle};

#[cfg(not(feature = "jit"))]
pub struct CraneliftJit;

#[cfg(not(feature = "jit"))]
impl CraneliftJit {
    pub fn new() -> Result<Self, JitError> { Err(JitError::NaoSuportado("compilado sem feature 'jit'")) }
}
