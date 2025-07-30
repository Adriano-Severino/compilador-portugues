use crate::ast;
use crate::ast::*;
use std::collections::HashMap;

pub struct VerificadorTipos<'a> {
    usings: Vec<String>,
    simbolos_namespaces: HashMap<String, &'a Declaracao>,
    pub classes: HashMap<String, &'a DeclaracaoClasse>,
    resolved_classes: HashMap<String, ResolvedClassInfo<'a>>,
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
}

impl<'a> VerificadorTipos<'a> {
    pub fn new() -> Self {
        Self {
            usings: Vec::new(),
            simbolos_namespaces: HashMap::new(),
            classes: HashMap::new(),
            resolved_classes: HashMap::new(),
            erros: Vec::new(),
        }
    }

    pub fn verificar_programa(&mut self, programa: &'a Programa) -> Result<(), Vec<String>> {
        // 1. usings
        self.usings = programa.usings.iter().map(|u| u.caminho.clone()).collect();

        // 2. primeira passagem: só registra classes e functions
        for decl in &programa.declaracoes {
            let nome = self.get_declaracao_nome(decl);
            if let Declaracao::DeclaracaoClasse(cl) = decl {
                self.classes.insert(nome.clone(), cl);
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

        if self.erros.is_empty() {
            Ok(())
        } else {
            Err(self.erros.clone())
        }
    }

    fn resolve_class_hierarchy(&mut self, class_name: &str, class_decl: &'a DeclaracaoClasse) {
        if self.resolved_classes.contains_key(class_name) {
            return;
        }

        let mut properties: Vec<&'a ast::PropriedadeClasse> =
            class_decl.propriedades.iter().collect();
        let mut fields: Vec<&'a ast::CampoClasse> = class_decl.campos.iter().collect();
        let mut methods: HashMap<String, &'a ast::MetodoClasse> = class_decl
            .metodos
            .iter()
            .map(|m| (m.nome.clone(), m))
            .collect();

        if let Some(parent_name_simple) = &class_decl.classe_pai {
            let parent_name = self.resolver_nome_classe(
                parent_name_simple,
                &self.get_namespace_from_full_name(class_name),
            );
            if let Some(parent_decl) = self.classes.get(&parent_name).copied() {
                self.resolve_class_hierarchy(&parent_name, parent_decl);
                if let Some(parent_info) = self.resolved_classes.get(&parent_name) {
                    let new_properties: Vec<_> = parent_info
                        .properties
                        .iter()
                        .filter(|p| !properties.iter().any(|ep| ep.nome == p.nome))
                        .cloned()
                        .collect();
                    properties.extend(new_properties);

                    let new_fields: Vec<_> = parent_info
                        .fields
                        .iter()
                        .filter(|f| !fields.iter().any(|ef| ef.nome == f.nome))
                        .cloned()
                        .collect();
                    fields.extend(new_fields);

                    for (name, method) in &parent_info.methods {
                        methods.entry(name.clone()).or_insert(method);
                    }
                }
            }
        }

        self.resolved_classes.insert(
            class_name.to_string(),
            ResolvedClassInfo {
                name: class_name.to_string(),
                parent_name: class_decl.classe_pai.clone(),
                properties,
                fields,
                methods,
                eh_estatica: class_decl.eh_estatica,
            },
        );
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

    fn get_declaracao_nome(&self, declaracao: &Declaracao) -> String {
        match declaracao {
            Declaracao::DeclaracaoFuncao(f) => f.nome.clone(),
            Declaracao::DeclaracaoClasse(c) => c.nome.clone(),
            _ => "".to_string(),
        }
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
                for metodo in &classe.metodos {
                    let mut metodo_vars = escopo_vars.clone();
                    for param in &metodo.parametros {
                        let resolved_param_type = match &param.tipo {
                            Tipo::Classe(nome_classe) => Tipo::Classe(
                                self.resolver_nome_classe(nome_classe, namespace_atual),
                            ),
                            _ => param.tipo.clone(),
                        };
                        metodo_vars.insert(param.nome.clone(), resolved_param_type);
                    }
                    println!(
                        "DEBUG: Verificando método \"{}\". Parâmetros no escopo: {:?}",
                        metodo.nome, metodo_vars
                    );
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
                        Tipo::Classe(self.resolver_nome_classe(nome_classe, namespace_atual))
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
                    if tipo_resolvido != tipo_expr && tipo_expr != Tipo::Inferido {
                        if !(tipo_resolvido == Tipo::Texto && tipo_expr == Tipo::Inteiro) {
                            self.erros.push(format!("Tipo da expressão ({:?}) não corresponde ao tipo da variável \"{}\" ({:?}).", tipo_expr, nome, tipo_resolvido));
                        }
                    }
                }
                escopo_vars.insert(nome.clone(), tipo_resolvido.clone());
                println!(
                    "DEBUG: Variável \"{}\" adicionada ao escopo com tipo {:?}. Escopo atual: {:?}",
                    nome, tipo_resolvido, escopo_vars
                );
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
                            if p_tipo != val_tipo && val_tipo != Tipo::Inferido {
                                if !((p_tipo == Tipo::Texto
                                    && (val_tipo == Tipo::Inteiro
                                        || val_tipo == Tipo::Booleano)))
                                {
                                    self.erros.push(format!("Atribuição de tipo inválido para propriedade \"{}\". Esperado {:?}, recebido {:?}.", prop_nome, p_tipo, val_tipo));
                                }
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
                        if class_info.properties.iter().any(|p| p.nome == *nome) || class_info.fields.iter().any(|f| f.nome == *nome) {
                            self.verificar_comando(&Comando::AtribuirPropriedade(Box::new(Expressao::Este), nome.clone(), expr.clone()), namespace_atual, classe_atual, escopo_vars);
                            return;
                        }
                    }
                }
                let tipo_expr =
                    self.inferir_tipo_expressao(expr, namespace_atual, classe_atual, escopo_vars);
                if let Some(tipo_var) = escopo_vars.get(nome) {
                    if *tipo_var != tipo_expr && tipo_expr != Tipo::Inferido {
                        self.erros.push(format!(
                            "Atribuição de tipo inválido para variável \"{}\". Esperado {:?}, recebido {:?}.",
                            nome,
                            tipo_var,
                            tipo_expr
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

    fn inferir_tipo_expressao(
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
            Expressao::Este => {
                classe_atual.map_or(Tipo::Inferido, |nome| Tipo::Classe(nome.clone()))
            }
            Expressao::Identificador(nome) => {
                if let Some(tipo) = escopo_vars.get(nome) {
                    return tipo.clone();
                }
                if let Some(class_name) = classe_atual {
                    if let Some(class_info) = self.resolved_classes.get(class_name) {
                        if let Some(prop) = class_info.properties.iter().find(|p| p.nome == *nome) {
                            return prop.tipo.clone();
                        }
                        if let Some(field) = class_info.fields.iter().find(|f| f.nome == *nome) {
                            return field.tipo.clone();
                        }
                    }
                }
                let fqn = self.resolver_nome_classe(nome, namespace_atual);
                if self.classes.contains_key(&fqn) {
                    return Tipo::Classe(fqn.clone());
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
                if let Tipo::Classe(nome_classe) = obj_tipo {
                    if let Some(class_info) = self.resolved_classes.get(&nome_classe) {
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
                Tipo::Inferido
            }
            Expressao::NovoObjeto(nome_classe, _) => {
                Tipo::Classe(self.resolver_nome_classe(nome_classe, namespace_atual))
            }
            Expressao::Aritmetica(_, esq, dir) => {
                let _te =
                    self.inferir_tipo_expressao(esq, namespace_atual, classe_atual, escopo_vars);
                let _td =
                    self.inferir_tipo_expressao(dir, namespace_atual, classe_atual, escopo_vars);
                Tipo::Inteiro
            }
            Expressao::Comparacao(_, _, _) => Tipo::Booleano,
            Expressao::Logica(_, _, _) => Tipo::Booleano,
            _ => Tipo::Inferido,
        }
    }
}