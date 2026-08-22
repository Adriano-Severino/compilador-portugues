use crate::ast;
use crate::ast::*;
use crate::error::{ErroCompilador, TipoErro};
use crate::library_loader::{self, LibSimbolo};
use std::collections::{HashMap, HashSet};
use super::ResolvedClassInfo;
use super::VerificadorTipos;

impl<'a> VerificadorTipos<'a> {
    // Substitui parâmetros genéricos por tipos concretos em um tipo arbitrário.
    pub(crate) fn substitute_generics_in_tipo(
        &self,
        t: &Tipo,
        subst: &std::collections::HashMap<String, Tipo>,
    ) -> Tipo {
        self.substitute_generics_in_tipo_recursive(t, subst, 0)
    }

    pub(crate) fn substitute_generics_in_tipo_recursive(
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

}
