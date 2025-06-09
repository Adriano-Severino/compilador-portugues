use crate::ast::*;
use std::collections::HashMap;

pub struct InferenciaTipos {
    tipos_inferidos: HashMap<String, Tipo>,
}

impl InferenciaTipos {
    pub fn new() -> Self {
        Self {
            tipos_inferidos: HashMap::new(),
        }
    }

    pub fn inferir_tipo(&mut self, expr: &Expressao) -> Result<Tipo, String> {
        match expr {
            Expressao::Inteiro(_) => Ok(Tipo::Inteiro),
            Expressao::Texto(_) => Ok(Tipo::Texto),
            Expressao::Booleano(_) => Ok(Tipo::Booleano),
            
            Expressao::NovoObjeto(classe, _) => Ok(Tipo::Classe(classe.clone())),
            
            Expressao::Aritmetica(op, esq, dir) => {
                let tipo_esq = self.inferir_tipo(esq)?;
                let tipo_dir = self.inferir_tipo(dir)?;
                
                match (op, &tipo_esq, &tipo_dir) {
                    (OperadorAritmetico::Soma, Tipo::Texto, _) => Ok(Tipo::Texto),
                    (OperadorAritmetico::Soma, _, Tipo::Texto) => Ok(Tipo::Texto),
                    (_, Tipo::Inteiro, Tipo::Inteiro) => Ok(Tipo::Inteiro),
                    _ => Err("Tipos incompatíveis para operação aritmética".to_string()),
                }
            }
            
            Expressao::Comparacao(_, _, _) => Ok(Tipo::Booleano),
            
            Expressao::Identificador(nome) => {
                if let Some(tipo) = self.tipos_inferidos.get(nome) {
                    Ok(tipo.clone())
                } else {
                    Err(format!("Não foi possível inferir o tipo da variável '{}'", nome))
                }
            }
            
            _ => Err("Tipo não pode ser inferido para esta expressão".to_string()),
        }
    }

    pub fn registrar_variavel(&mut self, nome: String, tipo: Tipo) {
        self.tipos_inferidos.insert(nome, tipo);
    }

    pub fn obter_tipo(&self, nome: &str) -> Option<&Tipo> {
        self.tipos_inferidos.get(nome)
    }
}