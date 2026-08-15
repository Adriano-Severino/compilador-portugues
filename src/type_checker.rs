use crate::ast;
use crate::ast::*;
use crate::library_loader::{self, Biblioteca, LibSimbolo};
use std::collections::HashMap;

// Helper function to convert string to ast::Tipo
pub fn string_para_tipo(s: &str) -> ast::Tipo {
    match s {
        "inteiro" => ast::Tipo::Inteiro,
        "texto" => ast::Tipo::Texto,
        "booleano" => ast::Tipo::Booleano,
        "flutuante" => ast::Tipo::Flutuante,
        "duplo" => ast::Tipo::Duplo,
        "decimal" => ast::Tipo::Decimal,
        "vazio" => ast::Tipo::Vazio,
        "objeto" => ast::Tipo::Objeto,
        _ => ast::Tipo::Classe(s.to_string()),
    }
}

#[derive(Clone)]
pub struct VerificadorTipos<'a> {
    usings: Vec<String>,
    simbolos_namespaces: HashMap<String, &'a Declaracao>,
    pub classes: HashMap<String, &'a DeclaracaoClasse>,
    pub interfaces: HashMap<String, &'a ast::DeclaracaoInterface>,
    pub enums: HashMap<String, &'a DeclaracaoEnum>,
    pub resolved_classes: HashMap<String, ResolvedClassInfo<'a>>,
    erros: Vec<String>,
    // Biblioteca externa carregada (metadados .pbl sem conversão para AST)
    pub biblioteca_externa: Option<library_loader::Biblioteca>,
    // Storage for library-loaded AST nodes (mantido para compatibilidade com tipo 'objeto')
    loaded_lib_declarations: Vec<Declaracao>,
    pub generic_scope: Vec<std::collections::HashSet<String>>,
    stdlib_namespaces: std::collections::HashSet<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedClassInfo<'a> {
    pub name: String,
    pub parent_name: Option<String>,
    pub properties: Vec<&'a ast::PropriedadeClasse>,
    pub fields: Vec<&'a ast::CampoClasse>,
    pub methods: HashMap<String, &'a ast::MetodoClasse>,
    pub eh_estatica: bool,
    pub eh_abstrata: bool,
    pub interfaces: Vec<String>,
}

impl<'a> VerificadorTipos<'a> {
    pub fn new() -> Self {
        let mut vt = Self {
            usings: Vec::new(),
            simbolos_namespaces: HashMap::new(),
            classes: HashMap::new(),
            interfaces: HashMap::new(),
            enums: HashMap::new(),
            resolved_classes: HashMap::new(),
            erros: Vec::new(),
            biblioteca_externa: None,
            loaded_lib_declarations: Vec::new(),
            generic_scope: Vec::new(),
            stdlib_namespaces: std::collections::HashSet::new(),
        };
        vt.inicializar_tipos_integrados();
        vt
    }

    /// Define a biblioteca externa carregada (.pbl) para consulta de metadados.
    /// Esta biblioteca contém LibClasse e LibMetodo que serão usados diretamente
    /// para resolução de tipos, evitando conversão para AST completa (que causa stack overflow).
    pub fn definir_biblioteca_externa(&mut self, biblioteca: library_loader::Biblioteca) {
        self.biblioteca_externa = Some(biblioteca);
    }

    fn inicializar_tipos_integrados(&mut self) {
        let objeto_classe = DeclaracaoClasse {
            nome: "objeto".to_string(),
            modificador: ModificadorAcesso::Publico,
            eh_estatica: false,
            eh_abstrata: false,
            classe_pai: None,
            interfaces: Vec::new(),
            propriedades: Vec::new(),
            campos: Vec::new(),
            construtores: Vec::new(),
            nested_classes: Vec::new(),
            metodos: Vec::new(),
            generic_params: Vec::new(),
        };

        let decl = Declaracao::DeclaracaoClasse(objeto_classe);
        self.loaded_lib_declarations.push(decl);

        // Use the same unsafe pattern as in carregar_biblioteca to avoid borrow checker issues
        let decl_ptr: *const Declaracao =
            &self.loaded_lib_declarations[self.loaded_lib_declarations.len() - 1];
        let decl_ref = unsafe { &*decl_ptr };

        if let Declaracao::DeclaracaoClasse(cl) = decl_ref {
            let cl_ptr: *const DeclaracaoClasse = cl;
            let cl_ref = unsafe { &*cl_ptr };
            self.classes.insert("objeto".to_string(), cl_ref);
        }
        self.simbolos_namespaces
            .insert("objeto".to_string(), decl_ref);
    }

    pub fn registrar_namespace_stdlib(&mut self, ns: &str) {
        self.stdlib_namespaces.insert(ns.to_string());
    }

    pub fn eh_classe_stdlib(&self, fqn: &str) -> bool {
        let ns = self.get_namespace_from_full_name(fqn);
        self.stdlib_namespaces.contains(&ns)
    }

