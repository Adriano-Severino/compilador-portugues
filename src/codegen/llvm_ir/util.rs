use crate::ast;
use std::collections::{HashMap, HashSet};
use crate::library_loader::LibSimbolo;

use super::LlvmGenerator;

impl<'a> LlvmGenerator<'a> {
    pub(crate) fn mangle_tipo_simple(&self, t: &ast::Tipo) -> String {
        match t {
            ast::Tipo::Booleano => "bool".to_string(),
            ast::Tipo::Inteiro => "int".to_string(),
            ast::Tipo::Flutuante => "f32".to_string(),
            ast::Tipo::Duplo => "f64".to_string(),
            ast::Tipo::Decimal => "dec".to_string(),
            ast::Tipo::Texto => "texto".to_string(),
            ast::Tipo::Vazio => "vazio".to_string(),
            ast::Tipo::Enum(n) => n.replace('.', "_"),
            ast::Tipo::Classe(n) => n.replace('.', "_"),
            ast::Tipo::Aplicado { nome, args } => {
                let base = nome.replace('.', "_");
                let parts: Vec<String> = args.iter().map(|a| self.mangle_tipo_simple(a)).collect();
                format!("{0}${1}", base, parts.join("_"))
            }
            ast::Tipo::Lista(elem) => format!("lista${}", self.mangle_tipo_simple(elem)),
            ast::Tipo::Opcional(inner) => format!("opt${}", self.mangle_tipo_simple(inner)),
            ast::Tipo::Funcao(params, ret) => {
                let mut v: Vec<String> =
                    params.iter().map(|p| self.mangle_tipo_simple(p)).collect();
                v.push(self.mangle_tipo_simple(ret));
                format!("fn${}", v.join("_"))
            }
            _ => "t".to_string(),
        }
    }

    pub(crate) fn mangle_aplicado_name(&self, base_fqn: &str, args: &[ast::Tipo]) -> String {
        let mangled_args: Vec<String> = args.iter().map(|a| self.mangle_tipo_simple(a)).collect();
        if mangled_args.is_empty() {
            base_fqn.to_string()
        } else {
            format!("{0}${1}", base_fqn, mangled_args.join("_"))
        }
    }

    // Substituição local de genéricos por tipos concretos (para layout especializado)
    pub(crate) fn subst_generics_local(&self, t: &ast::Tipo, subst: &HashMap<String, ast::Tipo>) -> ast::Tipo {
        use ast::Tipo::*;
        match t {
            Generico(n) => subst.get(n).cloned().unwrap_or_else(|| t.clone()),
            Classe(n) => subst.get(n).cloned().unwrap_or_else(|| t.clone()),
            Lista(inner) => Lista(Box::new(self.subst_generics_local(inner, subst))),
            Opcional(inner) => Opcional(Box::new(self.subst_generics_local(inner, subst))),
            Aplicado { nome, args } => {
                let novos: Vec<ast::Tipo> = args
                    .iter()
                    .map(|a| self.subst_generics_local(a, subst))
                    .collect();
                Aplicado {
                    nome: nome.clone(),
                    args: novos,
                }
            }
            Funcao(params, ret) => {
                let p2: Vec<ast::Tipo> = params
                    .iter()
                    .map(|p| self.subst_generics_local(p, subst))
                    .collect();
                let r2 = self.subst_generics_local(ret, subst);
                Funcao(p2, Box::new(r2))
            }
            _ => t.clone(),
        }
    }

