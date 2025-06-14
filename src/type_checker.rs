use crate::ast::*;
use std::collections::HashMap;

pub struct VerificadorTipos {
    variaveis: HashMap<String, Tipo>,
    classes: HashMap<String, DeclaracaoClasse>, // ✅ NOVO: Armazenar classes
    erros: Vec<String>,
}

impl VerificadorTipos {
    pub fn new() -> Self {
        Self {
            variaveis: HashMap::new(),
            classes: HashMap::new(), // ✅ NOVO
            erros: Vec::new(),
        }
    }

    pub fn verificar_programa(&mut self, programa: &Programa) -> Result<(), Vec<String>> {
        // ✅ NOVO: Primeiro passo - registrar todas as classes
        for declaracao in &programa.declaracoes {
            if let Declaracao::DeclaracaoClasse(classe) = declaracao {
                self.classes.insert(classe.nome.clone(), classe.clone());
            }
        }

        // ✅ NOVO: Segundo passo - verificar herança
        for declaracao in &programa.declaracoes {
            if let Declaracao::DeclaracaoClasse(classe) = declaracao {
                self.verificar_heranca_classe(classe);
            }
        }

        // ✅ EXISTENTE: Terceiro passo - verificar comandos
        for declaracao in &programa.declaracoes {
            self.verificar_declaracao(declaracao);
        }

        if self.erros.is_empty() {
            Ok(())
        } else {
            Err(self.erros.clone())
        }
    }

    // ✅ NOVO: Verificar herança da classe
    fn verificar_heranca_classe(&mut self, classe: &DeclaracaoClasse) {
        if let Some(classe_pai) = &classe.classe_pai {
            // Verificar se classe pai existe
            if !self.classes.contains_key(classe_pai) {
                self.erros.push(format!(
                    "Classe pai '{}' não encontrada para classe '{}'",
                    classe_pai, classe.nome
                ));
                return;
            }

            // Verificar dependência circular
            if self.tem_dependencia_circular(&classe.nome, classe_pai) {
                self.erros.push(format!(
                    "Dependência circular detectada: {} -> {}",
                    classe.nome, classe_pai
                ));
                return;
            }

            // Verificar métodos redefiníveis e sobrescreve
            for metodo in &classe.metodos {
                self.verificar_metodo_heranca(metodo, classe, classe_pai);
            }
        }

        // Verificar métodos sem herança
        for metodo in &classe.metodos {
            self.verificar_metodo_sem_heranca(metodo, classe);
        }
    }

    // ✅ NOVO: Verificar dependência circular
    fn tem_dependencia_circular(&self, classe_atual: &str, classe_pai: &str) -> bool {
        let mut visitadas = std::collections::HashSet::new();
        self.verificar_ciclo_recursivo(classe_pai, classe_atual, &mut visitadas)
    }

    fn verificar_ciclo_recursivo(
        &self,
        classe_verificando: &str,
        classe_origem: &str,
        visitadas: &mut std::collections::HashSet<String>,
    ) -> bool {
        if classe_verificando == classe_origem {
            return true;
        }

        if visitadas.contains(classe_verificando) {
            return false;
        }

        visitadas.insert(classe_verificando.to_string());

        if let Some(classe_def) = self.classes.get(classe_verificando) {
            if let Some(pai) = &classe_def.classe_pai {
                return self.verificar_ciclo_recursivo(pai, classe_origem, visitadas);
            }
        }

        false
    }