    // Método legado: Convertia LibClasse → DeclaracaoClasse (causava stack overflow)
    // Substituído por definir_biblioteca_externa que usa metadados diretamente
    /*
    pub fn carregar_biblioteca(&mut self, biblioteca: library_loader::Biblioteca) {
        let initial_len = self.loaded_lib_declarations.len();

        for (_nome_simbolo, simbolo) in biblioteca.simbolos {
            match simbolo {
                library_loader::LibSimbolo::Classe(lib_classe) => {
                    let metodos: Vec<MetodoClasse> = lib_classe
                        .metodos
                        .values()
                        .map(|lib_metodo| {
                            // Se o método tem chave nativa, adiciona o atributo [Nativo("chave")]
                            // para que o verificador de tipos e o gerador de bytecode o tratem corretamente.
                            let attributes = if let Some(chave) = &lib_metodo.chave_nativa {
                                vec![ast::Attribute {
                                    name: "Nativo".to_string(),
                                    arguments: vec![ast::Expressao::Texto(chave.clone())],
                                }]
                            } else {
                                vec![]
                            };
                            MetodoClasse {
                                attributes,
                                nome: lib_metodo.nome.clone(),
                                modificador: ModificadorAcesso::Publico,
                                eh_estatica: lib_metodo.eh_estatica,
                                eh_abstrato: false,
                                eh_virtual: false,
                                eh_override: false,
                                tipo_retorno: Some(string_para_tipo(&lib_metodo.tipo_retorno)),
                                parametros: lib_metodo
                                    .parametros
                                    .iter()
                                    .map(|(tipo, nome)| Parametro {
                                        nome: nome.clone(),
                                        tipo: string_para_tipo(tipo),
                                        valor_padrao: None,
                                    })
                                    .collect(),
                                corpo: Vec::new(),
                                generic_params: Vec::new(),
                            }
                        })
                        .collect();

                    let decl_classe = DeclaracaoClasse {
                        nome: lib_classe.fqn.clone(), // Use FQN
                        modificador: ModificadorAcesso::Publico,
                        eh_estatica: lib_classe.eh_estatica,
                        eh_abstrata: false,
                        classe_pai: lib_classe.nome_pai.map(|p| ast::Tipo::Classe(p)),
                        interfaces: Vec::new(),
                        propriedades: lib_classe
                            .propriedades
                            .iter()
                            .map(|p| PropriedadeClasse {
                                nome: p.nome.clone(),
                                tipo: string_para_tipo(&p.tipo),
                                modificador: ModificadorAcesso::Publico,
                                obter: None,
                                definir: None,
                                eh_estatica: lib_classe.eh_estatica,
                                valor_inicial: None,
                            })
                            .collect(),
                        campos: lib_classe
                            .campos
                            .iter()
                            .map(|c| CampoClasse {
                                nome: c.nome.clone(),
                                tipo: string_para_tipo(&c.tipo),
                                modificador: ModificadorAcesso::Privado,
                                eh_estatica: lib_classe.eh_estatica,
                                valor_inicial: None,
                            })
                            .collect(),
                        construtores: Vec::new(),
                        nested_classes: Vec::new(),
                        metodos,
                        generic_params: Vec::new(),
                    };

                    let decl = Declaracao::DeclaracaoClasse(decl_classe);
                    self.loaded_lib_declarations.push(decl);
                }
                library_loader::LibSimbolo::Funcao(_lib_funcao) => {
                    // TODO: Handle functions
                }
            }
        }

        // The `unsafe` block here is a controlled way to work around the borrow checker's limitations
        // with self-referential structs. We are adding references to `self.classes` and `self.simbolos_namespaces`
        // that point to data owned by `self.loaded_lib_declarations`.
        // This is safe because:
        // 1. The data in `loaded_lib_declarations` is guaranteed to live as long as `self`.
        // 2. We do not modify `loaded_lib_declarations` after these references are created,
        //    so the pointers will not be invalidated by a vector reallocation.
        for i in initial_len..self.loaded_lib_declarations.len() {
            let decl_ptr: *const Declaracao = &self.loaded_lib_declarations[i];
            let decl = unsafe { &*decl_ptr };
            let nome = self.get_declaracao_nome(decl);

            if let Declaracao::DeclaracaoClasse(cl) = decl {
                let cl_ptr: *const DeclaracaoClasse = cl;
                let cl_ref = unsafe { &*cl_ptr };
                self.classes.insert(nome.clone(), cl_ref);
            }
            self.simbolos_namespaces.insert(nome, decl);
        }
    }
    */

    // Substitui parâmetros genéricos por tipos concretos em um tipo arbitrário.
    fn substitute_generics_in_tipo(
        &self,
        t: &Tipo,
        subst: &std::collections::HashMap<String, Tipo>,
    ) -> Tipo {
        self.substitute_generics_in_tipo_recursive(t, subst, 0)
    }

    fn substitute_generics_in_tipo_recursive(
        &self,
        t: &Tipo,
        subst: &std::collections::HashMap<String, Tipo>,
        depth: usize,
    ) -> Tipo {
        if depth > 10 {
            return t.clone();
        }
        use Tipo::*;
        match t {
            Generico(nome) => subst.get(nome).cloned().unwrap_or_else(|| t.clone()),
            Classe(nome) => subst.get(nome).cloned().unwrap_or_else(|| t.clone()),
            Lista(inner) => Lista(Box::new(self.substitute_generics_in_tipo_recursive(
                inner,
                subst,
                depth + 1,
            ))),
            Opcional(inner) => Opcional(Box::new(self.substitute_generics_in_tipo_recursive(
                inner,
                subst,
                depth + 1,
            ))),
            Aplicado { nome, args } => {
                let novos_args = args
                    .iter()
                    .map(|a| self.substitute_generics_in_tipo_recursive(a, subst, depth + 1))
                    .collect();
                Aplicado {
                    nome: nome.clone(),
                    args: novos_args,
                }
            }
            Funcao(params, ret) => {
                let novos_params = params
                    .iter()
                    .map(|p| self.substitute_generics_in_tipo_recursive(p, subst, depth + 1))
                    .collect();
                let novo_ret = self.substitute_generics_in_tipo_recursive(ret, subst, depth + 1);
                Funcao(novos_params, Box::new(novo_ret))
            }
            _ => t.clone(),
        }
    }

    fn normalize_tipo_ro(&self, t: &Tipo, namespace_atual: &str) -> (Tipo, Vec<String>) {
        use Tipo::*;
        match t {
            Lista(inner) => {
                let (norm, errs) = self.normalize_tipo_ro(inner, namespace_atual);
                (Lista(Box::new(norm)), { errs })
            }
            Classe(n) => {
                // Check if it's a generic parameter
                for scope in self.generic_scope.iter().rev() {
                    if scope.contains(n) {
                        return (Generico(n.clone()), vec![]);
                    }
                }
                (
                    Classe(self.resolver_nome_classe(n, namespace_atual)),
                    vec![],
                )
            }
            Enum(n) => (Enum(self.resolver_nome_enum(n, namespace_atual)), vec![]),
            Aplicado { nome, args } => {
                // Check if it's a generic parameter
                for scope in self.generic_scope.iter().rev() {
                    if scope.contains(nome) {
                        return (Generico(nome.clone()), vec![]);
                    }
                }
                let fqn_cls = self.resolver_nome_classe(nome, namespace_atual);
                let fqn_iface = self.resolver_nome_interface(nome, namespace_atual);
                let (is_class, is_iface, resolved_name) = (
                    self.classes.contains_key(&fqn_cls),
                    self.interfaces.contains_key(&fqn_iface),
                    if self.classes.contains_key(&fqn_cls) {
                        fqn_cls.clone()
                    } else if self.interfaces.contains_key(&fqn_iface) {
                        fqn_iface.clone()
                    } else {
                        nome.clone()
                    },
                );
                let mut erros: Vec<String> = Vec::new();
                if is_class {
                    if let Some(decl) = self.classes.get(&fqn_cls) {
                        let expected = decl.generic_params.len();
                        if expected == 0 {
                            erros.push(format!(
                                "Tipo '{}' não é genérico, mas foi usado como '{}' com argumentos.",
                                fqn_cls, nome
                            ));
                        } else if expected != args.len() {
                            erros.push(format!(
                                "Aridade genérica incorreta para '{}': esperados {}, recebidos {}.",
                                fqn_cls,
                                expected,
                                args.len()
                            ));
                        }
                    }
                } else if is_iface {
                    if let Some(decl) = self.interfaces.get(&fqn_iface) {
                        let expected = decl.generic_params.len();
                        if expected == 0 {
                            erros.push(format!(
                                "Interface '{}' não é genérica, mas foi usada como '{}' com argumentos.",
                                fqn_iface, nome
                            ));
                        } else if expected != args.len() {
                            erros.push(format!(
                                "Aridade genérica incorreta para interface '{}': esperados {}, recebidos {}.",
                                fqn_iface, expected, args.len()
                            ));
                        }
                    }
                }

                let mut norm_args: Vec<Tipo> = Vec::new();
                for a in args.iter() {
                    let (na, mut e) = self.normalize_tipo_ro(a, namespace_atual);
                    norm_args.push(na);
                    erros.append(&mut e);
                }

                (
                    Aplicado {
                        nome: resolved_name,
                        args: norm_args,
                    },
                    erros,
                )
            }
            Funcao(params, ret) => {
                let mut erros = Vec::new();
                let mut norm_params: Vec<Tipo> = Vec::new();
                for p in params.iter() {
                    let (np, mut e) = self.normalize_tipo_ro(p, namespace_atual);
                    norm_params.push(np);
                    erros.append(&mut e);
                }
                let (nr, mut e2) = self.normalize_tipo_ro(ret, namespace_atual);
                erros.append(&mut e2);
                (Funcao(norm_params, Box::new(nr)), erros)
            }
            Opcional(inner) => {
                let (norm, errs) = self.normalize_tipo_ro(inner, namespace_atual);
                (Opcional(Box::new(norm)), { errs })
            }
            other => (other.clone(), vec![]),
        }
    }

