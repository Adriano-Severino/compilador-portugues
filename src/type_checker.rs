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

    pub fn is_class(&self, name: &str) -> bool {
        self.classes.contains_key(name)
    }

    pub fn verificar_programa(&mut self, programa: &'a Programa) -> Result<(), Vec<String>> {
        println!("--- Verificador de Tipos: Iniciando verificação do programa ---");

        // Fase 1: Registrar todos os usings.
        println!("[Fase 1] Registrando usings...");
        for u in &programa.usings {
            self.usings.push(u.caminho.clone());
        }
        println!("[Fase 1] Usings registrados: {:?}", self.usings);

        // Fase 2: Registrar todos os nomes de classes e funções (primeira passada).
        println!("[Fase 2] Registrando definições de namespaces e globais...");
        for ns in &programa.namespaces {
            for decl in &ns.declaracoes {
                let nome_simples = self.get_declaracao_nome(decl);
                let nome_completo = format!("{}.{}", ns.nome, nome_simples);
                println!("  - Registrando símbolo de namespace: {}", nome_completo);
                self.simbolos_namespaces.insert(nome_completo.clone(), decl);

                if let Declaracao::DeclaracaoClasse(classe) = decl {
                    self.classes.insert(nome_completo, classe);
                }
            }
        }
        for declaracao in &programa.declaracoes {
            let nome_simples = self.get_declaracao_nome(declaracao);
            if !nome_simples.is_empty() {
                println!("  - Registrando símbolo global: {}", nome_simples);
                self.simbolos_namespaces.insert(nome_simples.clone(), declaracao);
                if let Declaracao::DeclaracaoClasse(classe) = declaracao {
                    self.classes.insert(nome_simples, classe);
                }
            }
        }
        println!("[Fase 2] Registro concluído.");

        // Fase 3: Verificar tudo com o contexto de namespace correto.
        println!("[Fase 3] Verificando declarações...");
        for ns in &programa.namespaces {
            let namespace_atual = &ns.nome;
            println!("  -> Verificando namespace: {}", namespace_atual);
            for declaracao in &ns.declaracoes {
                println!("    - Verificando declaração: {}", self.get_declaracao_nome(declaracao));
                self.verificar_declaracao(declaracao, namespace_atual);
            }
        }
        println!("  -> Verificando declarações globais...");
        for declaracao in &programa.declaracoes {
            println!("    - Verificando declaração global: {}", self.get_declaracao_nome(declaracao));
            self.verificar_declaracao(declaracao, ""); // Namespace global
        }
        println!("[Fase 3] Verificação concluída.");

        if self.erros.is_empty() {
            println!("--- Verificador de Tipos: Verificação concluída sem erros. ---");
            Ok(())
        } else {
            println!("--- Verificador de Tipos: Verificação concluída com {} erros. ---", self.erros.len());
            Err(self.erros.clone())
        }
    }

    pub fn resolver_nome_classe(&self, nome_classe: &str, namespace_atual: &str) -> String {


        // Se o nome já for qualificado, retorne-o.
        if nome_classe.contains('.') {
            return nome_classe.to_string();
        }

        // 1. Tente resolver no namespace atual.
        if !namespace_atual.is_empty() {
            let nome_completo = format!("{}.{}", namespace_atual, nome_classe);
            if let Some(decl) = self.simbolos_namespaces.get(&nome_completo) {
                if let Declaracao::DeclaracaoClasse(_) = decl {
                    return nome_completo;
                }
            }
        }

        // 2. Tente resolver usando os `usings`.
        for using_path in &self.usings {
            let nome_completo = format!("{}.{}", using_path, nome_classe);
            if let Some(decl) = self.simbolos_namespaces.get(&nome_completo) {
                if let Declaracao::DeclaracaoClasse(_) = decl {
                    return nome_completo;
                }
            }
        }

        // 3. Verifique se é uma classe global (sem namespace).
        if self.classes.contains_key(nome_classe) {
            return nome_classe.to_string();
        }

        // Se não for encontrado, retorne o nome original como fallback.
        nome_classe.to_string()
    }

    pub fn resolver_nome_funcao(&self, nome_funcao: &str, namespace_atual: &str) -> String {
        // Se o nome já for qualificado, retorne-o.
        if nome_funcao.contains('.') {
            return nome_funcao.to_string();
        }

        // 1. Tente resolver no namespace atual.
        if !namespace_atual.is_empty() {
            let nome_completo = format!("{}.{}", namespace_atual, nome_funcao);
            if let Some(decl) = self.simbolos_namespaces.get(&nome_completo) {
                if let Declaracao::DeclaracaoFuncao(_) = decl {
                    return nome_completo;
                }
            }
        }

        // 2. Tente resolver usando os `usings`.
        for using_path in &self.usings {
            let nome_completo = format!("{}.{}", using_path, nome_funcao);
            if let Some(decl) = self.simbolos_namespaces.get(&nome_completo) {
                if let Declaracao::DeclaracaoFuncao(_) = decl {
                    return nome_completo;
                }
            }
        }

        // 3. Verifique se é uma função global (sem namespace).
        if let Some(decl) = self.simbolos_namespaces.get(nome_funcao) {
            if let Declaracao::DeclaracaoFuncao(_) = decl {
                // A chave em `simbolos_namespaces` para símbolos com namespace contém um ponto.
                // Se a chave for apenas `nome_funcao`, deve ser global.
                return nome_funcao.to_string();
            }
        }

        // Se não for encontrado, retorne o nome original como fallback.
        nome_funcao.to_string()
    }

    // ✅ NOVO: Verificar herança da classe
    fn get_declaracao_nome(&self, declaracao: &Declaracao) -> String {
        match declaracao {
            Declaracao::DeclaracaoFuncao(f) => f.nome.clone(),
            Declaracao::DeclaracaoClasse(c) => c.nome.clone(),
            _ => "".to_string(),
        }
    }

    fn verificar_heranca_classe(&mut self, classe: &DeclaracaoClasse, namespace_atual: &str) {
        if let Some(classe_pai_simples) = &classe.classe_pai {
            let nome_pai_completo = self.resolver_nome_classe(classe_pai_simples, namespace_atual);
    
            if !self.classes.contains_key(&nome_pai_completo) {
                self.erros.push(format!(
                    "Classe pai \"{}\" não encontrada para classe \"{}\"",
                    classe_pai_simples, classe.nome
                ));
                return;
            }
    
            let nome_classe_completo = if namespace_atual.is_empty() {
                classe.nome.clone()
            } else {
                format!("{}.{}", namespace_atual, classe.nome)
            };
    
            if self.tem_dependencia_circular(&nome_classe_completo, &nome_pai_completo) {
                self.erros.push(format!(
                    "Dependência circular detectada: {} -> {}",
                    classe.nome, classe_pai_simples
                ));
                return;
            }
    
            for metodo in &classe.metodos {
                self.verificar_metodo_heranca(metodo, classe, &nome_pai_completo);
            }
        }
    
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
                        "Método \"{}\" em classe \"{}\" sobrescreve método da classe pai \"{}\" mas as assinaturas são incompatíveis",
                        metodo.nome, classe.nome, classe_pai
                    ));
                }
            } else {
                self.erros.push(format!(
                    "Método \"{}\" marcado como \"sobrescreve\" mas não existe método \"redefinível\" na classe pai \"{}\"",
                    metodo.nome, classe_pai
                ));
            }
        }

        // Verificar se método redefinível não pode ter corpo vazio em classe não abstrata
        if metodo.eh_virtual && metodo.corpo.is_empty() && !classe.eh_abstrata {
            self.erros.push(format!(
                "Método redefinível \"{}\" deve ter implementação em classe não abstrata \"{}\"",
                metodo.nome, classe.nome
            ));
        }
    }

    // ✅ NOVO: Verificar método sem herança (não pode ter sobrescreve)
    fn verificar_metodo_sem_heranca(&mut self, metodo: &MetodoClasse, classe: &DeclaracaoClasse) {
        if metodo.eh_override && classe.classe_pai.is_none() {
            self.erros.push(format!(
                "Método \"{}\" marcado como \"sobrescreve\" mas classe \"{}\" não tem classe pai",
                metodo.nome, classe.nome
            ));
        }

        if metodo.eh_virtual && metodo.eh_override {
            self.erros.push(format!(
                "Método \"{}\" não pode ser \"redefinível\" e \"sobrescreve\" ao mesmo tempo",
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

    fn verificar_declaracao(&mut self, declaracao: &'a Declaracao, namespace_atual: &str) {
        match declaracao {
            Declaracao::DeclaracaoClasse(classe) => {
                self.verificar_heranca_classe(classe, namespace_atual);
                for metodo in &classe.metodos {
                    for comando in &metodo.corpo {
                        self.verificar_comando(comando, namespace_atual);
                    }
                }
            }
            Declaracao::DeclaracaoFuncao(funcao) => {
                for comando in &funcao.corpo {
                    self.verificar_comando(comando, namespace_atual);
                    }
                }
            Declaracao::Comando(cmd) => self.verificar_comando(cmd, namespace_atual),
            _ => {}
        }
    }

    fn verificar_comando(&mut self, comando: &Comando, namespace_atual: &str) {
        match comando {
            Comando::Expressao(Expressao::Chamada(nome_funcao, _)) => {
                let nome_resolvido = self.resolver_nome_funcao(nome_funcao, namespace_atual);
                if !self.simbolos_namespaces.contains_key(&nome_resolvido) {
                     self.erros.push(format!("Função \"{}\" não encontrada.", nome_funcao));
                }
            }
            Comando::DeclaracaoVariavel(tipo, nome, opt_expr) => {
                if let Some(expr) = opt_expr {
                    if let Expressao::NovoObjeto(classe_nome, _) = expr {
                        let nome_resolvido = self.resolver_nome_classe(classe_nome, namespace_atual);
                        if let Some(classe_def) = self.classes.get(&nome_resolvido) {
                            if classe_def.eh_estatica {
                                self.erros.push(format!("A classe estática '{}' não pode ser instanciada.", classe_nome));
                            }
                        }
                    }
                }

                if let Tipo::Classe(nome_classe) = tipo {
                    let nome_resolvido = self.resolver_nome_classe(nome_classe, namespace_atual);
                    if !self.classes.contains_key(&nome_resolvido) {
                        self.erros.push(format!("Tipo ou classe \"{}\" não encontrada.", nome_classe));
                    }
                }
                self.variaveis.insert(nome.clone(), tipo.clone());
            }
            Comando::Atribuicao(nome, _) => {
                if !self.variaveis.contains_key(nome) {
                    self.erros.push(format!("Variável \"{}\" não foi declarada", nome));
                }
            }
            Comando::Bloco(comandos) => {
                for cmd in comandos {
                    self.verificar_comando(cmd, namespace_atual);
                }
            }
            _ => {}
        }
    }
}