use crate::ast;
use crate::ast::*;
use std::collections::HashMap;

#[derive(Clone)]
pub struct VerificadorTipos<'a> {
    usings: Vec<String>,
    simbolos_namespaces: HashMap<String, &'a Declaracao>,
    pub classes: HashMap<String, &'a DeclaracaoClasse>,
    pub interfaces: HashMap<String, &'a ast::DeclaracaoInterface>,
    pub enums: HashMap<String, &'a DeclaracaoEnum>,
    pub resolved_classes: HashMap<String, ResolvedClassInfo<'a>>,
    erros: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedClassInfo<'a> {
    pub name: String,
    pub parent_name: Option<String>,
    pub properties: Vec<&'a ast::PropriedadeClasse>,
    pub fields: Vec<&'a ast::CampoClasse>,
    pub methods: HashMap<String, &'a ast::MetodoClasse>,
    pub eh_estatica: bool,
    // nova flag não essencial para layout, mas útil para checks em codegen/semântica
    pub eh_abstrata: bool,
    pub interfaces: Vec<String>,
}

impl<'a> VerificadorTipos<'a> {
    pub fn new() -> Self {
        Self {
            usings: Vec::new(),
            simbolos_namespaces: HashMap::new(),
            classes: HashMap::new(),
            interfaces: HashMap::new(),
            enums: HashMap::new(),
            resolved_classes: HashMap::new(),
            erros: Vec::new(),
        }
    }

    // Compatibilidade de tipos para atribuição: permite promoções numéricas (widening)
    fn tipos_compativeis_atribuicao(&self, destino: &Tipo, origem: &Tipo) -> bool {
        use Tipo::*;
        if destino == origem {
            return true;
        }
        match (destino, origem) {
            // Subtipagem de classes: permite atribuir derivada em variável do tipo base
            (Classe(dest), Classe(orig)) => {
                if dest == orig {
                    true
                } else {
                    self.is_subclass_of(orig, dest)
                }
            }
            // Enums: somente o mesmo enum é compatível implicitamente
            (Enum(a), Enum(b)) if a == b => true,
            // Texto aceita conversão implícita de inteiro/booleano (compatibilidade existente)
            (Texto, Inteiro) | (Texto, Booleano) => true,
            // Promoções numéricas
            (Flutuante, Inteiro) => true,
            (Duplo, Inteiro) => true,
            (Duplo, Flutuante) => true,
            _ => false,
        }
    }

    // Verifica se `sub` é subclasse (direta ou indireta) de `base`. Parâmetros são FQN.
    fn is_subclass_of(&self, sub: &str, base: &str) -> bool {
        if sub == base {
            return true;
        }
        let mut current = Some(sub.to_string());
        while let Some(cls_fqn) = current {
            if let Some(decl) = self.classes.get(&cls_fqn) {
                if let Some(parent_simple) = &decl.classe_pai {
                    let parent_fqn = self.resolver_nome_classe(
                        parent_simple,
                        &self.get_namespace_from_full_name(&cls_fqn),
                    );
                    if parent_fqn == base {
                        return true;
                    }
                    current = Some(parent_fqn);
                    continue;
                }
            }
            break;
        }
        false
    }

    pub fn verificar_programa(&mut self, programa: &'a Programa) -> Result<(), Vec<String>> {
        // 1. usings
        self.usings = programa.usings.iter().map(|u| u.caminho.clone()).collect();

        // 2. primeira passagem: registra classes, interfaces e enums
        for decl in &programa.declaracoes {
            let nome = self.get_declaracao_nome(decl);
            if let Declaracao::DeclaracaoClasse(cl) = decl {
                self.classes.insert(nome.clone(), cl);
            }
            if let Declaracao::DeclaracaoInterface(interf) = decl {
                self.interfaces.insert(nome.clone(), interf);
            }
            if let Declaracao::DeclaracaoEnum(en) = decl {
                self.enums.insert(nome.clone(), en);
            }
            self.simbolos_namespaces.insert(nome, decl);
        }
        for ns in &programa.namespaces {
            for decl in &ns.declaracoes {
                let nome_simples = self.get_declaracao_nome(decl);
                let fqn = format!("{}.{}", ns.nome, nome_simples);
                if let Declaracao::DeclaracaoClasse(cl) = decl {
                    self.classes.insert(fqn.clone(), cl);
                }
                if let Declaracao::DeclaracaoInterface(interf) = decl {
                    self.interfaces.insert(fqn.clone(), interf);
                }
                if let Declaracao::DeclaracaoEnum(en) = decl {
                    self.enums.insert(fqn.clone(), en);
                }
                self.simbolos_namespaces.insert(fqn, decl);
            }
        }

        // 3. resolve hierarquias agora que `self.classes` está cheia
        let classes_snapshot = self.classes.clone();
        for (nome, decl) in &classes_snapshot {
            self.resolve_class_hierarchy(nome, decl);
        }

        // 4. segunda passagem: verificação completa
        let mut vars_globais = HashMap::new();
        for decl in &programa.declaracoes {
            self.verificar_declaracao(decl, "", &mut vars_globais);
        }
        for ns in &programa.namespaces {
            self.verificar_namespace(ns);
        }

        // 5. validação de interfaces implementadas por classes
        for (fqn, classe) in &self.classes {
            let ns_atual = self.get_namespace_from_full_name(fqn);
            let classe_eh_abstrata = classe.eh_abstrata;
            // métodos resolvidos (inclui herdados)
            let resolved_methods = self
                .resolved_classes
                .get(fqn)
                .map(|ci| &ci.methods)
                .cloned()
                .unwrap_or_default();
            // lista de interfaces implementadas: AST + detectadas na resolução
            let mut ifaces_lista: Vec<String> = classe.interfaces.clone();
            if let Some(ci) = self.resolved_classes.get(fqn) {
                for i in &ci.interfaces {
                    if !ifaces_lista.contains(i) {
                        ifaces_lista.push(i.clone());
                    }
                }
            }

            for iface_nome in &ifaces_lista {
                let iface_fqn = self.resolver_nome_interface(iface_nome, &ns_atual);
                if let Some(iface) = self.interfaces.get(&iface_fqn) {
                    for sig in &iface.metodos {
                        let ret_i = sig
                            .tipo_retorno
                            .clone()
                            .or(Some(Tipo::Vazio))
                            .map(|t| match t {
                                Tipo::Classe(ref n) => {
                                    Tipo::Classe(self.resolver_nome_classe(n, &ns_atual))
                                }
                                Tipo::Enum(ref n) => {
                                    Tipo::Enum(self.resolver_nome_enum(n, &ns_atual))
                                }
                                other => other,
                            })
                            .unwrap();
                        let params_i: Vec<Tipo> = sig
                            .parametros
                            .iter()
                            .map(|p| match &p.tipo {
                                Tipo::Classe(n) => {
                                    Tipo::Classe(self.resolver_nome_classe(n, &ns_atual))
                                }
                                Tipo::Enum(n) => Tipo::Enum(self.resolver_nome_enum(n, &ns_atual)),
                                other => other.clone(),
                            })
                            .collect();

                        if let Some(m) = resolved_methods.get(&sig.nome) {
                            let (ret_c, params_c) = self.assinatura_metodo(m);
                            if ret_c != Some(ret_i.clone()) || params_c != params_i {
                                self.erros.push(format!(
                                    "Classe '{}' não implementa corretamente método '{}' da interface '{}'. Assinatura esperada: ({:?}) -> {:?}",
                                    fqn, sig.nome, iface_fqn, params_i, ret_i
                                ));
                            }
                        } else if !classe_eh_abstrata {
                            self.erros.push(format!(
                                "Classe '{}' não implementa método obrigatório '{}' da interface '{}'",
                                fqn, sig.nome, iface_fqn
                            ));
                        }
                    }
                } else {
                    self.erros.push(format!(
                        "Interface '{}' não encontrada (referenciada por '{}')",
                        iface_nome, fqn
                    ));
                }
            }
        }

        if self.erros.is_empty() {
            Ok(())
        } else {
            Err(self.erros.clone())
        }
    }

