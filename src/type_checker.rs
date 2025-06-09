use crate::ast::*;
use std::collections::HashMap;

pub struct VerificadorTipos {
    variaveis: HashMap<String, Tipo>,
    erros: Vec<String>,
}

impl VerificadorTipos {
    pub fn new() -> Self {
        Self {
            variaveis: HashMap::new(),
            erros: Vec::new(),
        }
    }

    pub fn verificar_programa(&mut self, programa: &Programa) -> Result<(), Vec<String>> {
        for declaracao in &programa.declaracoes {
            self.verificar_declaracao(declaracao);
        }

        if self.erros.is_empty() {
            Ok(())
        } else {
            Err(self.erros.clone())
        }
    }

    fn verificar_declaracao(&mut self, declaracao: &Declaracao) {
        match declaracao {
            Declaracao::Comando(cmd) => self.verificar_comando(cmd),
            Declaracao::DeclaracaoFuncao(funcao) => {
                for comando in &funcao.corpo {
                    self.verificar_comando(comando);
                }
            },
            _ => {}
        }
    }

    fn verificar_comando(&mut self, comando: &Comando) {
        match comando {
            Comando::DeclaracaoVariavel(tipo, nome, _) => {
                self.variaveis.insert(nome.clone(), tipo.clone());
            },
            Comando::Atribuicao(nome, _) => {
                if !self.variaveis.contains_key(nome) {
                    self.erros.push(format!("Variável '{}' não foi declarada", nome));
                }
            },
            Comando::Bloco(comandos) => {
                for cmd in comandos {
                    self.verificar_comando(cmd);
                }
            },
            _ => {}
        }
    }
}