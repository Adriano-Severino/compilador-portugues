use crate::ast;
use std::collections::{HashMap, HashSet};
use crate::library_loader::LibSimbolo;

use super::LlvmGenerator;

impl<'a> LlvmGenerator<'a> {
    pub(crate) fn define_all_interface_structs(&mut self) {
        // Cria um tipo LLVM identificado para cada interface conhecida para que possamos
        // referenciá-lo em parâmetros/retornos (%class.Interface*). Usa um layout mínimo
        // compatível com classes (primeiro campo: ponteiro para vtable i8**), embora
        // atualmente não haja vtable específica para interfaces.
        for (iface_fqn, _iface_decl) in &self.type_checker.interfaces {
            // Evita colisão caso exista uma classe com o mesmo FQN já definida
            if self.resolved_classes.contains_key(iface_fqn) {
                continue;
            }
            let sanitized = iface_fqn.replace('.', "_");
            let def = format!("%class.{0} = type {{ i8** }}\n", sanitized);
            self.header.push_str(&def);
        }
    }

    pub(crate) fn define_all_applied_interface_structs(&mut self) {
        // snapshot para evitar empréstimos conflitantes
        let mut items: Vec<(String, Vec<ast::Tipo>)> = Vec::new();
        for (iface_fqn, insts) in &self.applied_iface_insts {
            for args in insts {
                items.push((iface_fqn.clone(), args.clone()));
            }
        }
        for (iface_fqn, args) in items {
            let mangled = self.mangle_aplicado_name(&iface_fqn, &args);
            let sanitized = mangled.replace('.', "_");
            let def = format!("%class.{0} = type {{ i8** }}\n", sanitized);
            self.header.push_str(&def);
        }
    }

    pub(crate) fn define_all_structs(&mut self) {
        let mut fqns: Vec<_> = self.resolved_classes.keys().collect();
        fqns.sort(); // Ordena para garantir uma ordem consistente.

        for fqn in fqns {
            self.define_struct(fqn.as_str());
        }

        // Define structs especializados para classes aplicadas (monomorfização superficial)
        // snapshot para evitar empréstimo duplo
        let mut items: Vec<(String, Vec<ast::Tipo>)> = Vec::new();
        for (base_fqn, insts) in &self.applied_class_insts {
            for args in insts {
                items.push((base_fqn.clone(), args.clone()));
            }
        }
        for (base_fqn, args) in items {
            self.define_applied_struct(&base_fqn, &args);
        }
    }

    pub(crate) fn define_struct(&mut self, fqn: &str) {
        let mut field_types_llvm = Vec::new();
        // Primeiro campo: ponteiro para vtable (i8**)
        field_types_llvm.push("i8**".to_string());
        if let Some(resolved_info) = self.resolved_classes.get(fqn) {
            let mut all_fields: Vec<(&String, &ast::Tipo)> = resolved_info
                .fields
                .iter()
                .map(|f| (&f.nome, &f.tipo))
                .collect();
            all_fields.extend(resolved_info.properties.iter().map(|p| (&p.nome, &p.tipo)));

            for (_, tipo) in all_fields {
                field_types_llvm.push(self.map_type_to_llvm_storage(tipo));
            }
        }

        let struct_body = field_types_llvm.join(", ");
        let sanitized_fqn = fqn.replace('.', "_");
        let struct_def = format!("%class.{0} = type {{ {1} }}\n", sanitized_fqn, struct_body);
        self.header.push_str(&struct_def);
    }

