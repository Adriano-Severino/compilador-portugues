use crate::ast;
use crate::ast::*;
use crate::error::{ErroCompilador, TipoErro};
use crate::library_loader::{self, LibSimbolo};
use std::collections::{HashMap, HashSet};
use super::{ResolvedClassInfo, string_para_tipo};
use super::VerificadorTipos;

impl<'a> VerificadorTipos<'a> {
    pub(crate) fn tipos_compativeis_atribuicao(&self, destino: &Tipo, origem: &Tipo) -> bool {
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
                // Genéricos no C# são invariantes por padrão (salvo in/out, que não suportamos aqui)
                // Portanto, T1 deve ser estritamente igual a T2, não apenas assignable.
                da.iter().zip(oa.iter()).all(|(a1, a2)| a1 == a2)
            }
            (Aplicado { nome: dn, args: da }, Classe(orig)) => {
                self.class_implements_applied_interface(orig, dn, da)
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

    pub(crate) fn is_interface_type(&self, nome: &str) -> bool {
        self.interfaces.contains_key(nome)
    }

    pub(crate) fn class_implements_interface(&self, class_fqn: &str, iface_fqn: &str) -> bool {
        let ifaces = self.get_all_interfaces_of_class(class_fqn);
        ifaces.contains(iface_fqn)
    }

    pub(crate) fn class_implements_applied_interface(
        &self,
        class_fqn: &str,
        iface_fqn: &str,
        iface_args: &[Tipo],
    ) -> bool {
        let mut current = Some(class_fqn.to_string());
        while let Some(cls) = current {
            if let Some(ci) = self.resolved_classes.get(&cls) {
                let ns = self.get_namespace_from_full_name(&ci.name);
                // Check if any applied interface matches
                // Wait, resolved_classes stores the interface name as string, it lost the generic arguments!
                // Wait, I need to check the AST declaration instead.
            }
            if let Some(decl) = self.classes.get(&cls) {
                let ns = self.get_namespace_from_full_name(&cls);
                let mut potential_interfaces = decl.interfaces.clone();
                if let Some(ref pai) = decl.classe_pai {
                    potential_interfaces.push(pai.clone());
                }

                for i in &potential_interfaces {
                    if let Tipo::Aplicado { nome, args } = i {
                        let fqn = self.resolver_nome_interface(nome, &ns);
                        if (fqn == iface_fqn || fqn.ends_with(iface_fqn))
                            && args.len() == iface_args.len()
                        {
                            let mut ok = true;
                            for (a1, a2) in args.iter().zip(iface_args.iter()) {
                                if a1 != a2 {
                                    ok = false;
                                    break;
                                }
                            }
                            if ok {
                                return true;
                            }
                        }
                    }
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
        false
    }

    pub(crate) fn get_all_interfaces_of_class(&self, class_fqn: &str) -> std::collections::HashSet<String> {
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

    pub(crate) fn is_subclass_of(&self, sub: &str, base: &str) -> bool {
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

    pub(crate) fn resolve_class_hierarchy(&mut self, class_name: &str, class_decl: &'a DeclaracaoClasse) {
        let mut stack: Vec<String> = Vec::new();
        self.resolve_class_hierarchy_with_stack(class_name, class_decl, &mut stack, 0);
    }

    pub(crate) fn resolve_class_hierarchy_with_stack(
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
            self.erros.push(ErroCompilador::novo(
                TipoErro::Semântico,
                format!("Herança circular detectada: {}", ciclo.join(" -> ")),
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
                    self.erros.push(ErroCompilador::novo(
                        TipoErro::Semântico,
                        format!(
                            "Tipo inválido no cabeçalho da classe como base: {:?}",
                            other
                        ),
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
                self.erros.push(ErroCompilador::novo(
                    TipoErro::Semântico,
                    format!("Herança circular detectada: {}", ciclo.join(" -> ")),
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
                    self.erros.push(ErroCompilador::novo(
                        TipoErro::Semântico,
                        format!(
                            "Classe pai '{}' não encontrada para '{}'.",
                            parent_name, class_name
                        ),
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
                self.erros.push(ErroCompilador::novo(
                    TipoErro::Semântico,
                    "Classe base 'objeto' não encontrada no sistema.".into(),
                ));
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

    pub(crate) fn verificar_namespace(&mut self, ns: &'a DeclaracaoNamespace) {
        let mut ns_vars = HashMap::new();
        for decl in &ns.declaracoes {
            self.verificar_declaracao(decl, &ns.nome, &mut ns_vars);
        }
    }

    pub(crate) fn assinatura_metodo(&self, m: &'a ast::MetodoClasse) -> (Option<Tipo>, Vec<Tipo>) {
        let ret = m.tipo_retorno.clone().or(Some(Tipo::Vazio));
        let params = m.parametros.iter().map(|p| p.tipo.clone()).collect();
        (ret, params)
    }

    pub(crate) fn encontrar_metodo_na_base(
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

}