    pub fn resolver_nome_interface(&self, nome_iface: &str, namespace_atual: &str) -> String {
        if nome_iface.contains('.') {
            return nome_iface.to_string();
        }
        if !namespace_atual.is_empty() {
            let fqn = format!("{}.{}", namespace_atual, nome_iface);
            if self.interfaces.contains_key(&fqn) {
                return fqn;
            }
        }
        for using_path in &self.usings {
            let fqn = format!("{}.{}", using_path, nome_iface);
            if self.interfaces.contains_key(&fqn) {
                return fqn;
            }
        }
        nome_iface.to_string()
    }

    fn assinatura_metodo(&self, m: &'a ast::MetodoClasse) -> (Option<Tipo>, Vec<Tipo>) {
        let ret = m.tipo_retorno.clone().or(Some(Tipo::Vazio));
        let params = m.parametros.iter().map(|p| p.tipo.clone()).collect();
        (ret, params)
    }

    fn encontrar_metodo_na_base(
        &self,
        mut parent_name: Option<String>,
        nome: &str,
    ) -> Option<&'a ast::MetodoClasse> {
        while let Some(pn) = parent_name {
            if let Some(parent_decl) = self.classes.get(&pn) {
                if let Some(found) = parent_decl.metodos.iter().find(|m| m.nome == nome) {
                    return Some(found);
                }
                parent_name = parent_decl.classe_pai.clone().map(|simple| {
                    self.resolver_nome_classe(&simple, &self.get_namespace_from_full_name(&pn))
                });
            } else {
                break;
            }
        }
        None
    }

    fn resolve_class_hierarchy(&mut self, class_name: &str, class_decl: &'a DeclaracaoClasse) {
        let mut stack: Vec<String> = Vec::new();
        self.resolve_class_hierarchy_with_stack(class_name, class_decl, &mut stack);
    }

    fn resolve_class_hierarchy_with_stack(
        &mut self,
        class_name: &str,
        class_decl: &'a DeclaracaoClasse,
        stack: &mut Vec<String>,
    ) {
        if self.resolved_classes.contains_key(class_name) {
            return;
        }

        if stack.contains(&class_name.to_string()) {
            // ciclo direto (auto-referência) — reporte e pare
            let mut ciclo = stack.clone();
            ciclo.push(class_name.to_string());
            self.erros.push(format!(
                "Herança circular detectada: {}",
                ciclo.join(" -> ")
            ));
            return;
        }

        stack.push(class_name.to_string());

        // Para herança correta no backend LLVM, os membros do pai devem vir primeiro
        // no layout da classe, seguidos pelos membros específicos do filho (base-prefix layout).
        let mut properties: Vec<&'a ast::PropriedadeClasse> = Vec::new();
        let mut fields: Vec<&'a ast::CampoClasse> = Vec::new();
        let mut methods: HashMap<String, &'a ast::MetodoClasse> = class_decl
            .metodos
            .iter()
            .map(|m| (m.nome.clone(), m))
            .collect();

        // Vamos calcular dinamicamente o pai e as interfaces finais, pois o primeiro item após ':' pode ser uma interface
        let mut interfaces_final: Vec<String> = class_decl.interfaces.clone();
        let mut parent_effective: Option<String> = None;
        if let Some(parent_name_simple) = &class_decl.classe_pai {
            let parent_name = self.resolver_nome_classe(
                parent_name_simple,
                &self.get_namespace_from_full_name(class_name),
            );

            if parent_name == class_name || stack.contains(&parent_name) {
                // Detecta ciclo A -> ... -> B -> A
                let mut ciclo = stack.clone();
                ciclo.push(parent_name.clone());
                self.erros.push(format!(
                    "Herança circular detectada: {}",
                    ciclo.join(" -> ")
                ));
            } else if let Some(parent_decl) = self.classes.get(&parent_name).copied() {
                // Resolve pai primeiro (DFS)
                self.resolve_class_hierarchy_with_stack(&parent_name, parent_decl, stack);
                if let Some(parent_info) = self.resolved_classes.get(&parent_name) {
                    // Herda membros do pai, preservando ordem
                    properties.extend(parent_info.properties.iter().cloned());
                    fields.extend(parent_info.fields.iter().cloned());
                    // Métodos do pai entram se não forem sobrescritos pelo filho
                    for (name, method) in &parent_info.methods {
                        methods.entry(name.clone()).or_insert(method);
                    }
                }
                parent_effective = Some(parent_name.clone());
            } else {
                // Não é classe — pode ser uma interface listada após ':' (estilo C#)
                let iface_fqn = self.resolver_nome_interface(
                    parent_name_simple,
                    &self.get_namespace_from_full_name(class_name),
                );
                if self.interfaces.contains_key(&iface_fqn) {
                    interfaces_final.push(parent_name_simple.clone());
                    // Sem classe pai efetiva
                } else {
                    // Nem classe, nem interface conhecida — erro
                    self.erros.push(format!(
                        "Classe pai '{}' não encontrada para '{}'.",
                        parent_name, class_name
                    ));
                }
            }
        }

        // Agora adiciona os membros do próprio filho (sem duplicados), ao final
        for p in &class_decl.propriedades {
            if !properties.iter().any(|ep| ep.nome == p.nome) {
                properties.push(p);
            }
        }
        for f in &class_decl.campos {
            if !fields.iter().any(|ef| ef.nome == f.nome) {
                fields.push(f);
            }
        }

        self.resolved_classes.insert(
            class_name.to_string(),
            ResolvedClassInfo {
                name: class_name.to_string(),
                parent_name: parent_effective,
                properties,
                fields,
                methods,
                eh_estatica: class_decl.eh_estatica,
                eh_abstrata: class_decl.eh_abstrata,
                interfaces: interfaces_final,
            },
        );

        stack.pop();
    }

    pub fn is_static_class(&self, class_name: &str) -> bool {
        if let Some(class_info) = self.resolved_classes.get(class_name) {
            class_info.eh_estatica
        } else if let Some(class_decl) = self.classes.get(class_name) {
            class_decl.eh_estatica
        } else {
            false
        }
    }

    fn get_namespace_from_full_name(&self, full_name: &str) -> String {
        if let Some(pos) = full_name.rfind('.') {
            full_name[..pos].to_string()
        } else {
            "".to_string()
        }
    }

    fn verificar_namespace(&mut self, ns: &'a DeclaracaoNamespace) {
        let mut ns_vars = HashMap::new();
        for decl in &ns.declaracoes {
            self.verificar_declaracao(decl, &ns.nome, &mut ns_vars);
        }
    }

    pub fn resolver_nome_classe(&self, nome_classe: &str, namespace_atual: &str) -> String {
        println!(
            "DEBUG: Resolvendo nome de classe: \"{}\", namespace atual: \"{}\"",
            nome_classe, namespace_atual
        );
        if nome_classe.contains('.') {
            println!("DEBUG: Nome já qualificado: {}", nome_classe);
            return nome_classe.to_string();
        }
        if !namespace_atual.is_empty() {
            let fqn = format!("{}.{}", namespace_atual, nome_classe);
            println!("DEBUG: Tentando FQN com namespace atual: {}", fqn);
            if self.classes.contains_key(&fqn) {
                println!("DEBUG: Encontrado FQN com namespace atual: {}", fqn);
                return fqn;
            }
        }
        for using_path in &self.usings {
            let fqn = format!("{}.{}", using_path, nome_classe);
            println!("DEBUG: Tentando FQN com using: {}", fqn);
            if self.classes.contains_key(&fqn) {
                println!("DEBUG: Encontrado FQN com using: {}", fqn);
                return fqn;
            }
        }
        if self.classes.contains_key(nome_classe) {
            println!("DEBUG: Encontrado como classe global: {}", nome_classe);
            return nome_classe.to_string();
        }
        println!(
            "DEBUG: Classe \"{}\" não resolvida. Retornando nome original.",
            nome_classe
        );
        nome_classe.to_string()
    }

    pub fn resolver_nome_funcao(&self, nome_funcao: &str, namespace_atual: &str) -> String {
        if nome_funcao.contains('.') {
            return nome_funcao.to_string();
        }
        if !namespace_atual.is_empty() {
            let fqn = format!("{}.{}", namespace_atual, nome_funcao);
            if let Some(decl) = self.simbolos_namespaces.get(&fqn) {
                if let Declaracao::DeclaracaoFuncao(_) = *decl {
                    return fqn;
                }
            }
        }
        for using_path in &self.usings {
            let fqn = format!("{}.{}", using_path, nome_funcao);
            if let Some(decl) = self.simbolos_namespaces.get(&fqn) {
                if let Declaracao::DeclaracaoFuncao(_) = *decl {
                    return fqn;
                }
            }
        }
        if let Some(decl) = self.simbolos_namespaces.get(nome_funcao) {
            if let Declaracao::DeclaracaoFuncao(_) = *decl {
                return nome_funcao.to_string();
            }
        }
        nome_funcao.to_string()
    }

    pub fn is_member_of_class(&self, class_name: &str, member_name: &str) -> bool {
        if let Some(class_info) = self.resolved_classes.get(class_name) {
            return class_info.fields.iter().any(|f| f.nome == member_name)
                || class_info.properties.iter().any(|p| p.nome == member_name);
        }
        false
    }

    pub fn get_field_info(&self, class_name: &str, field_name: &str) -> Option<(u32, Tipo)> {
        if let Some(class_info) = self.resolved_classes.get(class_name) {
            if let Some(pos) = class_info.fields.iter().position(|f| f.nome == field_name) {
                return Some((pos as u32, class_info.fields[pos].tipo.clone()));
            }
            if let Some(pos) = class_info
                .properties
                .iter()
                .position(|p| p.nome == field_name)
            {
                return Some((pos as u32, class_info.properties[pos].tipo.clone()));
            }
        }
        None
    }

    pub fn get_function_return_type(
        &self,
        nome_funcao: &str,
        namespace_atual: &str,
    ) -> Option<Tipo> {
        let fqn = self.resolver_nome_funcao(nome_funcao, namespace_atual);
        if let Some(Declaracao::DeclaracaoFuncao(func_decl)) = self.simbolos_namespaces.get(&fqn) {
            func_decl.tipo_retorno.clone()
        } else {
            None
        }
    }

    pub fn get_variable_type(&self, name: &str, namespace_atual: &str) -> Option<Tipo> {
        println!(
            "DEBUG: get_variable_type: name='{}', namespace_atual='{}'",
            name, namespace_atual
        );
        // Esta é uma implementação simplificada. Em um cenário real, você precisaria
        // de uma tabela de símbolos mais robusta que rastreie os escopos.
        // Por enquanto, vamos apenas verificar os símbolos globais.
        let fqn = self.resolver_nome_funcao(name, namespace_atual);
        if let Some(Declaracao::DeclaracaoFuncao(func_decl)) = self.simbolos_namespaces.get(&fqn) {
            return func_decl.tipo_retorno.clone();
        }

        let fqn_class = self.resolver_nome_classe(name, namespace_atual);
        if self.classes.contains_key(&fqn_class) {
            return Some(Tipo::Classe(fqn_class));
        }

        None
    }

    fn get_declaracao_nome(&self, declaracao: &Declaracao) -> String {
        match declaracao {
            Declaracao::DeclaracaoFuncao(f) => f.nome.clone(),
            Declaracao::DeclaracaoClasse(c) => c.nome.clone(),
            Declaracao::DeclaracaoInterface(i) => i.nome.clone(),
            Declaracao::DeclaracaoEnum(e) => e.nome.clone(),
            _ => "".to_string(),
        }
    }

    pub fn resolver_nome_enum(&self, nome: &str, namespace_atual: &str) -> String {
        if nome.contains('.') {
            return nome.to_string();
        }
        if !namespace_atual.is_empty() {
            let fqn = format!("{}.{}", namespace_atual, nome);
            if self.enums.contains_key(&fqn) {
                return fqn;
            }
        }
        for using_path in &self.usings {
            let fqn = format!("{}.{}", using_path, nome);
            if self.enums.contains_key(&fqn) {
                return fqn;
            }
        }
        if self.enums.contains_key(nome) {
            return nome.to_string();
        }
        nome.to_string()
    }

    fn verificar_declaracao(
        &mut self,
        declaracao: &'a Declaracao,
        namespace_atual: &str,
        escopo_vars: &mut HashMap<String, Tipo>,
    ) {
        println!(
            "DEBUG: Verificando declaração em namespace \"{}\". Escopo inicial: {:?}",
            namespace_atual, escopo_vars
        );
        match declaracao {
            Declaracao::DeclaracaoClasse(classe) => {
                let fqn = if namespace_atual.is_empty() {
                    classe.nome.clone()
                } else {
                    format!("{}.{}", namespace_atual, classe.nome)
                };
                println!(
                    "DEBUG: Verificando classe \"{}\". FQN: \"{}\"",
                    classe.nome, fqn
                );
                // Regras de abstracao
                // 1) Nao pode haver metodo abstrato em classe nao-abstrata
                for m in &classe.metodos {
                    if m.eh_abstrato && !classe.eh_abstrata {
                        self.erros.push(format!(
                            "Método abstrato '{}' em classe não abstrata '{}'",
                            m.nome, fqn
                        ));
                    }
                    // 2) método abstrato não pode ter corpo
                    if m.eh_abstrato && !m.corpo.is_empty() {
                        self.erros.push(format!(
                            "Método abstrato '{}' não pode ter corpo em '{}'",
                            m.nome, fqn
                        ));
                    }
                    // 3) método abstrato não pode ser estático
                    if m.eh_abstrato && m.eh_estatica {
                        self.erros.push(format!(
                            "Método abstrato '{}' não pode ser estático em '{}'",
                            m.nome, fqn
                        ));
                    }
                }
                // 4) Classe estática não pode ser abstrata (como em C#)
                if classe.eh_abstrata && classe.eh_estatica {
                    self.erros.push(format!(
                        "Classe '{}' não pode ser 'abstrata' e 'estática' ao mesmo tempo",
                        fqn
                    ));
                }
                for metodo in &classe.metodos {
                    let mut metodo_vars = escopo_vars.clone();
                    // Validação de override/virtual
                    if let Some(parent_simple) = &classe.classe_pai {
                        let parent_fqn = self.resolver_nome_classe(parent_simple, namespace_atual);
                        if metodo.eh_override {
                            if let Some(base_m) = self
                                .encontrar_metodo_na_base(Some(parent_fqn.clone()), &metodo.nome)
                            {
                                if !base_m.eh_virtual {
                                    self.erros.push(format!(
                                        "Método '{}' em '{}' usa 'sobrescreve' mas o método da classe base não é 'redefinível'. Dica: marque o método da base como 'redefinível'.",
                                        metodo.nome, fqn
                                    ));
                                } else {
                                    let (ret_c, params_c) = self.assinatura_metodo(metodo);
                                    let (ret_b, params_b) = self.assinatura_metodo(base_m);
                                    if ret_c != ret_b || params_c != params_b {
                                        self.erros.push(format!(
                                            "Assinatura incompatível no override de '{}.{}'. Dica: a assinatura deve ser exatamente a mesma da base (retorno e parâmetros).",
                                            fqn, metodo.nome
                                        ));
                                    }
                                }
                            } else {
                                self.erros.push(format!(
                                    "Método '{}' marcado como 'sobrescreve' mas não existe método correspondente na classe base de '{}'. Dica: verifique nome, parâmetros e se o método da base está visível.",
                                    metodo.nome, fqn
                                ));
                            }
                        }
                    }
                    for param in &metodo.parametros {
                        let resolved_param_type = match &param.tipo {
                            Tipo::Classe(nome_classe) => {
                                let fqn_cls =
                                    self.resolver_nome_classe(nome_classe, namespace_atual);
                                if self.classes.contains_key(&fqn_cls) {
                                    Tipo::Classe(fqn_cls)
                                } else {
                                    let fqn_en =
                                        self.resolver_nome_enum(nome_classe, namespace_atual);
                                    if self.enums.contains_key(&fqn_en) {
                                        Tipo::Enum(fqn_en)
                                    } else {
                                        param.tipo.clone()
                                    }
                                }
                            }
                            _ => param.tipo.clone(),
                        };
                        metodo_vars.insert(param.nome.clone(), resolved_param_type);
                    }
                    println!(
                        "DEBUG: Verificando método \"{}\". Parâmetros no escopo: {:?}",
                        metodo.nome, metodo_vars
                    );
                    if !metodo.eh_abstrato {
                        for comando in &metodo.corpo {
                            self.verificar_comando(
                                comando,
                                namespace_atual,
                                Some(&fqn),
                                &mut metodo_vars,
                            );
                        }
                    }
                }
            }
            Declaracao::DeclaracaoFuncao(funcao) => {
                println!("DEBUG: Verificando função \"{}\"", funcao.nome);
                let mut func_vars = escopo_vars.clone();
                for param in &funcao.parametros {
                    func_vars.insert(param.nome.clone(), param.tipo.clone());
                }
                println!(
                    "DEBUG: Verificando função \"{}\". Parâmetros no escopo: {:?}",
                    funcao.nome, func_vars
                );
                for comando in &funcao.corpo {
                    self.verificar_comando(comando, namespace_atual, None, &mut func_vars);
                }
            }
            Declaracao::Comando(cmd) => {
                println!("DEBUG: Verificando comando global: {:?}", cmd);
                self.verificar_comando(cmd, namespace_atual, None, escopo_vars);
            }
            _ => {}
        }
    }

    fn verificar_comando(
        &mut self,
        comando: &Comando,
        namespace_atual: &str,
        classe_atual: Option<&String>,
        escopo_vars: &mut HashMap<String, Tipo>,
    ) {
        println!(
            "DEBUG: Verificando comando: {:?}. Escopo atual: {:?}",
            comando, escopo_vars
        );
        match comando {
            Comando::DeclaracaoVariavel(tipo, nome, expr) => {
                println!(
                    "DEBUG: DeclaracaoVariavel: nome=\"{}\", tipo={:?}",
                    nome, tipo
                );
                let tipo_resolvido = match tipo {
                    Tipo::Classe(nome_classe) => {
                        let fqn_cls = self.resolver_nome_classe(nome_classe, namespace_atual);
                        if self.classes.contains_key(&fqn_cls) {
                            Tipo::Classe(fqn_cls)
                        } else {
                            let fqn_en = self.resolver_nome_enum(nome_classe, namespace_atual);
                            if self.enums.contains_key(&fqn_en) {
                                Tipo::Enum(fqn_en)
                            } else {
                                tipo.clone()
                            }
                        }
                    }
                    _ => tipo.clone(),
                };
                println!(
                    "DEBUG: tipo_resolvido after resolution: {:?}",
                    tipo_resolvido
                );
                if let Some(e) = expr {
                    let tipo_expr =
                        self.inferir_tipo_expressao(e, namespace_atual, classe_atual, escopo_vars);
                    println!(
                        "DEBUG: Tipo inferido para expressão de inicialização: {:?}",
                        tipo_expr
                    );
                    if tipo_expr != Tipo::Inferido
                        && !self.tipos_compativeis_atribuicao(&tipo_resolvido, &tipo_expr)
                    {
                        self.erros.push(format!(
                            "Tipo da expressão ({:?}) não corresponde ao tipo da variável \"{}\" ({:?}).",
                            tipo_expr, nome, tipo_resolvido
                        ));
                    }
                }
                escopo_vars.insert(nome.clone(), tipo_resolvido.clone());
                println!(
                    "DEBUG: Variável \"{}\" adicionada ao escopo com tipo {:?}. Escopo atual: {:?}",
                    nome, tipo_resolvido, escopo_vars
                );
            }
            Comando::AtribuirIndice(alvo, idx, valor) => {
                let t_alvo =
                    self.inferir_tipo_expressao(alvo, namespace_atual, classe_atual, escopo_vars);
                let t_idx =
                    self.inferir_tipo_expressao(idx, namespace_atual, classe_atual, escopo_vars);
                if t_idx != Tipo::Inteiro {
                    self.erros.push("Índice de array deve ser inteiro".into());
                }
                if let Tipo::Lista(elem) = t_alvo {
                    let t_val = self.inferir_tipo_expressao(
                        valor,
                        namespace_atual,
                        classe_atual,
                        escopo_vars,
                    );
                    if !self.tipos_compativeis_atribuicao(&elem, &t_val) {
                        self.erros.push(format!(
                            "Atribuição de elemento incompatível: esperado {:?}, recebido {:?}",
                            elem, t_val
                        ));
                    }
                } else {
                    self.erros
                        .push("Atribuição por índice requer alvo do tipo lista".into());
                }
            }
            Comando::AtribuirPropriedade(obj_expr, prop_nome, val_expr) => {
                println!(
                    "DEBUG: AtribuirPropriedade: objeto_expr={:?}, prop_nome=\"{}\", val_expr={:?}",
                    obj_expr, prop_nome, val_expr
                );
                let obj_tipo = self.inferir_tipo_expressao(
                    obj_expr,
                    namespace_atual,
                    classe_atual,
                    escopo_vars,
                );
                println!(
                    "DEBUG: Tipo do objeto para atribuição de propriedade: {:?}",
                    obj_tipo
                );
                if let Tipo::Classe(nome_classe) = obj_tipo {
                    if let Some(class_info) = self.resolved_classes.get(&nome_classe) {
                        let prop_tipo = class_info
                            .properties
                            .iter()
                            .find(|p| p.nome == *prop_nome)
                            .map(|p| p.tipo.clone())
                            .or_else(|| {
                                class_info
                                    .fields
                                    .iter()
                                    .find(|f| f.nome == *prop_nome)
                                    .map(|f| f.tipo.clone())
                            });

                        if let Some(p_tipo) = prop_tipo {
                            let val_tipo = self.inferir_tipo_expressao(
                                val_expr,
                                namespace_atual,
                                classe_atual,
                                escopo_vars,
                            );
                            println!(
                                "DEBUG: Tipo da propriedade \"{}\": {:?}. Tipo do valor: {:?}",
                                prop_nome, p_tipo, val_tipo
                            );
                            if val_tipo != Tipo::Inferido
                                && !self.tipos_compativeis_atribuicao(&p_tipo, &val_tipo)
                            {
                                self.erros.push(format!(
                                    "Atribuição de tipo inválido para propriedade \"{}\". Esperado {:?}, recebido {:?}.",
                                    prop_nome, p_tipo, val_tipo
                                ));
                            }
                        } else {
                            self.erros.push(format!(
                                "Propriedade \"{}\" não encontrada na classe \"{}\".",
                                prop_nome, nome_classe
                            ));
                        }
                    } else {
                        self.erros.push(format!(
                            "Classe \"{}\" não encontrada para atribuição de propriedade.",
                            nome_classe
                        ));
                    }
                } else {
                    self.erros
                        .push("Atribuição de propriedade em algo que não é um objeto.".to_string());
                }
            }
            Comando::Bloco(comandos) => {
                println!("DEBUG: Verificando Bloco de comandos.");
                let mut bloco_vars = escopo_vars.clone();
                for cmd in comandos {
                    self.verificar_comando(cmd, namespace_atual, classe_atual, &mut bloco_vars);
                }
            }
            Comando::DeclaracaoVar(nome, expr) => {
                let tipo_expr =
                    self.inferir_tipo_expressao(expr, namespace_atual, classe_atual, escopo_vars);
                escopo_vars.insert(nome.clone(), tipo_expr);
            }
            Comando::Imprima(expr) => {
                self.inferir_tipo_expressao(expr, namespace_atual, classe_atual, escopo_vars);
            }
            Comando::Retorne(expr) => {
                if let Some(e) = expr {
                    self.inferir_tipo_expressao(e, namespace_atual, classe_atual, escopo_vars);
                }
            }
            Comando::Se(cond, corpo, senao) => {
                self.inferir_tipo_expressao(cond, namespace_atual, classe_atual, escopo_vars);
                self.verificar_comando(corpo, namespace_atual, classe_atual, escopo_vars);
                if let Some(s) = senao {
                    self.verificar_comando(s, namespace_atual, classe_atual, escopo_vars);
                }
            }
            Comando::Enquanto(cond, corpo) => {
                self.inferir_tipo_expressao(cond, namespace_atual, classe_atual, escopo_vars);
                self.verificar_comando(corpo, namespace_atual, classe_atual, escopo_vars);
            }
            Comando::Expressao(expr) => {
                self.inferir_tipo_expressao(expr, namespace_atual, classe_atual, escopo_vars);
            }
            Comando::Atribuicao(nome, expr) => {
                if let Some(class_name) = classe_atual {
                    if let Some(class_info) = self.resolved_classes.get(class_name) {
                        if class_info.properties.iter().any(|p| p.nome == *nome)
                            || class_info.fields.iter().any(|f| f.nome == *nome)
                        {
                            self.verificar_comando(
                                &Comando::AtribuirPropriedade(
                                    Box::new(Expressao::Este),
                                    nome.clone(),
                                    expr.clone(),
                                ),
                                namespace_atual,
                                classe_atual,
                                escopo_vars,
                            );
                            return;
                        }
                    }
                }
                let tipo_expr =
                    self.inferir_tipo_expressao(expr, namespace_atual, classe_atual, escopo_vars);
                if let Some(tipo_var) = escopo_vars.get(nome) {
                    if tipo_expr != Tipo::Inferido
                        && !self.tipos_compativeis_atribuicao(tipo_var, &tipo_expr)
                    {
                        self.erros.push(format!(
                            "Atribuição de tipo inválido para variável \"{}\". Esperado {:?}, recebido {:?}.",
                            nome, tipo_var, tipo_expr
                        ));
                    }
                } else {
                    self.erros
                        .push(format!("Variável \"{}\" não declarada.", nome));
                }
            }
            Comando::ChamarMetodo(obj_expr, _, args) => {
                self.inferir_tipo_expressao(obj_expr, namespace_atual, classe_atual, escopo_vars);
                for arg in args {
                    self.inferir_tipo_expressao(arg, namespace_atual, classe_atual, escopo_vars);
                }
            }
            Comando::AcessarCampo(obj, _campo) => {
                let _obj_tipo = self.inferir_tipo_expressao(
                    &Expressao::Identificador(obj.clone()),
                    namespace_atual,
                    classe_atual,
                    escopo_vars,
                );
            }
            _ => {
                println!("DEBUG: Comando não tratado: {:?}", comando);
            }
        }
    }

    pub fn inferir_tipo_expressao(
        &mut self,
        expressao: &Expressao,
        namespace_atual: &str,
        classe_atual: Option<&String>,
        escopo_vars: &HashMap<String, Tipo>,
    ) -> Tipo {
        match expressao {
            Expressao::Inteiro(_) => Tipo::Inteiro,
            Expressao::Texto(_) => Tipo::Texto,
            Expressao::Booleano(_) => Tipo::Booleano,
            Expressao::FlutuanteLiteral(_) => Tipo::Flutuante,
            Expressao::DuploLiteral(_) => Tipo::Duplo,
            Expressao::Decimal(_) => Tipo::Decimal,
            Expressao::Este => {
                classe_atual.map_or(Tipo::Inferido, |nome| Tipo::Classe(nome.clone()))
            }
            Expressao::Identificador(nome) => {
                if escopo_vars.contains_key(nome) {
                    return escopo_vars.get(nome).unwrap().clone();
                }
                if let Some(class_name) = classe_atual {
                    if let Some(class_info) = self.resolved_classes.get(class_name) {
                        if class_info.properties.iter().any(|p| p.nome == *nome)
                            || class_info.fields.iter().any(|f| f.nome == *nome)
                        {
                            return self.inferir_tipo_expressao(
                                &Expressao::AcessoMembro(Box::new(Expressao::Este), nome.clone()),
                                namespace_atual,
                                classe_atual,
                                escopo_vars,
                            );
                        }
                    }
                }
                // Classe?
                let fqn_class = self.resolver_nome_classe(nome, namespace_atual);
                if self.classes.contains_key(&fqn_class) {
                    return Tipo::Classe(fqn_class);
                }
                // Enum?
                let fqn_enum = self.resolver_nome_enum(nome, namespace_atual);
                if self.enums.contains_key(&fqn_enum) {
                    return Tipo::Enum(fqn_enum);
                }
                self.erros
                    .push(format!("Identificador \"{}\" não encontrado.", nome));
                Tipo::Inferido
            }
            Expressao::AcessoMembro(obj_expr, membro_nome) => {
                let obj_tipo = self.inferir_tipo_expressao(
                    obj_expr,
                    namespace_atual,
                    classe_atual,
                    escopo_vars,
                );
                if let Tipo::Classe(ref nome_classe) = obj_tipo {
                    if let Some(class_info) = self.resolved_classes.get(nome_classe) {
                        if let Some(prop) = class_info
                            .properties
                            .iter()
                            .find(|p| p.nome == *membro_nome)
                        {
                            return prop.tipo.clone();
                        }
                        if let Some(field) =
                            class_info.fields.iter().find(|f| f.nome == *membro_nome)
                        {
                            return field.tipo.clone();
                        }
                    }
                }
                // Propriedade especial de arrays e textos
                if membro_nome == "tamanho" {
                    if matches!(obj_tipo, Tipo::Lista(_) | Tipo::Texto) {
                        return Tipo::Inteiro;
                    }
                }
                // Enum membro? O membro possui o tipo do próprio enum
                if let Tipo::Enum(ref fqn_enum) = obj_tipo {
                    if let Some(en) = self.enums.get(fqn_enum) {
                        if en.valores.iter().any(|v| v == membro_nome) {
                            return Tipo::Enum(fqn_enum.clone());
                        } else {
                            self.erros.push(format!(
                                "Membro \"{}\" não existe no enum \"{}\".",
                                membro_nome, fqn_enum
                            ));
                        }
                    } else {
                        self.erros.push(format!(
                            "Enum \"{}\" não encontrado ao acessar membro \"{}\".",
                            fqn_enum, membro_nome
                        ));
                    }
                }
                Tipo::Inferido
            }
            Expressao::ListaLiteral(items) => {
                // Homogeneidade: infere tipo comum, por ora exige todos iguais
                if items.is_empty() {
                    return Tipo::Lista(Box::new(Tipo::Inferido));
                }
                let first = self.inferir_tipo_expressao(
                    &items[0],
                    namespace_atual,
                    classe_atual,
                    escopo_vars,
                );
                for e in &items[1..] {
                    let te =
                        self.inferir_tipo_expressao(e, namespace_atual, classe_atual, escopo_vars);
                    if !self.tipos_compativeis_atribuicao(&first, &te)
                        || !self.tipos_compativeis_atribuicao(&te, &first)
                    {
                        self.erros
                            .push("Elementos do array devem ter tipos compatíveis".into());
                        break;
                    }
                }
                return Tipo::Lista(Box::new(first));
            }
            Expressao::AcessoIndice(obj, idx) => {
                let t_obj =
                    self.inferir_tipo_expressao(obj, namespace_atual, classe_atual, escopo_vars);
                let t_idx =
                    self.inferir_tipo_expressao(idx, namespace_atual, classe_atual, escopo_vars);
                if t_idx != Tipo::Inteiro {
                    self.erros.push("Índice de acesso deve ser inteiro".into());
                }
                if let Tipo::Lista(elem) = t_obj {
                    return *elem;
                }
                self.erros.push("Acesso por índice requer lista".into());
                Tipo::Inferido
            }
            Expressao::NovoObjeto(nome_classe, _) => {
                Tipo::Classe(self.resolver_nome_classe(nome_classe, namespace_atual))
            }
            Expressao::Aritmetica(_, esq, dir) => {
                let te =
                    self.inferir_tipo_expressao(esq, namespace_atual, classe_atual, escopo_vars);
                let td =
                    self.inferir_tipo_expressao(dir, namespace_atual, classe_atual, escopo_vars);
                // Promoção numérica simples: Duplo > Flutuante > Inteiro; Decimal tratado a parte
                use Tipo::*;
                match (te, td) {
                    (Decimal, _) | (_, Decimal) => Decimal,
                    (Duplo, _) | (_, Duplo) => Duplo,
                    (Flutuante, _) | (_, Flutuante) => Flutuante,
                    (Inteiro, Inteiro) => Inteiro,
                    _ => Inteiro,
                }
            }
            Expressao::Comparacao(_, _, _) => Tipo::Booleano,
            Expressao::Logica(_, _, _) => Tipo::Booleano,
            _ => Tipo::Inferido,
        }
    }

    pub fn get_expr_type(
        &self,
        expressao: &Expressao,
        namespace_atual: &str,
        classe_atual: Option<&String>,
        escopo_vars: &HashMap<String, Tipo>,
    ) -> Tipo {
        match expressao {
            Expressao::Inteiro(_) => Tipo::Inteiro,
            Expressao::Texto(_) => Tipo::Texto,
            Expressao::Booleano(_) => Tipo::Booleano,
            Expressao::Decimal(_) => Tipo::Decimal,
            Expressao::Este => {
                classe_atual.map_or(Tipo::Inferido, |nome| Tipo::Classe(nome.clone()))
            }
            Expressao::Identificador(nome) => {
                if escopo_vars.contains_key(nome) {
                    return escopo_vars.get(nome).unwrap().clone();
                }
                if let Some(class_name) = classe_atual {
                    if let Some(class_info) = self.resolved_classes.get(class_name) {
                        if class_info.properties.iter().any(|p| p.nome == *nome)
                            || class_info.fields.iter().any(|f| f.nome == *nome)
                        {
                            return self.get_expr_type(
                                &Expressao::AcessoMembro(Box::new(Expressao::Este), nome.clone()),
                                namespace_atual,
                                classe_atual,
                                escopo_vars,
                            );
                        }
                    }
                }
                let fqn_class = self.resolver_nome_classe(nome, namespace_atual);
                if self.classes.contains_key(&fqn_class) {
                    return Tipo::Classe(fqn_class);
                }
                let fqn_enum = self.resolver_nome_enum(nome, namespace_atual);
                if self.enums.contains_key(&fqn_enum) {
                    return Tipo::Enum(fqn_enum);
                }
                Tipo::Inferido
            }
            Expressao::AcessoMembro(obj_expr, membro_nome) => {
                let obj_tipo =
                    self.get_expr_type(obj_expr, namespace_atual, classe_atual, escopo_vars);
                if let Tipo::Classe(ref nome_classe) = obj_tipo {
                    if let Some(class_info) = self.resolved_classes.get(nome_classe) {
                        if let Some(prop) = class_info
                            .properties
                            .iter()
                            .find(|p| p.nome == *membro_nome)
                        {
                            return prop.tipo.clone();
                        }
                        if let Some(field) =
                            class_info.fields.iter().find(|f| f.nome == *membro_nome)
                        {
                            return field.tipo.clone();
                        }
                    }
                }
                if membro_nome == "tamanho" {
                    if matches!(obj_tipo, Tipo::Lista(_) | Tipo::Texto) {
                        return Tipo::Inteiro;
                    }
                }
                if let Tipo::Enum(ref fqn_enum) = obj_tipo {
                    if let Some(en) = self.enums.get(fqn_enum) {
                        if en.valores.iter().any(|v| v == membro_nome) {
                            return Tipo::Enum(fqn_enum.clone());
                        }
                    }
                }
                Tipo::Inferido
            }
            Expressao::ListaLiteral(items) => {
                if items.is_empty() {
                    return Tipo::Lista(Box::new(Tipo::Inferido));
                }
                let first =
                    self.get_expr_type(&items[0], namespace_atual, classe_atual, escopo_vars);
                return Tipo::Lista(Box::new(first));
            }
            Expressao::AcessoIndice(obj, _idx) => {
                let t_obj = self.get_expr_type(obj, namespace_atual, classe_atual, escopo_vars);
                if let Tipo::Lista(elem) = t_obj {
                    return *elem;
                }
                Tipo::Inferido
            }
            Expressao::NovoObjeto(nome_classe, _) => {
                Tipo::Classe(self.resolver_nome_classe(nome_classe, namespace_atual))
            }
            Expressao::Aritmetica(_, esq, dir) => {
                let te = self.get_expr_type(esq, namespace_atual, classe_atual, escopo_vars);
                let td = self.get_expr_type(dir, namespace_atual, classe_atual, escopo_vars);
                use Tipo::*;
                match (te, td) {
                    (Decimal, _) | (_, Decimal) => Decimal,
                    (Duplo, _) | (_, Duplo) => Duplo,
                    (Flutuante, _) | (_, Flutuante) => Flutuante,
                    (Inteiro, Inteiro) => Inteiro,
                    _ => Inteiro,
                }
            }
            Expressao::Comparacao(_, _, _) => Tipo::Booleano,
            Expressao::Logica(_, _, _) => Tipo::Booleano,
            _ => Tipo::Inferido,
        }
    }
}