    pub(crate) fn define_applied_struct(&mut self, base_fqn: &str, args: &Vec<ast::Tipo>) {
        // Monta substituição: parâmetros genéricos da classe -> args
        let class_decl = match self.type_checker.classes.get(base_fqn) {
            Some(c) => *c,
            None => return,
        };
        if class_decl.generic_params.is_empty() {
            return;
        }
        if class_decl.generic_params.len() != args.len() {
            return;
        }

        let mut subst: HashMap<String, ast::Tipo> = HashMap::new();
        for (g, a) in class_decl.generic_params.iter().zip(args.iter()) {
            subst.insert(g.clone(), a.clone());
        }

        // Começa sempre com vptr como primeiro campo
        let mut field_types_llvm = vec!["i8**".to_string()];
        // Herdar campos da classe base resolvida (já expandida por herança)
        if let Some(resolved_info) = self.resolved_classes.get(base_fqn) {
            let mut all_fields: Vec<&ast::Tipo> =
                resolved_info.fields.iter().map(|f| &f.tipo).collect();
            all_fields.extend(resolved_info.properties.iter().map(|p| &p.tipo));

            for t in all_fields {
                let t2 = self.subst_generics_local(t, &subst);
                field_types_llvm.push(self.map_type_to_llvm_storage(&t2));
            }
        }

        let mangled = self.mangle_aplicado_name(base_fqn, args);
        let struct_body = field_types_llvm.join(", ");
        let sanitized = mangled.replace('.', "_");
        let struct_def = format!("%class.{0} = type {{ {1} }}\n", sanitized, struct_body);
        self.header.push_str(&struct_def);
    }

    pub(crate) fn resolve_type(&self, tipo: &ast::Tipo, namespace: &str) -> ast::Tipo {
        match tipo {
            ast::Tipo::Classe(unresolved_name) => {
                // Primeiro tenta resolver como classe
                let fqn_class = self
                    .type_checker
                    .resolver_nome_classe(unresolved_name, namespace);
                if self.type_checker.classes.contains_key(&fqn_class) {
                    return ast::Tipo::Classe(fqn_class);
                }
                // Depois tenta como enumeração
                let fqn_enum = self
                    .type_checker
                    .resolver_nome_enum(unresolved_name, namespace);
                if self.type_checker.enums.contains_key(&fqn_enum) {
                    return ast::Tipo::Enum(fqn_enum);
                }
                // Mantém original caso não resolva
                tipo.clone()
            }
            ast::Tipo::Aplicado { nome, args } => {
                // Pode ser classe ou interface aplicada
                let fqn_cls = self.type_checker.resolver_nome_classe(nome, namespace);
                let fqn_iface = self.type_checker.resolver_nome_interface(nome, namespace);
                let mut norm_args: Vec<ast::Tipo> = Vec::new();
                for a in args {
                    norm_args.push(self.resolve_type(a, namespace));
                }
                if self.type_checker.classes.contains_key(&fqn_cls) {
                    let mangled = self.mangle_aplicado_name(&fqn_cls, &norm_args);
                    ast::Tipo::Aplicado {
                        nome: mangled,
                        args: vec![],
                    }
                } else if self.type_checker.interfaces.contains_key(&fqn_iface) {
                    let mangled = self.mangle_aplicado_name(&fqn_iface, &norm_args);
                    ast::Tipo::Aplicado {
                        nome: mangled,
                        args: vec![],
                    }
                } else {
                    // fallback para classe
                    let fqn_class = self.type_checker.resolver_nome_classe(nome, namespace);
                    ast::Tipo::Classe(fqn_class)
                }
            }
            other => other.clone(),
        }
    }

    pub(crate) fn map_type_to_llvm_storage(&self, tipo: &ast::Tipo) -> String {
        match tipo {
            ast::Tipo::Inteiro => "i32".to_string(),
            ast::Tipo::Texto => "i8*".to_string(),
            ast::Tipo::Flutuante => "float".to_string(),
            ast::Tipo::Duplo => "double".to_string(),
            ast::Tipo::Decimal => "i8*".to_string(),
            ast::Tipo::Booleano => "i1".to_string(),
            ast::Tipo::Vazio => "void".to_string(),
            ast::Tipo::Enum(_) => "i32".to_string(),
            ast::Tipo::Classe(_) => self.map_type_to_llvm_ptr(tipo),
            ast::Tipo::Aplicado { .. } => self.map_type_to_llvm_ptr(tipo),
            ast::Tipo::Lista(_) => "%array*".to_string(),
            _ => panic!("Tipo LLVM não mapeado para armazenamento: {:?}", tipo),
        }
    }

