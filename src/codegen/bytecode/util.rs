use crate::ast;
use std::collections::{HashMap, HashSet};
use crate::library_loader::LibSimbolo;

/// Escapa um texto para emissão em `LOAD_CONST_STR "..."` (formato de linha única).
/// O interpretador reverte estes escapes ao carregar a constante.
pub(crate) fn escapar_texto_bytecode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\0' => out.push_str("\\0"),
            outro => out.push(outro),
        }
    }
    out
}

use super::BytecodeGenerator;

impl<'a> BytecodeGenerator<'a> {
    pub(crate) fn spawn_child(&self) -> Self {
        BytecodeGenerator {
            programa: self.programa,
            type_checker: self.type_checker,
            namespace_path: self.namespace_path.clone(),
            bytecode_instructions: Vec::new(),
            props_por_classe: self.props_por_classe.clone(),
            construtor_params_por_classe: self.construtor_params_por_classe.clone(),
            current_class_name: self.current_class_name.clone(),
            current_params: self.current_params.clone(),
        }
    }
    pub(crate) fn get_class_declaration(&self, class_name: &str) -> Option<&'a ast::DeclaracaoClasse> {
        self.type_checker.classes.get(class_name).copied()
    }

    pub(crate) fn qual(&self, local: &str) -> String {
        if self.namespace_path.is_empty() {
            local.to_owned()
        } else {
            format!("{}.{}", self.namespace_path, local)
        }
    }

}
