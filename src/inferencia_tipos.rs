use crate::ast::*;
use std::collections::HashMap;

pub struct InferenciaTipos {
    tipos_inferidos: HashMap<String, Tipo>,
}

impl InferenciaTipos {
    pub fn new() -> Self {
        Self { tipos_inferidos: HashMap::new() }
    }

    pub fn inferir_tipo(&mut self, expr: &Expressao) -> Result<Tipo, String> {
        match expr {
            Expressao::Inteiro(_)   => Ok(Tipo::Inteiro),
            Expressao::Texto(_)     => Ok(Tipo::Texto),
            Expressao::Booleano(_)  => Ok(Tipo::Booleano),
            Expressao::NovoObjeto(c, _) => Ok(Tipo::Classe(c.clone())),
            Expressao::Aritmetica(op, esq, dir) => {
                let t_esq = self.inferir_tipo(esq)?;
                let t_dir = self.inferir_tipo(dir)?;
                match (op, &t_esq, &t_dir) {
                    (OperadorAritmetico::Soma, Tipo::Texto, _)
                    | (OperadorAritmetico::Soma, _, Tipo::Texto) => Ok(Tipo::Texto),
                    (_, Tipo::Inteiro, Tipo::Inteiro)            => Ok(Tipo::Inteiro),
                    _ => Err("Tipos incompatíveis para operação aritmética".into()),
                }
            }
            Expressao::Comparacao(_, _, _) => Ok(Tipo::Booleano),
            Expressao::Identificador(n) =>
                self.tipos_inferidos.get(n).cloned()
                    .ok_or_else(|| format!("Não foi possível inferir o tipo de '{}'", n)),
            _ => Err("Tipo não pode ser inferido para esta expressão".into()),
        }
    }

    pub fn registrar_variavel(&mut self, nome: String, tipo: Tipo) {
        self.tipos_inferidos.insert(nome, tipo);
    }
    pub fn obter_tipo(&self, nome: &str) -> Option<&Tipo> {
        self.tipos_inferidos.get(nome)
    }
}