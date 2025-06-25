use crate::ast::*;
use std::collections::HashMap;

pub struct VerificadorTipos<'a> {
    usings: Vec<String>,
    simbolos_namespaces: HashMap<String, &'a Declaracao>,
    variaveis: HashMap<String, Tipo>,
    classes: HashMap<String, &'a DeclaracaoClasse>,
    erros: Vec<String>,
}

impl<'a> VerificadorTipos<'a> {
    pub fn new() -> Self {
        Self {
            usings: Vec::new(),
            simbolos_namespaces: HashMap::new(),
            variaveis: HashMap::new(),
            classes: HashMap::new(),
            erros: Vec::new(),
        }
    }

    pub fn verificar_programa(&mut self, programa: &'a Programa) -> Result<(), Vec<String>> {
        // 1. Registrar todos os símbolos de todos os namespaces
        for ns in &programa.namespaces {
            for decl in &ns.declaracoes {
                let nome_completo = format!("{}.{}", ns.nome, self.get_declaracao_nome(decl));
                self.simbolos_namespaces.insert(nome_completo, decl);

                if let Declaracao::DeclaracaoClasse(classe) = decl {
                    self.classes.insert(classe.nome.clone(), classe);
                }
            }
        }

        // 2. Registrar os usings
        for u in &programa.usings {
            self.usings.push(u.caminho.clone());
        }

        // 3. Registrar classes globais
        for declaracao in &programa.declaracoes {
            if let Declaracao::DeclaracaoClasse(classe) = declaracao {
                self.classes.insert(classe.nome.clone(), classe);
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
    fn get_declaracao_nome(&self, declaracao: &Declaracao) -> String {
        match declaracao {
            Declaracao::DeclaracaoFuncao(f) => f.nome.clone(),
            Declaracao::DeclaracaoClasse(c) => c.nome.clone(),
            _ => "".to_string(),
        }
    }

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
                if !self.assinaturas_compativeis(metodo, metodo_pai) {
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
    fn buscar_metodo_redefinivel_pai(&self, classe_pai: &str, nome_metodo: &str) -> Option<&'a MetodoClasse> {
        let mut classe_atual = self.classes.get(classe_pai);

        while let Some(def_classe) = classe_atual {
            // Procurar método redefinível na classe atual
            for metodo in &def_classe.metodos {
                if metodo.nome == nome_metodo && metodo.eh_virtual {
                    return Some(metodo);
                }
            }
            // Ir para a classe pai
            if let Some(pai_nome) = &def_classe.classe_pai {
                classe_atual = self.classes.get(pai_nome);
            } else {
                classe_atual = None;
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
        if let Comando::Expressao(Expressao::Chamada(nome_funcao, _)) = comando {
            self.resolver_funcao(nome_funcao);
        }

        if let Comando::DeclaracaoVariavel(Tipo::Classe(nome_classe), _, _) = comando {
            self.resolver_tipo_classe(nome_classe);
        }

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

    fn resolver_tipo_classe(&mut self, nome_classe: &str) {
        // 1. Tenta encontrar no escopo global
        if self.classes.contains_key(nome_classe) {
            return;
        }

        // 2. Tenta encontrar usando os 'usings'
        for u in &self.usings {
            let nome_completo = format!("{}.{}", u, nome_classe);
            if let Some(decl) = self.simbolos_namespaces.get(&nome_completo) {
                if let Declaracao::DeclaracaoClasse(_) = decl {
                    // Encontrou a classe via 'usando', está ok.
                    return;
                }
            }
        }

        self.erros.push(format!("Tipo ou classe '{}' não encontrada.", nome_classe));
    }

    fn resolver_funcao(&mut self, nome_funcao: &str) {
        // 1. Tenta encontrar no escopo global (não implementado ainda, mas deveria)

        // 2. Tenta encontrar usando os 'usings'
        for u in &self.usings {
            let nome_completo = format!("{}.{}", u, nome_funcao);
            if self.simbolos_namespaces.contains_key(&nome_completo) {
                return; // Encontrou
            }
        }

        self.erros.push(format!("Função '{}' não encontrada.", nome_funcao));
    }
}