    pub(crate) fn map_type_to_llvm_ptr(&self, tipo: &ast::Tipo) -> String {
        match tipo {
            ast::Tipo::Inteiro => "i32*".to_string(),
            ast::Tipo::Texto => "i8**".to_string(),
            ast::Tipo::Flutuante => "float*".to_string(),
            ast::Tipo::Duplo => "double*".to_string(),
            ast::Tipo::Decimal => "i8**".to_string(),
            ast::Tipo::Booleano => "i1*".to_string(),
            ast::Tipo::Enum(_) => "i32*".to_string(),
            ast::Tipo::Classe(name) => {
                let sanitized_name = name.replace('.', "_");
                format!("%class.{0}*", sanitized_name)
            }
            ast::Tipo::Aplicado { nome, .. } => {
                // 'nome' aqui já deve estar mangled via resolve_type
                let sanitized_name = nome.replace('.', "_");
                format!("%class.{0}*", sanitized_name)
            }
            ast::Tipo::Lista(_) => "%array**".to_string(),
            _ => panic!("Não é possível criar um ponteiro para o tipo: {:?}", tipo),
        }
    }

    pub(crate) fn map_type_to_llvm_arg(&self, tipo: &ast::Tipo) -> String {
        match tipo {
            ast::Tipo::Inteiro => "i32".to_string(),
            ast::Tipo::Texto => "i8*".to_string(),
            ast::Tipo::Flutuante => "float".to_string(),
            ast::Tipo::Duplo => "double".to_string(),
            ast::Tipo::Decimal => "i8*".to_string(),
            ast::Tipo::Booleano => "i1".to_string(),
            ast::Tipo::Vazio => "void".to_string(),
            ast::Tipo::Enum(_) => "i32".to_string(),
            ast::Tipo::Classe(_) => self.map_type_to_llvm_ptr(tipo),
            ast::Tipo::Aplicado { .. } => self.map_type_to_llvm_ptr(tipo),
            ast::Tipo::Lista(_) => "%array*".to_string(),
            _ => panic!("Tipo LLVM não mapeado para argumento: {:?}", tipo),
        }
    }
}

impl<'a> LlvmGenerator<'a> {
    pub(crate) fn vtable_global_symbol(&self, fqn_class: &str) -> String {
        format!("@.vtable.{}", fqn_class.replace('.', "_"))
    }

    pub(crate) fn build_all_vtables(&mut self) {
        // Ordena por nome para determinismo
        let mut classes: Vec<String> = self.resolved_classes.keys().cloned().collect();
        classes.sort();
        for fqn in classes {
            let entries = self.compute_vtable_for(&fqn);
            // Índices
            let mut index = HashMap::new();
            for (i, (name, _)) in entries.iter().enumerate() {
                index.insert(name.clone(), i);
            }
            self.vtable_index.insert(fqn.to_string(), index);
            self.vtables.insert(fqn.to_string(), entries);
        }

        // Criar vtables para classes genéricas aplicadas (monomorfização)
        let mut applied_items: Vec<(String, Vec<ast::Tipo>)> = Vec::new();
        for (base_fqn, insts) in &self.applied_class_insts {
            for args in insts {
                applied_items.push((base_fqn.clone(), args.clone()));
            }
        }
        for (base_fqn, args) in applied_items {
            let mangled_name = self.mangle_aplicado_name(&base_fqn, &args);
            // Reusa a vtable da classe base para instanciações genéricas
            // (os métodos têm a mesma assinatura, apenas os tipos de campo mudam)
            if let Some(base_entries) = self.vtables.get(&base_fqn) {
                let mut index = HashMap::new();
                for (i, (name, _)) in base_entries.iter().enumerate() {
                    index.insert(name.clone(), i);
                }
                self.vtable_index.insert(mangled_name.clone(), index);
                self.vtables.insert(mangled_name, base_entries.clone());
            }
        }
    }

    pub(crate) fn compute_vtable_for(&self, fqn: &str) -> Vec<(String, String)> {
        // Começa com vtable do pai
        let mut result: Vec<(String, String)> = Vec::new();
        if let Some(info) = self.resolved_classes.get(fqn) {
            if let Some(parent_simple) = &info.parent_name {
                let parent_fqn = self
                    .type_checker
                    .resolver_nome_classe(parent_simple, &self.get_namespace_from_fqn(fqn));
                if self.resolved_classes.contains_key(&parent_fqn) {
                    result = self.compute_vtable_for(&parent_fqn);
                }
            }
        }
        // Métodos declarados nesta classe
        let decl = match self.type_checker.classes.get(fqn) {
            Some(d) => *d,
            None => return result,
        };
        for m in &decl.metodos {
            if m.eh_abstrato || m.eh_estatica {
                continue;
            }
            if m.eh_override || m.eh_virtual {
                // Se já existe no pai, substitui; senão, adiciona
                if let Some(pos) = result.iter().position(|(n, _)| n == &m.nome) {
                    result[pos] = (m.nome.clone(), fqn.to_string());
                } else if m.eh_virtual {
                    result.push((m.nome.clone(), fqn.to_string()));
                }
            }
        }
        result
    }

