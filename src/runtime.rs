use std::collections::HashMap;

// ✅ Comentar para evitar dependência serde se não estiver disponível
// use serde::{Serialize, Deserialize};

// ✅ Bytecode Universal 
// #[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Debug, Clone)]
pub struct Bytecode {
    pub instrucoes: Vec<Instrucao>,
    pub constantes: Vec<ValorAvaliado>,
}

impl Bytecode {
    pub fn new() -> Self {
        Self {
            instrucoes: Vec::new(),
            constantes: Vec::new(),
        }
    }

    pub fn push(&mut self, instrucao: Instrucao) {
        self.instrucoes.push(instrucao);
    }

    pub fn obter_constante(&self, slot: usize) -> Option<ValorAvaliado> {
        self.constantes.get(slot).cloned()
    }
    
    pub fn constante(&mut self, valor: ValorAvaliado) -> usize {
        self.constantes.push(valor);
        self.constantes.len() - 1
    }

    pub fn len(&self) -> usize {
        self.instrucoes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.instrucoes.is_empty()
    }
}

// #[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Debug, Clone)]
pub enum Instrucao {
    AtribuirPropriedade {
        slot: usize,
        nome: String,
        constante: ValorAvaliado,
    },
    ChamarMetodo {
        objeto: String,
        metodo: String,
        argumentos: Vec<ValorAvaliado>,
    },
    ImprimirConstante(ValorAvaliado),
}

// #[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Debug, Clone)]
pub enum ValorAvaliado {
    Inteiro(i64),
    Texto(String),
    Booleano(bool),
    Objeto {
        classe: String,
        propriedades: HashMap<String, ValorAvaliado>,
    },
}

impl PartialEq for ValorAvaliado {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ValorAvaliado::Inteiro(a), ValorAvaliado::Inteiro(b)) => a == b,
            (ValorAvaliado::Texto(a), ValorAvaliado::Texto(b)) => a == b,
            (ValorAvaliado::Booleano(a), ValorAvaliado::Booleano(b)) => a == b,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bytecode_novo() {
        let bytecode = Bytecode::new();
        assert!(bytecode.is_empty());
        assert_eq!(bytecode.len(), 0);
    }

    #[test]
    fn test_bytecode_adicionar_instrucao() {
        let mut bytecode = Bytecode::new();
        let valor = ValorAvaliado::Texto("Olá".to_string());
        
        bytecode.push(Instrucao::ImprimirConstante(valor));
        assert_eq!(bytecode.len(), 1);
        assert!(!bytecode.is_empty());
    }

    #[test]
    fn test_valor_avaliado_igualdade() {
        let valor1 = ValorAvaliado::Inteiro(42);
        let valor2 = ValorAvaliado::Inteiro(42);
        let valor3 = ValorAvaliado::Inteiro(24);
        
        assert_eq!(valor1, valor2);
        assert_ne!(valor1, valor3);
    }
}