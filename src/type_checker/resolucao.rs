use crate::ast;
use crate::ast::*;
use crate::error::{ErroCompilador, TipoErro};
use crate::library_loader::{self, LibSimbolo};
use std::collections::{HashMap, HashSet};
use super::ResolvedClassInfo;
use super::VerificadorTipos;

impl<'a> VerificadorTipos<'a> {
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

    pub(crate) fn get_namespace_from_full_name(&self, full_name: &str) -> String {
        if let Some(pos) = full_name.rfind('.') {
            full_name[..pos].to_string()
        } else {
            "".to_string()
        }
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

    pub(crate) fn get_declaracao_nome(&self, declaracao: &Declaracao) -> String {
        match declaracao {
            Declaracao::DeclaracaoFuncao(f) => f.nome.clone(),
            Declaracao::DeclaracaoClasse(c) => c.nome.clone(),
            Declaracao::DeclaracaoInterface(i) => i.nome.clone(),
            Declaracao::DeclaracaoEnum(e) => e.nome.clone(),
            _ => "".to_string(),
        }
    }

    pub(crate) fn validar_tipo_conhecido(&mut self, tipo: &Tipo, namespace_atual: &str, contexto: String) {
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

                if !is_generic
                    && !self.classes.contains_key(&fqn_class)
                    && !self.interfaces.contains_key(&fqn_iface)
                    && !self.enums.contains_key(&fqn_enum)
                    && !in_extern_lib
                {
                    self.erros.push(ErroCompilador::novo(
                        TipoErro::Semântico,
                        format!("Tipo '{}' desconhecido para {}.", class_name, contexto),
                    ));
                }
            }
            Tipo::Lista(inner) => {
                self.validar_tipo_conhecido(inner, namespace_atual, contexto);
            }
            Tipo::Aplicado { nome, args } => {
                self.validar_tipo_conhecido(
                    &Tipo::Classe(nome.clone()),
                    namespace_atual,
                    contexto.clone(),
                );
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

    pub fn is_member_of_class(&self, class_name: &str, member_name: &str) -> bool {
        if let Some(class_info) = self.resolved_classes.get(class_name) {
            return class_info.fields.iter().any(|f| f.nome == member_name)
                || class_info.properties.iter().any(|p| p.nome == member_name);
        }
        false
    }

}
