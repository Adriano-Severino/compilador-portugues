pub mod tipos_e_structs;
pub mod genericos;
pub mod hierarquia;
pub mod resolucao;
pub mod declaracoes;
pub mod comandos;
pub mod expressoes;

pub use tipos_e_structs::{VerificadorTipos, ResolvedClassInfo, string_para_tipo};

use crate::ast;
use crate::ast::*;
use crate::error::{ErroCompilador, TipoErro};
use crate::library_loader::{self, LibSimbolo};
use std::collections::{HashMap, HashSet};

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

    pub(crate) fn inicializar_tipos_integrados(&mut self) {
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
                                eh_assincrona: false,
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

    pub fn verificar_programa(
        &mut self,
        programa: &'a Programa,
    ) -> Result<(), Vec<ErroCompilador>> {
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
                                self.erros.push(ErroCompilador::novo(
                                    TipoErro::Semântico,
                                    format!(
                                    "Classe '{}' não implementa corretamente método '{}' da interface '{}'. Assinatura esperada: ({:?}) -> {:?}",
                                    fqn, sig.nome, iface_fqn, params_i, ret_i
                                )));
                            }
                        } else if !classe_eh_abstrata {
                            self.erros.push(ErroCompilador::novo(
                                TipoErro::Semântico,
                                format!(
                                "Classe '{}' não implementa método obrigatório '{}' da interface '{}'",
                                fqn, sig.nome, iface_fqn
                            )));
                        }
                    }
                } else {
                    self.erros.push(ErroCompilador::novo(
                        TipoErro::Semântico,
                        format!(
                            "Interface '{}' não encontrada (referenciada por '{}')",
                            iface_nome, fqn
                        ),
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

}