    fn tipos_compativeis_atribuicao(&self, destino: &Tipo, origem: &Tipo) -> bool {
        use Tipo::*;
        if destino == origem {
            return true;
        }

        let is_dest_base_obj =
            matches!(destino, Objeto) || matches!(destino, Classe(n) if n == "objeto");
        let is_orig_base_obj =
            matches!(origem, Objeto) || matches!(origem, Classe(n) if n == "objeto");

        if is_dest_base_obj {
            return true;
        }
        if is_orig_base_obj {
            return true;
        }

        if let Inferido = destino {
            return true;
        }
        if let Inferido = origem {
            return true;
        }
        match (destino, origem) {
            (Generico(n1), Generico(n2)) => n1 == n2,
            (Generico(_), _) => true,
            (_, Generico(_)) => true,
            (Aplicado { nome: dn, args: da }, Aplicado { nome: on, args: oa }) => {
                if !dn.ends_with(on) && !on.ends_with(dn) {
                    return false;
                }
                if da.len() != oa.len() {
                    return false;
                }
                da.iter()
                    .zip(oa.iter())
                    .all(|(a1, a2)| self.tipos_compativeis_atribuicao(a1, a2))
            }
            (Lista(d), Lista(o)) => self.tipos_compativeis_atribuicao(d, o),
            (Classe(dest), Classe(orig)) => {
                if dest == orig {
                    true
                } else if self.is_subclass_of(orig, dest) {
                    true
                } else if self.is_interface_type(dest) {
                    self.class_implements_interface(orig, dest)
                } else {
                    false
                }
            }
            (Enum(a), Enum(b)) if a == b => true,
            (Texto, Inteiro) | (Texto, Booleano) => true,
            (Flutuante, Inteiro) => true,
            (Duplo, Inteiro) => true,
            (Duplo, Flutuante) => true,
            _ => false,
        }
    }

    fn is_interface_type(&self, nome: &str) -> bool {
        self.interfaces.contains_key(nome)
    }

    fn class_implements_interface(&self, class_fqn: &str, iface_fqn: &str) -> bool {
        let ifaces = self.get_all_interfaces_of_class(class_fqn);
        ifaces.contains(iface_fqn)
    }

    fn get_all_interfaces_of_class(&self, class_fqn: &str) -> std::collections::HashSet<String> {
        use std::collections::HashSet;
        let mut set: HashSet<String> = HashSet::new();
        let mut current = Some(class_fqn.to_string());
        while let Some(cls) = current {
            if let Some(ci) = self.resolved_classes.get(&cls) {
                let ns = self.get_namespace_from_full_name(&ci.name);
                for i in &ci.interfaces {
                    let fqn = self.resolver_nome_interface(i, &ns);
                    set.insert(fqn);
                }
                current = ci.parent_name.clone();
            } else if let Some(decl) = self.classes.get(&cls) {
                let ns = self.get_namespace_from_full_name(&cls);
                for i in &decl.interfaces {
                    let nome = match i {
                        Tipo::Classe(n) => n.as_str(),
                        Tipo::Aplicado { nome, .. } => nome.as_str(),
                        _ => "",
                    };
                    let fqn = self.resolver_nome_interface(nome, &ns);
                    set.insert(fqn);
                }
                current = decl.classe_pai.as_ref().map(|p| match p {
                    Tipo::Classe(n) => self.resolver_nome_classe(n, &ns),
                    Tipo::Aplicado { nome, .. } => self.resolver_nome_classe(nome, &ns),
                    _ => String::new(),
                });
            } else {
                break;
            }
        }
        set
    }