    // ✅ NOVO: Verificar método com herança
    fn verificar_metodo_heranca(
        &mut self,
        metodo: &MetodoClasse,
        classe: &DeclaracaoClasse,
        classe_pai: &str,
    ) {
        if metodo.eh_override {
            // Verificar se existe método redefinível na classe pai
            if let Some(metodo_pai) = self.buscar_metodo_redefinivel_pai(classe_pai, &metodo.nome) {
                // Verificar compatibilidade de assinatura
                if !self.assinaturas_compativeis(metodo, &metodo_pai) {
                    self.erros.push(format!(
                        "Método '{}' em classe '{}' sobrescreve método da classe pai '{}' mas as assinaturas são incompatíveis",
                        metodo.nome, classe.nome, classe_pai
                    ));
                }
            } else {
                self.erros.push(format!(
                    "Método '{}' marcado como 'sobrescreve' mas não existe método 'redefinível' na classe pai '{}'",
                    metodo.nome, classe_pai
                ));
            }
        }

        // Verificar se método redefinível não pode ter corpo vazio em classe não abstrata
        if metodo.eh_virtual && metodo.corpo.is_empty() && !classe.eh_abstrata {
            self.erros.push(format!(
                "Método redefinível '{}' deve ter implementação em classe não abstrata '{}'",
                metodo.nome, classe.nome
            ));
        }
    }

    // ✅ NOVO: Verificar método sem herança (não pode ter sobrescreve)
    fn verificar_metodo_sem_heranca(&mut self, metodo: &MetodoClasse, classe: &DeclaracaoClasse) {
        if metodo.eh_override && classe.classe_pai.is_none() {
            self.erros.push(format!(
                "Método '{}' marcado como 'sobrescreve' mas classe '{}' não tem classe pai",
                metodo.nome, classe.nome
            ));
        }

        if metodo.eh_virtual && metodo.eh_override {
            self.erros.push(format!(
                "Método '{}' não pode ser 'redefinível' e 'sobrescreve' ao mesmo tempo",
                metodo.nome
            ));
        }
    }

    // ✅ NOVO: Buscar método redefinível na hierarquia pai
    fn buscar_metodo_redefinivel_pai(&self, classe_pai: &str, nome_metodo: &str) -> Option<MetodoClasse> {
        let mut classe_atual = Some(classe_pai.to_string());

        while let Some(classe) = classe_atual {
            if let Some(def_classe) = self.classes.get(&classe) {
                // Procurar método redefinível na classe atual
                for metodo in &def_classe.metodos {
                    if metodo.nome == nome_metodo && metodo.eh_virtual {
                        return Some(metodo.clone());
                    }
                }
                // Ir para classe pai
                classe_atual = def_classe.classe_pai.clone();
            } else {
                break;
            }
        }

        None
    }

    // ✅ NOVO: Verificar compatibilidade de assinaturas
    fn assinaturas_compativeis(&self, metodo1: &MetodoClasse, metodo2: &MetodoClasse) -> bool {
        // Verificar tipo de retorno
        if metodo1.tipo_retorno != metodo2.tipo_retorno {
            return false;
        }

        // Verificar número de parâmetros
        if metodo1.parametros.len() != metodo2.parametros.len() {
            return false;
        }

        // Verificar tipos dos parâmetros
        for (param1, param2) in metodo1.parametros.iter().zip(metodo2.parametros.iter()) {
            if param1.tipo != param2.tipo {
                return false;
            }
        }

        true
    }

    fn verificar_declaracao(&mut self, declaracao: &Declaracao) {
        match declaracao {
            Declaracao::Comando(cmd) => self.verificar_comando(cmd),
            Declaracao::DeclaracaoFuncao(funcao) => {
                for comando in &funcao.corpo {
                    self.verificar_comando(comando);
                }
            }
            Declaracao::DeclaracaoClasse(classe) => {
                // ✅ NOVO: Verificar métodos da classe
                for metodo in &classe.metodos {
                    for comando in &metodo.corpo {
                        self.verificar_comando(comando);
                    }
                }
            }
            _ => {}
        }
    }

    fn verificar_comando(&mut self, comando: &Comando) {
        match comando {
            Comando::DeclaracaoVariavel(tipo, nome, _) => {
                self.variaveis.insert(nome.clone(), tipo.clone());
            }
            Comando::Atribuicao(nome, _) => {
                if !self.variaveis.contains_key(nome) {
                    self.erros.push(format!("Variável '{}' não foi declarada", nome));
                }
            }
            Comando::Bloco(comandos) => {
                for cmd in comandos {
                    self.verificar_comando(cmd);
                }
            }
            _ => {}
        }
    }
}