    pub(crate) fn collect_applied_instantiations(&mut self) {
        // Varre AST para encontrar todos os Tipos::Aplicado usados e registra em maps
        pub(crate) fn collect_in_tipo<'a>(this: &mut LlvmGenerator<'a>, tipo: &ast::Tipo, ns: &str) {
            if let ast::Tipo::Aplicado { nome, args } = tipo {
                // Tenta resolver como classe ou interface
                let fqn_cls = this.type_checker.resolver_nome_classe(nome, ns);
                let fqn_iface = this.type_checker.resolver_nome_interface(nome, ns);
                let mut norm_args: Vec<ast::Tipo> = Vec::new();
                for a in args {
                    // resolve recursivamente nomes dentro de args (mínimo)
                    norm_args.push(this.resolve_type(a, ns));
                }
                if this.type_checker.classes.contains_key(&fqn_cls) {
                    this.applied_class_insts
                        .entry(fqn_cls.clone())
                        .or_default()
                        .push(norm_args);
                } else if this.type_checker.interfaces.contains_key(&fqn_iface) {
                    this.applied_iface_insts
                        .entry(fqn_iface.clone())
                        .or_default()
                        .push(norm_args);
                }
            }
            // Descer nos filhos
            match tipo {
                ast::Tipo::Lista(inner) | ast::Tipo::Opcional(inner) => {
                    collect_in_tipo(this, inner, ns)
                }
                ast::Tipo::Aplicado { args, .. } => {
                    for a in args {
                        collect_in_tipo(this, a, ns)
                    }
                }
                ast::Tipo::Funcao(params, ret) => {
                    for p in params {
                        collect_in_tipo(this, p, ns);
                    }
                    collect_in_tipo(this, ret, ns);
                }
                _ => {}
            }
        }