    fn is_subclass_of(&self, sub: &str, base: &str) -> bool {
        if sub == base {
            return true;
        }
        let mut current = Some(sub.to_string());
        while let Some(cls_fqn) = current {
            if let Some(ci) = self.resolved_classes.get(&cls_fqn) {
                if let Some(parent) = &ci.parent_name {
                    if parent == base {
                        return true;
                    }
                    current = Some(parent.clone());
                    continue;
                }
            } else if let Some(decl) = self.classes.get(&cls_fqn) {
                if let Some(parent_simple) = &decl.classe_pai {
                    let parent_name = match parent_simple {
                        Tipo::Classe(n) => n.as_str(),
                        Tipo::Aplicado { nome, .. } => nome.as_str(),
                        _ => "",
                    };
                    let parent_fqn = self.resolver_nome_classe(
                        parent_name,
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
            let mut ifaces_lista: Vec<String> = classe
                .interfaces
                .iter()
                .map(|t| match t {
                    Tipo::Classe(n) => n.clone(),
                    Tipo::Aplicado { nome, .. } => nome.clone(),
                    _ => String::new(),
                })
                .collect();
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
                    // Se a classe implementa a interface como tipo aplicado (ex.: I<TTexto>),
                    // criamos um mapa de substituição dos parâmetros genéricos da interface.
                    let mut subst_map: std::collections::HashMap<String, Tipo> =
                        std::collections::HashMap::new();
                    // Procura a interface aplicada tanto na lista de interfaces quanto no campo classe_pai
                    let iface_aplicada_opt: Option<&Tipo> = classe
                        .interfaces
                        .iter()
                        .find(|t| match t {
                            Tipo::Aplicado { nome, .. } => {
                                self.resolver_nome_interface(nome, &ns_atual) == iface_fqn
                            }
                            Tipo::Classe(n) => {
                                self.resolver_nome_interface(n, &ns_atual) == iface_fqn
                            }
                            _ => false,
                        })
                        .or_else(|| {
                            classe.classe_pai.as_ref().and_then(|p| match p {
                                Tipo::Aplicado { nome, .. } => {
                                    if self.resolver_nome_interface(nome, &ns_atual) == iface_fqn {
                                        Some(p)
                                    } else {
                                        None
                                    }
                                }
                                Tipo::Classe(n) => {
                                    if self.resolver_nome_interface(n, &ns_atual) == iface_fqn {
                                        Some(p)
                                    } else {
                                        None
                                    }
                                }
                                _ => None,
                            })
                        });

                    if let Some(iface_aplicada) = iface_aplicada_opt {
                        if let Tipo::Aplicado { nome: _, args } = iface_aplicada {
                            if !iface.generic_params.is_empty()
                                && iface.generic_params.len() == args.len()
                            {
                                for (g, a) in iface.generic_params.iter().zip(args.iter()) {
                                    let (a_norm, mut e) = self.normalize_tipo_ro(a, &ns_atual);
                                    self.erros.append(&mut e);
                                    subst_map.insert(g.clone(), a_norm);
                                }
                            }
                        }
                    }
                    for sig in &iface.metodos {
                        let (ret_i_norm, mut errs1) = self.normalize_tipo_ro(
                            &sig.tipo_retorno.clone().or(Some(Tipo::Vazio)).unwrap(),
                            &ns_atual,
                        );
                        self.erros.append(&mut errs1);
                        let mut params_i: Vec<Tipo> = Vec::new();
                        for p in sig.parametros.iter() {
                            let (tp_norm, mut e) = self.normalize_tipo_ro(&p.tipo, &ns_atual);
                            self.erros.append(&mut e);
                            params_i.push(tp_norm);
                        }
                        // Aplica substituição de genéricos nas assinaturas da interface, se houver
                        let ret_i = if subst_map.is_empty() {
                            ret_i_norm.clone()
                        } else {
                            self.substitute_generics_in_tipo(&ret_i_norm, &subst_map)
                        };
                        if !subst_map.is_empty() {
                            params_i = params_i
                                .into_iter()
                                .map(|t| self.substitute_generics_in_tipo(&t, &subst_map))
                                .collect();
                        }

                        if let Some(m) = resolved_methods.get(&sig.nome) {
                            let (ret_c_opt, params_c_orig) = self.assinatura_metodo(m);
                            let mut ret_c = ret_c_opt.clone();
                            if let Some(r) = ret_c_opt.as_ref() {
                                let (nr, mut e) = self.normalize_tipo_ro(r, &ns_atual);
                                self.erros.append(&mut e);
                                ret_c = Some(nr);
                            }
                            let mut params_c_norm: Vec<Tipo> = Vec::new();
                            for p in params_c_orig.into_iter() {
                                let (np, mut e) = self.normalize_tipo_ro(&p, &ns_atual);
                                self.erros.append(&mut e);
                                params_c_norm.push(np);
                            }
                            let params_c = params_c_norm;
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
                parent_name = parent_decl.classe_pai.clone().map(|p| match p {
                    Tipo::Classe(ref n) => {
                        self.resolver_nome_classe(n, &self.get_namespace_from_full_name(&pn))
                    }
                    Tipo::Aplicado { ref nome, .. } => {
                        self.resolver_nome_classe(nome, &self.get_namespace_from_full_name(&pn))
                    }
                    _ => String::new(),
                });
            } else {
                break;
            }
        }
        None
    }

    fn resolve_class_hierarchy(&mut self, class_name: &str, class_decl: &'a DeclaracaoClasse) {
        let mut stack: Vec<String> = Vec::new();
        self.resolve_class_hierarchy_with_stack(class_name, class_decl, &mut stack, 0);
    }

    fn resolve_class_hierarchy_with_stack(
        &mut self,
        class_name: &str,
        class_decl: &'a DeclaracaoClasse,
        stack: &mut Vec<String>,
        depth: usize,
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
        let mut interfaces_final: Vec<String> = class_decl
            .interfaces
            .iter()
            .map(|t| match t {
                ast::Tipo::Classe(n) => n.clone(),
                ast::Tipo::Aplicado { nome, .. } => nome.clone(),
                _ => {
                    self.get_declaracao_nome(&ast::Declaracao::DeclaracaoClasse(class_decl.clone()))
                }
            })
            .collect();
        let mut parent_effective: Option<String> = None;
        if let Some(parent_name_simple) = &class_decl.classe_pai {
            let parent_name_simple = match parent_name_simple {
                ast::Tipo::Classe(n) => n.clone(),
                ast::Tipo::Aplicado { nome, .. } => nome.clone(),
                other => {
                    self.erros.push(format!(
                        "Tipo inválido no cabeçalho da classe como base: {:?}",
                        other
                    ));
                    return;
                }
            };
            let parent_name = self.resolver_nome_classe(
                &parent_name_simple,
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
                self.resolve_class_hierarchy_with_stack(
                    &parent_name,
                    parent_decl,
                    stack,
                    depth + 1,
                );
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
                    &parent_name_simple,
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
        } else if class_name != "objeto" {
            // Por padrão, todas as classes herdam de 'objeto' (exceto o próprio 'objeto')
            let parent_name = "objeto".to_string();
            if let Some(parent_decl) = self.classes.get(&parent_name).copied() {
                self.resolve_class_hierarchy_with_stack(
                    &parent_name,
                    parent_decl,
                    stack,
                    depth + 1,
                );
                if let Some(parent_info) = self.resolved_classes.get(&parent_name) {
                    properties.extend(parent_info.properties.iter().cloned());
                    fields.extend(parent_info.fields.iter().cloned());
                    for (name, method) in &parent_info.methods {
                        methods.entry(name.clone()).or_insert(method);
                    }
                }
                parent_effective = Some(parent_name);
            } else {
                self.erros
                    .push("Classe base 'objeto' não encontrada no sistema.".into());
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
        if nome_classe.contains('.') {
            return nome_classe.to_string();
        }
        if !namespace_atual.is_empty() {
            let fqn = format!("{}.{}", namespace_atual, nome_classe);
            if self.classes.contains_key(&fqn) {
                return fqn;
            }
        }
        // NOVO: Consultar biblioteca externa primeiro
        if let Some(bib) = &self.biblioteca_externa {
            for using_path in &self.usings {
                let fqn = format!("{}.{}", using_path, nome_classe);
                if bib.simbolos.contains_key(&fqn) {
                    return fqn; // Classe existe na biblioteca externa
                }
            }
        }
        for using_path in &self.usings {
            let fqn = format!("{}.{}", using_path, nome_classe);
            if self.classes.contains_key(&fqn) {
                return fqn;
            }
            // Se o using é um namespace stdlib, confia que a classe existe
            if self.eh_classe_stdlib(using_path) {
                return fqn;
            }
        }
        if self.classes.contains_key(nome_classe) {
            return nome_classe.to_string();
        }
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

    fn validar_tipo_conhecido(&mut self, tipo: &Tipo, namespace_atual: &str, contexto: String) {
        match tipo {
            Tipo::Classe(class_name) => {
                let fqn_class = self.resolver_nome_classe(class_name, namespace_atual);
                let fqn_iface = self.resolver_nome_interface(class_name, namespace_atual);
                let fqn_enum = self.resolver_nome_enum(class_name, namespace_atual);

                let in_extern_lib = if let Some(bib) = &self.biblioteca_externa {
                    bib.simbolos.contains_key(&fqn_class)
                } else {
                    false
                };
                let is_generic = self.generic_scope.iter().any(|s| s.contains(class_name));

                if !is_generic && !self.classes.contains_key(&fqn_class) && !self.interfaces.contains_key(&fqn_iface) && !self.enums.contains_key(&fqn_enum) && !in_extern_lib {
                    self.erros.push(format!("Tipo '{}' desconhecido para {}.", class_name, contexto));
                }
            },
            Tipo::Lista(inner) => {
                self.validar_tipo_conhecido(inner, namespace_atual, contexto);
            }
            Tipo::Aplicado { nome, args } => {
                self.validar_tipo_conhecido(&Tipo::Classe(nome.clone()), namespace_atual, contexto.clone());
                for arg in args {
                    self.validar_tipo_conhecido(arg, namespace_atual, contexto.clone());
                }
            }
            _ => {} // Primitivos são sempre válidos
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
        match declaracao {
            Declaracao::DeclaracaoClasse(classe) => {
                let params: std::collections::HashSet<String> =
                    classe.generic_params.iter().cloned().collect();
                self.generic_scope.push(params);

                let fqn = if namespace_atual.is_empty() {
                    classe.nome.clone()
                } else {
                    format!("{}.{}", namespace_atual, classe.nome)
                };
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
                    let is_nativo = metodo.attributes.iter().any(|a| a.name == "Nativo");
                    // Um método é "externo" quando tem corpo vazio, não é abstrato, e tem o atributo [Nativo]
                    // (parseado via palavra-chave `externo` no parser).
                    let is_externo = !metodo.eh_abstrato
                        && !is_nativo
                        && metodo.corpo.is_empty()
                        && !metodo.eh_virtual
                        && !metodo.eh_override;
                    let _ = is_externo; // será usado em validações futuras

                    if is_nativo && !metodo.corpo.is_empty() {
                        self.erros.push(format!(
                            "Método nativo '{}' não pode ter corpo em '{}'",
                            metodo.nome, fqn
                        ));
                    }

                    let mut metodo_vars = escopo_vars.clone();
                    // Validação de override/virtual
                    if let Some(parent_simple) = &classe.classe_pai {
                        let base = match parent_simple {
                            Tipo::Classe(n) => n.as_str(),
                            Tipo::Aplicado { nome, .. } => nome.as_str(),
                            _ => "",
                        };
                        let parent_fqn = self.resolver_nome_classe(base, namespace_atual);
                        if metodo.eh_override {
                            if let Some(base_m) = self
                                .encontrar_metodo_na_base(Some(parent_fqn.clone()), &metodo.nome)
                            {
                                // Em C#, métodos abstratos são implicitamente virtuais (overridáveis)
                                if !(base_m.eh_virtual || base_m.eh_abstrato) {
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
                    if let Some(return_type) = &metodo.tipo_retorno {
                        let (resolved_return_type, mut errs) = self.normalize_tipo_ro(return_type, namespace_atual);
                        self.erros.append(&mut errs);
                        self.validar_tipo_conhecido(&resolved_return_type, namespace_atual, format!("o tipo de retorno do método '{}'", metodo.nome));
                    }

                    let mut metodo_vars = escopo_vars.clone();
                    for param in &metodo.parametros {
                        let (resolved_param_type, mut e) =
                            self.normalize_tipo_ro(&param.tipo, namespace_atual);
                        self.erros.append(&mut e);
                        self.validar_tipo_conhecido(&resolved_param_type, namespace_atual, format!("o parâmetro '{}' do método '{}'", param.nome, metodo.nome));
                        metodo_vars.insert(param.nome.clone(), resolved_param_type);
                    }
                    println!(
                        "DEBUG: Verificando método \"{}\". Parâmetros no escopo: {:?}",
                        metodo.nome, metodo_vars
                    );

                    let eh_stdlib = self.eh_classe_stdlib(&fqn);

                    if !metodo.eh_abstrato && !is_nativo && !eh_stdlib {
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
                self.generic_scope.pop();
            }
            Declaracao::DeclaracaoFuncao(funcao) => {
                let is_nativo = funcao.attributes.iter().any(|a| a.name == "Nativo");

                if is_nativo && !funcao.corpo.is_empty() {
                    self.erros.push(format!(
                        "Função nativa '{}' não pode ter corpo.",
                        funcao.nome
                    ));
                }

                let mut func_vars = escopo_vars.clone();
                for param in &funcao.parametros {
                    func_vars.insert(param.nome.clone(), param.tipo.clone());
                }

                let eh_stdlib = self.stdlib_namespaces.contains(namespace_atual);

                if !is_nativo && !eh_stdlib {
                    for comando in &funcao.corpo {
                        self.verificar_comando(comando, namespace_atual, None, &mut func_vars);
                    }
                }
            }
            Declaracao::Comando(cmd) => {
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
        match comando {
            Comando::DeclaracaoVariavel(tipo, nome, expr) => {
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
                if let Some(e) = expr {
                    let tipo_expr =
                        self.inferir_tipo_expressao(e, namespace_atual, classe_atual, escopo_vars);
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
                let obj_tipo = self.inferir_tipo_expressao(
                    obj_expr,
                    namespace_atual,
                    classe_atual,
                    escopo_vars,
                );
                if let Tipo::Classe(nome_classe) = obj_tipo {
                    if let Some(class_info) = self.resolved_classes.get(&nome_classe) {
                        let prop = class_info.properties.iter().find(|p| p.nome == *prop_nome);
                        let prop_tipo = prop.map(|p| p.tipo.clone()).or_else(|| {
                            class_info
                                .fields
                                .iter()
                                .find(|f| f.nome == *prop_nome)
                                .map(|f| f.tipo.clone())
                        });

                        if let Some(p) = prop {
                            if p.definir.is_none() {
                                self.erros.push(format!(
                                    "A propriedade \"{}\" é somente leitura (read-only) e não pode ser atribuída.",
                                    prop_nome
                                ));
                            }
                        }

                        if let Some(p_tipo) = prop_tipo {
                            let val_tipo = self.inferir_tipo_expressao(
                                val_expr,
                                namespace_atual,
                                classe_atual,
                                escopo_vars,
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
                } else if obj_tipo == Tipo::Inferido || obj_tipo == Tipo::Objeto {
                    // Treat Inferido or Objeto as a dynamic object to avoid blocking compilation of library code
                    // that might have complex late-bound types.
                    let val_tipo = self.inferir_tipo_expressao(
                        val_expr,
                        namespace_atual,
                        classe_atual,
                        escopo_vars,
                    );
                } else {
                    self.erros
                        .push("Atribuição de propriedade em algo que não é um objeto.".to_string());
                }
            }
            Comando::Bloco(comandos) => {
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
                // Verifica tipo do objeto e existência do método no tipo estático
                let obj_tipo = self.inferir_tipo_expressao(
                    obj_expr,
                    namespace_atual,
                    classe_atual,
                    escopo_vars,
                );
                for arg in args {
                    self.inferir_tipo_expressao(arg, namespace_atual, classe_atual, escopo_vars);
                }
                // Descobre o nome do método a partir do comando
                let metodo_nome = match comando {
                    Comando::ChamarMetodo(_, m, _) => m,
                    _ => unreachable!(),
                };

                match obj_tipo {
                    Tipo::Classe(ref nome) => {
                        // Pode ser interface ou classe
                        if self.interfaces.contains_key(nome) {
                            // Método deve existir na interface
                            if let Some(iface) = self.interfaces.get(nome) {
                                if !iface.metodos.iter().any(|s| &s.nome == metodo_nome) {
                                    self.erros.push(format!(
                                        "Método '{}' não existe na interface '{}'.",
                                        metodo_nome, nome
                                    ));
                                }
                            }
                        } else if let Some(class_info) = self.resolved_classes.get(nome) {
                            if !class_info.methods.contains_key(metodo_nome) {
                                // Pode existir em declaração bruta, mas resolved já inclui herdados
                                self.erros.push(format!(
                                    "Método '{}' não existe na classe '{}'.",
                                    metodo_nome, nome
                                ));
                            } else {
                                // Verificar modificador de acesso (semântica C#)
                                let metodo = class_info.methods.get(metodo_nome).unwrap();
                                
                                let mut is_static_access = false;
                                if let Expressao::Identificador(nome_id) = obj_expr.as_ref() {
                                    if !escopo_vars.contains_key(nome_id) {
                                        // Verifica se não é um campo ou propriedade da classe atual (implicit this)
                                        let is_member = classe_atual.and_then(|c| self.resolved_classes.get(c)).map_or(false, |info| {
                                            info.properties.iter().any(|p| p.nome == *nome_id) || info.fields.iter().any(|f| f.nome == *nome_id)
                                        });
                                        if !is_member {
                                            is_static_access = true;
                                        }
                                    }
                                }

                                if metodo.eh_estatica && !is_static_access {
                                    self.erros.push(format!(
                                        "O método '{}' de '{}' é estático e não pode ser chamado a partir de uma instância.",
                                        metodo_nome, nome
                                    ));
                                } else if !metodo.eh_estatica && is_static_access {
                                    self.erros.push(format!(
                                        "O método '{}' de '{}' não é estático e não pode ser chamado diretamente pela classe.",
                                        metodo_nome, nome
                                    ));
                                }

                                let is_private   = metodo.modificador == ModificadorAcesso::Privado;
                                let is_protected = metodo.modificador == ModificadorAcesso::Protegido;
                                let inside_same  = classe_atual.map_or(false, |c| c == nome);
                                let inside_sub   = classe_atual.map_or(false, |c| self.is_subclass_of(c, nome));

                                if is_private && !inside_same {
                                    self.erros.push(format!(
                                        "O método '{}' de '{}' é inacessível: é privado e só pode ser chamado dentro da própria classe.",
                                        metodo_nome, nome
                                    ));
                                } else if is_protected && !inside_same && !inside_sub {
                                    self.erros.push(format!(
                                        "O método '{}' de '{}' é inacessível: é protegido e só pode ser chamado dentro da classe ou de subclasses.",
                                        metodo_nome, nome
                                    ));
                                }
                            }
                        }
                    }
                    _ => {
                        // outros tipos por ora não têm métodos
                        self.erros.push(format!(
                            "Chamando método '{}' em tipo que não é objeto: {:?}",
                            metodo_nome, obj_tipo
                        ));
                    }
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
            _ => {}
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
            Expressao::Nulo => Tipo::Classe("objeto".to_string()),
            Expressao::Aguarde(inner) => match inner.as_ref() {
                Expressao::Chamada(nome, argumentos) => {
                    for argumento in argumentos {
                        self.inferir_tipo_expressao(
                            argumento,
                            namespace_atual,
                            classe_atual,
                            escopo_vars,
                        );
                    }
                    match nome.as_str() {
                        "LerArquivoAssíncrono" => Tipo::Texto,
                        "EscreverArquivoAssíncrono" | "VerificarArquivoAssíncrono" => {
                            Tipo::Booleano
                        }
                        _ => {
                            let fqn = self.resolver_nome_funcao(nome, namespace_atual);
                            match self.simbolos_namespaces.get(&fqn) {
                                Some(Declaracao::DeclaracaoFuncao(function))
                                    if function.eh_assincrona =>
                                {
                                    function.tipo_retorno.clone().unwrap_or(Tipo::Vazio)
                                }
                                _ => {
                                    self.erros.push(format!(
                                        "aguarde requer uma função assíncrona; '{}' não foi encontrada ou não foi marcada como assíncrona.",
                                        nome
                                    ));
                                    Tipo::Inferido
                                }
                            }
                        }
                    }
                }
                Expressao::ChamadaMetodo(objeto, metodo, argumentos) if matches!(objeto.as_ref(), Expressao::Identificador(nome) if nome == "Arquivo") =>
                {
                    for argumento in argumentos {
                        self.inferir_tipo_expressao(
                            argumento,
                            namespace_atual,
                            classe_atual,
                            escopo_vars,
                        );
                    }
                    match metodo.as_str() {
                        "LerTextoAssíncrono" => Tipo::Texto,
                        "EscreverTextoAssíncrono" | "ExisteAssíncrono" => Tipo::Booleano,
                        _ => Tipo::Inferido,
                    }
                }
                _ => {
                    self.erros
                        .push("aguarde requer uma chamada assíncrona.".to_string());
                    Tipo::Inferido
                }
            },
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
                // Se o nome é um namespace stdlib (ex: "Sistema"), trata como classe para acesso a membros
                if self.eh_classe_stdlib(nome) {
                    return Tipo::Classe(nome.to_string());
                }
                // Classe?
                let fqn_class = self.resolver_nome_classe(nome, namespace_atual);
                if self.classes.contains_key(&fqn_class) {
                    return Tipo::Classe(fqn_class);
                }
                // NOVO: Verificar se a classe está na biblioteca externa
                if let Some(bib) = &self.biblioteca_externa {
                    if bib.simbolos.contains_key(&fqn_class) {
                        return Tipo::Classe(fqn_class);
                    }
                }
                // Se a classe está em um namespace stdlib, confia que existe (evita erro)
                let namespace_do_identificador = if fqn_class.contains('.') {
                    fqn_class.rfind('.').map(|i| &fqn_class[..i]).unwrap_or("")
                } else {
                    ""
                };
                if self.eh_classe_stdlib(&namespace_do_identificador) {
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

                let lookup_class_name = match &obj_tipo {
                    Tipo::Classe(nome) => Some(nome.clone()),
                    Tipo::Aplicado { nome, .. } => Some(nome.clone()),
                    _ => None,
                };

                if let Some(nome_classe) = lookup_class_name {
                    let fqn = self.resolver_nome_classe(&nome_classe, namespace_atual);

                    // NOVO: Consultar biblioteca externa para métodos
                    if let Some(bib) = &self.biblioteca_externa {
                        if let Some(LibSimbolo::Classe(lib_classe)) = bib.simbolos.get(&fqn) {
                            if let Some(metodo) = lib_classe.metodos.get(membro_nome) {
                                return string_para_tipo(&metodo.tipo_retorno);
                            }
                        }
                    }

                    if let Some(class_info) = self.resolved_classes.get(&fqn) {
                        if let Some(prop) = class_info
                            .properties
                            .iter()
                            .find(|p| p.nome == *membro_nome)
                        {
                            // Verificar modificador de acesso para propriedades (semântica C#)
                            let is_private   = prop.modificador == ModificadorAcesso::Privado;
                            let is_protected = prop.modificador == ModificadorAcesso::Protegido;
                            let inside_same  = classe_atual.map_or(false, |c| c == &fqn);
                            let inside_sub   = classe_atual.map_or(false, |c| self.is_subclass_of(c, &fqn));

                            if is_private && !inside_same {
                                self.erros.push(format!(
                                    "A propriedade '{}' de '{}' é inacessível: é privada e só pode ser acessada dentro da própria classe.",
                                    membro_nome, fqn
                                ));
                            } else if is_protected && !inside_same && !inside_sub {
                                self.erros.push(format!(
                                    "A propriedade '{}' de '{}' é inacessível: é protegida e só pode ser acessada dentro da classe ou de subclasses.",
                                    membro_nome, fqn
                                ));
                            }
                            return prop.tipo.clone();
                        }
                        if let Some(field) =
                            class_info.fields.iter().find(|f| f.nome == *membro_nome)
                        {
                            // Verificar modificador de acesso para campos (semântica C#)
                            let is_private   = field.modificador == ModificadorAcesso::Privado;
                            let is_protected = field.modificador == ModificadorAcesso::Protegido;
                            let inside_same  = classe_atual.map_or(false, |c| c == &fqn);
                            let inside_sub   = classe_atual.map_or(false, |c| self.is_subclass_of(c, &fqn));

                            if is_private && !inside_same {
                                self.erros.push(format!(
                                    "O campo '{}' de '{}' é inacessível: é privado e só pode ser acessado dentro da própria classe.",
                                    membro_nome, fqn
                                ));
                            } else if is_protected && !inside_same && !inside_sub {
                                self.erros.push(format!(
                                    "O campo '{}' de '{}' é inacessível: é protegido e só pode ser acessado dentro da classe ou de subclasses.",
                                    membro_nome, fqn
                                ));
                            }
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

                // Fallback: if the object is a class, we return Inferido but avoid immediate error
                if let Tipo::Classe(_) = obj_tipo {
                    return Tipo::Inferido;
                }

                Tipo::Inferido
            }
            Expressao::ListaLiteral(items) => {
                // Inferência de tipo para listas: tenta encontrar tipo comum
                if items.is_empty() {
                    return Tipo::Lista(Box::new(Tipo::Inferido));
                }
                // Coletar tipos de todos os itens
                let tipos: Vec<Tipo> = items
                    .iter()
                    .map(|e| {
                        self.inferir_tipo_expressao(e, namespace_atual, classe_atual, escopo_vars)
                    })
                    .collect();
                // 1) Se todos compatíveis com o primeiro (e vice-versa), use o primeiro
                let first = tipos[0].clone();
                let mut todos_compat = true;
                for t in &tipos[1..] {
                    if !self.tipos_compativeis_atribuicao(&first, t)
                        || !self.tipos_compativeis_atribuicao(t, &first)
                    {
                        todos_compat = false;
                        break;
                    }
                }
                if todos_compat {
                    return Tipo::Lista(Box::new(first));
                }
                // 2) Se todos forem classes, tentar achar interface comum

                let classes: Option<Vec<String>> = tipos
                    .iter()
                    .map(|t| {
                        if let Tipo::Classe(c) = t {
                            Some(c.clone())
                        } else {
                            None
                        }
                    })
                    .collect();
                if let Some(cls_vec) = classes {
                    if !cls_vec.is_empty() {
                        use std::collections::HashSet;
                        // Começa com interfaces do primeiro e intersecta com os demais
                        let mut intersec: HashSet<String> =
                            self.get_all_interfaces_of_class(&cls_vec[0]);
                        for c in &cls_vec[1..] {
                            let si = self.get_all_interfaces_of_class(c);
                            intersec = intersec.intersection(&si).cloned().collect::<HashSet<_>>();
                            if intersec.is_empty() {
                                break;
                            }
                        }
                        if let Some(iface_fqn) = intersec.into_iter().next() {
                            return Tipo::Lista(Box::new(Tipo::Classe(iface_fqn)));
                        }
                    }
                }
                // 3) Falha — tipos heterogêneos sem supertipo comum
                self.erros
                    .push("Elementos do array devem ter tipos compatíveis".into());
                Tipo::Lista(Box::new(Tipo::Inferido))
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
            Expressao::NovoObjeto(t, args) => {
                let (t_norm, mut errs) = self.normalize_tipo_ro(t, namespace_atual);
                self.erros.append(&mut errs);

                if let Tipo::Classe(nome_classe) = &t_norm {
                    if let Some(classe_decl) = self.classes.get(nome_classe) {
                        // Se não houver construtores definidos e a chamada é sem argumentos,
                        // é uma chamada ao construtor padrão implícito, que é público.
                        if classe_decl.construtores.is_empty() && args.is_empty() {
                            // Construtor padrão, acesso permitido.
                        } else if let Some(construtor) =
                            classe_decl.construtores.iter().find(|c| {
                                // Aceita se o número de args está entre
                                // os parâmetros obrigatórios e o total (C# semantics)
                                let total = c.parametros.len();
                                let obrigatorios = c.parametros.iter()
                                    .filter(|p| p.valor_padrao.is_none())
                                    .count();
                                args.len() >= obrigatorios && args.len() <= total
                            })
                        {
                            let is_private = construtor.modificador == ModificadorAcesso::Privado;
                            let is_protected = construtor.modificador == ModificadorAcesso::Protegido;

                            let is_inside_same_class =
                                classe_atual.map_or(false, |current_class_fqn| current_class_fqn == nome_classe);
                            
                            let is_inside_subclass = if let Some(current_class_fqn) = classe_atual {
                                self.is_subclass_of(current_class_fqn, nome_classe)
                            } else {
                                false
                            };

                            if (is_private && !is_inside_same_class) || (is_protected && !is_inside_subclass) {
                                self.erros.push(format!(
                                    "O construtor da classe '{}' é inacessível devido ao seu nível de proteção.",
                                    nome_classe
                                ));
                            }
                        } else {
                            self.erros.push(format!(
                                "A classe '{}' não contém um construtor que receba {} argumentos.",
                                nome_classe,
                                args.len()
                            ));
                        }

                    }
                }

                t_norm
            }
            Expressao::NovoArray(t, _) => {
                let (t_norm, mut errs) = self.normalize_tipo_ro(t, namespace_atual);
                self.erros.append(&mut errs);
                Tipo::Lista(Box::new(t_norm))
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
            Expressao::ChamadaMetodo(obj_expr, metodo_nome, _args) => {
                let obj_tipo = self.inferir_tipo_expressao(
                    obj_expr,
                    namespace_atual,
                    classe_atual,
                    escopo_vars,
                );
                if let Tipo::Classe(nome_classe) = obj_tipo {
                    // NOVO: Consultar biblioteca externa primeiro
                    if let Some(bib) = &self.biblioteca_externa {
                        if let Some(LibSimbolo::Classe(lib_classe)) = bib.simbolos.get(&nome_classe)
                        {
                            if let Some(metodo) = lib_classe.metodos.get(metodo_nome) {
                                return string_para_tipo(&metodo.tipo_retorno);
                            }
                        }
                    }
                    if let Some(class_info) = self.resolved_classes.get(&nome_classe) {
                        if let Some(metodo) = class_info.methods.get(metodo_nome) {
                            let mut is_static_access = false;
                            if let Expressao::Identificador(nome_id) = obj_expr.as_ref() {
                                if !escopo_vars.contains_key(nome_id) {
                                    let is_member = classe_atual.and_then(|c| self.resolved_classes.get(c)).map_or(false, |info| {
                                        info.properties.iter().any(|p| p.nome == *nome_id) || info.fields.iter().any(|f| f.nome == *nome_id)
                                    });
                                    if !is_member {
                                        is_static_access = true;
                                    }
                                }
                            }

                            if metodo.eh_estatica && !is_static_access {
                                self.erros.push(format!(
                                    "O método '{}' de '{}' é estático e não pode ser chamado a partir de uma instância.",
                                    metodo_nome, nome_classe
                                ));
                            } else if !metodo.eh_estatica && is_static_access {
                                self.erros.push(format!(
                                    "O método '{}' de '{}' não é estático e não pode ser chamado diretamente pela classe.",
                                    metodo_nome, nome_classe
                                ));
                            }

                            // Verificar modificador de acesso (semântica C#)
                            let is_private   = metodo.modificador == ModificadorAcesso::Privado;
                            let is_protected = metodo.modificador == ModificadorAcesso::Protegido;
                            let inside_same  = classe_atual.map_or(false, |c| c == &nome_classe);
                            let inside_sub   = classe_atual.map_or(false, |c| self.is_subclass_of(c, &nome_classe));

                            if is_private && !inside_same {
                                self.erros.push(format!(
                                    "O método '{}' de '{}' é inacessível: é privado e só pode ser chamado dentro da própria classe.",
                                    metodo_nome, nome_classe
                                ));
                            } else if is_protected && !inside_same && !inside_sub {
                                self.erros.push(format!(
                                    "O método '{}' de '{}' é inacessível: é protegido e só pode ser chamado dentro da classe ou de subclasses.",
                                    metodo_nome, nome_classe
                                ));
                            }
                            return metodo.tipo_retorno.clone().unwrap_or(Tipo::Vazio);
                        }
                    }
                }
                Tipo::Inferido
            }
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
            Expressao::Nulo => Tipo::Classe("objeto".to_string()),
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
                // NOVO: Verificar se a classe está na biblioteca externa
                if let Some(bib) = &self.biblioteca_externa {
                    if bib.simbolos.contains_key(&fqn_class) {
                        return Tipo::Classe(fqn_class);
                    }
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
                    // NOVO: Consultar biblioteca externa primeiro
                    if let Some(bib) = &self.biblioteca_externa {
                        if let Some(LibSimbolo::Classe(lib_classe)) = bib.simbolos.get(nome_classe)
                        {
                            if let Some(metodo) = lib_classe.metodos.get(membro_nome) {
                                return string_para_tipo(&metodo.tipo_retorno);
                            }
                        }
                    }
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
            Expressao::NovoObjeto(t, _) => {
                let (t_norm, _) = self.normalize_tipo_ro(t, namespace_atual);
                t_norm
            }
            Expressao::NovoArray(t, _) => {
                let (t_norm, _) = self.normalize_tipo_ro(t, namespace_atual);
                Tipo::Lista(Box::new(t_norm))
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