    pub(crate) fn map_string_to_llvm_type(&self, tipo_str: &str) -> String {
        let tipo_str_lower = tipo_str.to_lowercase();
        match tipo_str_lower.as_str() {
            "inteiro" | "i32" => "i32".to_string(),
            "texto" | "i8*" => "i8*".to_string(),
            "booleano" | "i1" => "i1".to_string(),
            "flutuante" | "float" => "float".to_string(),
            "duplo" | "double" => "double".to_string(),
            "vazio" | "void" | "" => "void".to_string(),
            _ => {
                if tipo_str.ends_with("[]") {
                    return "%array*".to_string();
                }
                let sanitized = tipo_str
                    .replace('.', "_")
                    .replace('<', "$")
                    .replace('>', "")
                    .replace(", ", "_");
                format!("%class.{}*", sanitized)
            }
        }
    }

    pub(crate) fn define_all_vtable_globals(&mut self) {
        let mut fqns: Vec<_> = self.vtables.keys().cloned().collect();
        fqns.sort();
        let local_classes = self.get_classes_in_ast();
        for fqn in fqns {
            let is_local = local_classes.contains(&fqn) || fqn.contains('$');
            let is_external = !is_local && self.type_checker.biblioteca_externa.is_some();
            let sym = self.vtable_global_symbol(&fqn);

            if is_external {
                self.header
                    .push_str(&format!("{0} = external global [0 x i8*], align 8\n", sym));
                continue;
            }

            let entries = self.vtables.get(&fqn).cloned().unwrap_or_default();
            let elems: Vec<String> = entries
                .iter()
                .map(|(metodo_nome, decl_cls)| {
                    // Símbolo LLVM do método declarado
                    let fun_sym = format!("{}::{}", decl_cls, metodo_nome).replace('.', "_");

                    // Descobre a assinatura exata do método na classe declarante
                    let metodo_decl = self
                        .type_checker
                        .classes
                        .get(decl_cls)
                        .and_then(|c| c.metodos.iter().find(|m| m.nome == *metodo_nome))
                        .unwrap_or_else(|| panic!(
                            "Método '{}' não encontrado em classe declarante '{}' ao construir vtable de '{}'",
                            metodo_nome, decl_cls, fqn
                        ));

                    // Resolve tipos no namespace da classe declarante
                    let decl_ns = self.get_namespace_from_fqn(decl_cls);
                    let ret_tipo_resolvido = self.resolve_type(
                        &metodo_decl
                            .tipo_retorno
                            .clone()
                            .unwrap_or(ast::Tipo::Vazio),
                        &decl_ns,
                    );
                    let ret_llvm = self.map_type_to_llvm_arg(&ret_tipo_resolvido);

                    // Primeiro parâmetro é o ponteiro para a classe declarante (self)
                    let self_ptr_ty = self.map_type_to_llvm_ptr(&ast::Tipo::Classe(decl_cls.clone()));
                    let mut params_llvm: Vec<String> = vec![self_ptr_ty];
                    for p in &metodo_decl.parametros {
                        let p_res = self.resolve_type(&p.tipo, &decl_ns);
                        params_llvm.push(self.map_type_to_llvm_arg(&p_res));
                    }
                    let params_sig = params_llvm.join(", ");

                    // Bitcast do ponteiro de função tipado para i8*
                    format!(
                        "i8* bitcast ({ret} ({params})* @\"{sym}\" to i8*)",
                        ret = ret_llvm,
                        params = params_sig,
                        sym = fun_sym
                    )
                })
                .collect();
            // Caso sem entradas, cria um array vazio de i8*
            let count = elems.len();
            let array_elems = if count == 0 {
                String::new()
            } else {
                elems.join(", ")
            };
            self.header.push_str(&format!(
                "{0} = global [{1} x i8*] [ {2} ], align 8\n",
                sym, count, array_elems
            ));
        }
    }

}