        pub(crate) fn collect_in_decl<'a>(this: &mut LlvmGenerator<'a>, decl: &'a ast::Declaracao, ns: &str) {
            match decl {
                ast::Declaracao::DeclaracaoClasse(c) => {
                    if let Some(tp) = &c.classe_pai {
                        collect_in_tipo(this, tp, ns);
                    }
                    for i in &c.interfaces {
                        collect_in_tipo(this, i, ns);
                    }
                    for f in &c.campos {
                        collect_in_tipo(this, &f.tipo, ns);
                    }
                    for p in &c.propriedades {
                        collect_in_tipo(this, &p.tipo, ns);
                    }
                    for m in &c.metodos {
                        if let Some(ret) = &m.tipo_retorno {
                            collect_in_tipo(this, ret, ns);
                        }
                        for p in &m.parametros {
                            collect_in_tipo(this, &p.tipo, ns);
                        }
                    }
                }
                ast::Declaracao::DeclaracaoInterface(i) => {
                    for m in &i.metodos {
                        if let Some(ret) = &m.tipo_retorno {
                            collect_in_tipo(this, ret, ns);
                        }
                        for p in &m.parametros {
                            collect_in_tipo(this, &p.tipo, ns);
                        }
                    }
                }
                ast::Declaracao::DeclaracaoFuncao(f) => {
                    if let Some(ret) = &f.tipo_retorno {
                        collect_in_tipo(this, ret, ns);
                    }
                    for p in &f.parametros {
                        collect_in_tipo(this, &p.tipo, ns);
                    }
                }
                _ => {}
            }
        }

        // raiz
        for d in &self.programa.declaracoes {
            collect_in_decl(self, d, "");
        }
        // namespaces
        for ns in &self.programa.namespaces {
            for d in &ns.declaracoes {
                collect_in_decl(self, d, &ns.nome);
            }
        }

        // Deduplica args iguais
        pub(crate) fn dedup(map: &mut HashMap<String, Vec<Vec<ast::Tipo>>>) {
            for (_k, v) in map.iter_mut() {
                let mut uniq: Vec<Vec<ast::Tipo>> = Vec::new();
                'outer: for args in v.drain(..) {
                    if uniq.iter().any(|e| e == &args) {
                        continue 'outer;
                    }
                    uniq.push(args);
                }
                *v = uniq;
            }
        }
        dedup(&mut self.applied_class_insts);
        dedup(&mut self.applied_iface_insts);
    }

    pub(crate) fn find_principal_function_fqn(&self) -> Option<String> {
        // Procura no escopo global
        for decl in &self.programa.declaracoes {
            if let ast::Declaracao::DeclaracaoFuncao(func) = decl {
                if func.nome == "Principal" {
                    // No global, FQN é apenas o nome simples
                    return Some("Principal".to_string());
                }
            }
        }
        // Procura dentro dos namespaces e retorna o FQN
        for ns in &self.programa.namespaces {
            for decl in &ns.declaracoes {
                if let ast::Declaracao::DeclaracaoFuncao(func) = decl {
                    if func.nome == "Principal" {
                        return Some(format!("{}.{}", ns.nome, func.nome));
                    }
                }
            }
        }
        None
    }

    pub(crate) fn define_static_globals(&mut self) {
        // Varre todas as classes (globais e em namespaces) e cria globais LLVM para membros estáticos com inicialização simples
        // Suporta: inteiro/booleano; demais tipos usam zeroinitializer
        pub(crate) fn process_class<'a>(
            this: &mut LlvmGenerator<'a>,
            fqn: &str,
            class: &'a ast::DeclaracaoClasse,
        ) {
            // Campos estáticos
            for campo in &class.campos {
                if campo.eh_estatica {
                    let sym = this.static_global_symbol(fqn, &campo.nome);
                    let ty = this.map_type_to_llvm_storage(&campo.tipo);
                    if let Some(init) = &campo.valor_inicial {
                        if let Some((val, _)) = this.const_llvm_init_for_expr(init, &campo.tipo) {
                            this.header.push_str(&format!(
                                "{0} = global {1} {2}, align 4\n",
                                sym, ty, val
                            ));
                        } else {
                            this.header.push_str(&format!(
                                "{0} = global {1} zeroinitializer, align 4\n",
                                sym, ty
                            ));
                        }
                    } else {
                        this.header.push_str(&format!(
                            "{0} = global {1} zeroinitializer, align 4\n",
                            sym, ty
                        ));
                    }
                }
            }
            // Propriedades estáticas com valor_inicial
            for prop in &class.propriedades {
                if prop.eh_estatica {
                    let sym = this.static_global_symbol(fqn, &prop.nome);
                    let ty = this.map_type_to_llvm_storage(&prop.tipo);
                    if let Some(init) = &prop.valor_inicial {
                        if let Some((val, _)) = this.const_llvm_init_for_expr(init, &prop.tipo) {
                            this.header.push_str(&format!(
                                "{0} = global {1} {2}, align 4\n",
                                sym, ty, val
                            ));
                        } else {
                            this.header.push_str(&format!(
                                "{0} = global {1} zeroinitializer, align 4\n",
                                sym, ty
                            ));
                        }
                    } else {
                        this.header.push_str(&format!(
                            "{0} = global {1} zeroinitializer, align 4\n",
                            sym, ty
                        ));
                    }
                }
            }
        }

        for decl in &self.programa.declaracoes {
            if let ast::Declaracao::DeclaracaoClasse(class) = decl {
                let fqn = class.nome.clone();
                process_class(self, &fqn, class);
            }
        }
        for ns in &self.programa.namespaces {
            for decl in &ns.declaracoes {
                if let ast::Declaracao::DeclaracaoClasse(class) = decl {
                    let fqn = format!("{}.{}", ns.nome, class.nome);
                    process_class(self, &fqn, class);
                }
            }
        }
    }

    pub(crate) fn static_global_symbol(&self, fqn_class: &str, member: &str) -> String {
        let suffix = format!(".static.{}.{}", fqn_class.replace('.', "_"), member);
        let mut s = String::from("@");
        s.push_str(&suffix);
        s
    }

    pub(crate) fn infer_member_type(&self, fqn_class: &str, member: &str) -> Option<ast::Tipo> {
        if let Some(info) = self.resolved_classes.get(fqn_class) {
            if let Some(f) = info.fields.iter().find(|f| f.nome == member) {
                return Some(f.tipo.clone());
            }
            if let Some(p) = info.properties.iter().find(|p| p.nome == member) {
                return Some(p.tipo.clone());
            }
        }
        None
    }

    pub(crate) fn setup_parameters(&mut self, params: &[ast::Parametro]) {
        for param in params {
            let ptr_reg = format!("%var.{0}", param.nome);
            let var_type = self.resolve_type(&param.tipo, &self.namespace_path);
            let llvm_type = self.map_type_to_llvm_storage(&var_type);
            let align = self.get_type_alignment(&var_type);

            self.body.push_str(&format!(
                "  {0} = alloca {1}, align {2}\n",
                ptr_reg, llvm_type, align
            ));

            let param_reg = format!("%param.{0}", param.nome);
            self.body.push_str(&format!(
                "  store {0} {1}, {0}* {2}\n",
                llvm_type, param_reg, ptr_reg
            ));

            self.variables
                .insert(param.nome.to_string(), (ptr_reg, var_type));
        }
    }

    pub(crate) fn get_type_alignment(&self, var_type: &ast::Tipo) -> u32 {
        match var_type {
            ast::Tipo::Inteiro => 4,
            ast::Tipo::Texto => 8,
            ast::Tipo::Flutuante => 4,
            ast::Tipo::Duplo => 8,
            ast::Tipo::Decimal => 8,
            ast::Tipo::Booleano => 1,
            ast::Tipo::Enum(_) => 4,
            ast::Tipo::Classe(_) => 8,
            ast::Tipo::Lista(_) => 8,
            _ => 8,
        }
    }

    pub(crate) fn get_unique_temp_name(&mut self) -> String {
        let name = format!("%tmp.{0}", self.temp_counter);
        self.temp_counter += 1;
        name
    }

    pub(crate) fn get_unique_label(&mut self, prefix: &str) -> String {
        let label = format!("{0}.{1}", prefix, self.temp_counter);
        self.temp_counter += 1;
        label
    }

    pub(crate) fn get_namespace_from_fqn(&self, full: &str) -> String {
        full.rsplit_once('.')
            .map(|(ns, _)| ns.to_string())
            .unwrap_or_default()
    }

    pub(crate) fn get_namespace_from_full_name(&self, full: &str) -> String {
        self.get_namespace_from_fqn(full)
    }

    // Helpers para arrays
    pub(crate) fn get_array_data_and_len(&mut self, arr_ptr_reg: &str) -> (String, String) {
        // arr_ptr_reg: %array*
        let len_ptr = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {0} = getelementptr inbounds %array, %array* {1}, i32 0, i32 0\n",
            len_ptr, arr_ptr_reg
        ));
        let len_reg = self.get_unique_temp_name();
        self.body
            .push_str(&format!("  {0} = load i32, i32* {1}\n", len_reg, len_ptr));
        let data_ptr_ptr = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {0} = getelementptr inbounds %array, %array* {1}, i32 0, i32 1\n",
            data_ptr_ptr, arr_ptr_reg
        ));
        let data_ptr = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {0} = load i8*, i8** {1}\n",
            data_ptr, data_ptr_ptr
        ));
        (data_ptr, len_reg)
    }

    pub(crate) fn zero_value_of(&mut self, tipo: &ast::Tipo) -> String {
        match tipo {
            ast::Tipo::Inteiro | ast::Tipo::Enum(_) => "0".to_string(),
            ast::Tipo::Booleano => "0".to_string(),
            ast::Tipo::Flutuante => {
                let z = self.get_unique_temp_name();
                self.body
                    .push_str(&format!("  {0} = fptrunc double 0.0 to float\n", z));
                z
            }
            ast::Tipo::Duplo => "0.0".to_string(),
            ast::Tipo::Texto | ast::Tipo::Decimal | ast::Tipo::Classe(_) | ast::Tipo::Lista(_) => {
                "null".to_string()
            }
            _ => "0".to_string(),
        }
    }
}
