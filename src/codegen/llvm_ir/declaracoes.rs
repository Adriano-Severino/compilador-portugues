use crate::ast;
use std::collections::{HashMap, HashSet};
use crate::library_loader::LibSimbolo;

use super::LlvmGenerator;

impl<'a> LlvmGenerator<'a> {
    pub(crate) fn generate_applied_class_methods(&mut self) {
        // Para cada instanciação de classe genérica coletada, gera métodos especializados
        let mut applied_items: Vec<(String, Vec<ast::Tipo>)> = Vec::new();
        for (base_fqn, insts) in &self.applied_class_insts {
            for args in insts {
                applied_items.push((base_fqn.clone(), args.clone()));
            }
        }

        for (base_fqn, args) in applied_items {
            let class_decl = match self.type_checker.classes.get(&base_fqn) {
                Some(c) => *c,
                None => continue,
            };

            if class_decl.generic_params.is_empty() {
                continue;
            }

            let mangled_name = self.mangle_aplicado_name(&base_fqn, &args);
            let old_namespace = self.namespace_path.clone();
            let old_classe_atual = self.classe_atual.clone();

            // Configurar contexto para a classe aplicada
            self.classe_atual = Some(mangled_name.clone());
            self.namespace_path = self.get_namespace_from_fqn(&base_fqn);

            // Gerar métodos para a instanciação
            for metodo in &class_decl.metodos {
                if metodo.eh_abstrato {
                    continue;
                }
                self.generate_applied_metodo(metodo, &mangled_name, &base_fqn, &args);
            }

            // Gerar construtores para a instanciação
            for construtor in &class_decl.construtores {
                self.generate_applied_construtor(construtor, &mangled_name, &base_fqn, &args);
            }

            // Restaurar contexto
            self.namespace_path = old_namespace;
            self.classe_atual = old_classe_atual;
        }
    }

    pub(crate) fn generate_applied_metodo(
        &mut self,
        metodo: &'a ast::MetodoClasse,
        mangled_name: &str,
        base_fqn: &str,
        args: &Vec<ast::Tipo>,
    ) {
        // Monta substituição de tipos genéricos
        let class_decl = match self.type_checker.classes.get(base_fqn) {
            Some(c) => *c,
            None => return,
        };

        let mut subst: HashMap<String, ast::Tipo> = HashMap::new();
        for (g, a) in class_decl.generic_params.iter().zip(args.iter()) {
            subst.insert(g.clone(), a.clone());
        }

        let namespace = self.get_namespace_from_fqn(base_fqn);
        let nome_metodo = format!("{0}::{1}", mangled_name, metodo.nome).replace('.', "_");

        // Resolver tipo de retorno com substituição
        let tipo_retorno_base = metodo.tipo_retorno.clone().unwrap_or(ast::Tipo::Vazio);
        let tipo_retorno_subst = self.subst_generics_local(&tipo_retorno_base, &subst);
        let tipo_retorno_resolvido = self.resolve_type(&tipo_retorno_subst, &namespace);
        let tipo_retorno_llvm = self.map_type_to_llvm_arg(&tipo_retorno_resolvido);

        let mut params_llvm = Vec::new();
        let self_type = self.map_type_to_llvm_ptr(&ast::Tipo::Classe(mangled_name.to_string()));
        params_llvm.push(format!("{0} %param.self", self_type));

        for param in &metodo.parametros {
            let tipo_param_subst = self.subst_generics_local(&param.tipo, &subst);
            let tipo_param_resolvido = self.resolve_type(&tipo_param_subst, &namespace);
            let tipo_param_llvm = self.map_type_to_llvm_arg(&tipo_param_resolvido);
            params_llvm.push(format!("{0} %param.{1}", tipo_param_llvm, param.nome));
        }

        let mut old_body = self.body.clone();
        let old_vars = self.variables.clone();
        self.body = String::new();
        self.variables.clear();

        self.body.push_str(&format!(
            "define {0} @\"{1}\"({2}) {{ \n",
            tipo_retorno_llvm,
            nome_metodo,
            params_llvm.join(", ")
        ));
        self.body.push_str("entry:\n");

        let self_ptr_reg = "%var.self".to_string();
        self.body.push_str(&format!(
            "  {0} = alloca {1}, align 8\n",
            self_ptr_reg, self_type
        ));
        self.body.push_str(&format!(
            "  store {0} %param.self, {0}* {1}\n",
            self_type, self_ptr_reg
        ));
        self.variables.insert(
            "self".to_string(),
            (self_ptr_reg, ast::Tipo::Classe(mangled_name.to_string())),
        );

        // Configurar parâmetros com tipos substituídos
        for (_i, param) in metodo.parametros.iter().enumerate() {
            let tipo_param_subst = self.subst_generics_local(&param.tipo, &subst);
            let tipo_param_resolvido = self.resolve_type(&tipo_param_subst, &namespace);
            let ptr_reg = format!("%var.{}", param.nome);
            let llvm_type = self.map_type_to_llvm_storage(&tipo_param_resolvido);
            let align = self.get_type_alignment(&tipo_param_resolvido);

            self.body.push_str(&format!(
                "  {0} = alloca {1}, align {2}\n",
                ptr_reg, llvm_type, align
            ));
            let param_reg = format!("%param.{}", param.nome);
            self.body.push_str(&format!(
                "  store {0} {1}, {0}* {2}\n",
                llvm_type, param_reg, ptr_reg
            ));
            self.variables
                .insert(param.nome.clone(), (ptr_reg, tipo_param_resolvido));
        }

        // Gerar corpo do método (reutilizando a lógica existente)
        for comando in &metodo.corpo {
            self.generate_comando(comando);
        }

        let last_instruction = self.body.trim().lines().last().unwrap_or("").trim();
        if !last_instruction.starts_with("ret") && !last_instruction.starts_with("unreachable") {
            if metodo.tipo_retorno.is_none() || metodo.tipo_retorno == Some(ast::Tipo::Vazio) {
                self.body.push_str("  ret void\n");
            } else {
                self.body.push_str(&format!(
                    "  unreachable ; O método '{0}' deve ter um retorno\n",
                    metodo.nome
                ));
            }
        }

        self.body.push_str("}\n");
        old_body.push_str(&self.body);
        self.body = old_body;
        self.variables = old_vars;
    }

    pub(crate) fn generate_applied_construtor(
        &mut self,
        construtor: &'a ast::ConstrutorClasse,
        mangled_name: &str,
        base_fqn: &str,
        args: &Vec<ast::Tipo>,
    ) {
        // Monta substituição de tipos genéricos
        let class_decl = match self.type_checker.classes.get(base_fqn) {
            Some(c) => *c,
            None => return,
        };

        let mut subst: HashMap<String, ast::Tipo> = HashMap::new();
        for (g, a) in class_decl.generic_params.iter().zip(args.iter()) {
            subst.insert(g.clone(), a.clone());
        }

        let namespace = self.get_namespace_from_fqn(base_fqn);
        let total_params = construtor.parametros.len();
        let nome_ctor =
            format!("{0}::construtor${1}", mangled_name, total_params).replace('.', "_");

        let tipo_retorno_llvm = "void".to_string();

        let mut params_llvm = Vec::new();
        let self_type = self.map_type_to_llvm_ptr(&ast::Tipo::Classe(mangled_name.to_string()));
        params_llvm.push(format!("{0} %param.self", self_type));

        for param in &construtor.parametros {
            let tipo_param_subst = self.subst_generics_local(&param.tipo, &subst);
            let tipo_param_resolvido = self.resolve_type(&tipo_param_subst, &namespace);
            let tipo_param_llvm = self.map_type_to_llvm_arg(&tipo_param_resolvido);
            params_llvm.push(format!("{0} %param.{1}", tipo_param_llvm, param.nome));
        }

        let mut old_body = self.body.clone();
        let old_vars = self.variables.clone();
        self.body = String::new();
        self.variables.clear();

        self.body.push_str(&format!(
            "define {0} @\"{1}\"({2}) {{ \n",
            tipo_retorno_llvm,
            nome_ctor,
            params_llvm.join(", ")
        ));
        self.body.push_str("entry:\n");

        // Aloca e armazena self
        let self_ptr_reg = "%var.self".to_string();
        self.body.push_str(&format!(
            "  {0} = alloca {1}, align 8\n",
            self_ptr_reg, self_type
        ));
        self.body.push_str(&format!(
            "  store {0} %param.self, {0}* {1}\n",
            self_type, self_ptr_reg
        ));
        self.variables.insert(
            "self".to_string(),
            (self_ptr_reg, ast::Tipo::Classe(mangled_name.to_string())),
        );

        // Configurar parâmetros com tipos substituídos
        for param in &construtor.parametros {
            let tipo_param_subst = self.subst_generics_local(&param.tipo, &subst);
            let tipo_param_resolvido = self.resolve_type(&tipo_param_subst, &namespace);
            let ptr_reg = format!("%var.{}", param.nome);
            let llvm_type = self.map_type_to_llvm_storage(&tipo_param_resolvido);
            let align = self.get_type_alignment(&tipo_param_resolvido);

            self.body.push_str(&format!(
                "  {0} = alloca {1}, align {2}\n",
                ptr_reg, llvm_type, align
            ));
            let param_reg = format!("%param.{}", param.nome);
            self.body.push_str(&format!(
                "  store {0} {1}, {0}* {2}\n",
                llvm_type, param_reg, ptr_reg
            ));
            self.variables
                .insert(param.nome.clone(), (ptr_reg, tipo_param_resolvido));
        }

        // Gerar corpo do construtor (reutilizando a lógica existente)
        for comando in &construtor.corpo {
            self.generate_comando(comando);
        }

        self.body.push_str("  ret void\n");
        self.body.push_str("}\n");
        old_body.push_str(&self.body);
        self.body = old_body;
        self.variables = old_vars;
    }

    pub(crate) fn generate_namespace_definitions(&mut self, ns: &'a ast::DeclaracaoNamespace) {
        let old_namespace = self.namespace_path.clone();
        self.namespace_path = if old_namespace.is_empty() {
            ns.nome.clone()
        } else {
            format!("{}.{}", old_namespace, ns.nome)
        };

        for decl in &ns.declaracoes {
            match decl {
                ast::Declaracao::DeclaracaoFuncao(func) => {
                    self.generate_funcao(func, &self.namespace_path.clone());
                }
                ast::Declaracao::DeclaracaoClasse(class) => {
                    self.generate_classe_definitions(class, &self.namespace_path.clone());
                }
                _ => {}
            }
        }

        self.namespace_path = old_namespace;
    }

    pub(crate) fn generate_classe_definitions(&mut self, class: &'a ast::DeclaracaoClasse, namespace: &str) {
        if !class.generic_params.is_empty() {
            return;
        }

        let fqn = if namespace.is_empty() {
            class.nome.clone()
        } else {
            format!("{}.{}", namespace, class.nome)
        };
        self.classe_atual = Some(fqn);
        // Métodos (pula abstratos)
        for metodo in &class.metodos {
            if metodo.eh_abstrato {
                continue;
            }
            self.generate_metodo(metodo);
        }
        // Construtores
        for construtor in &class.construtores {
            self.generate_construtor(construtor);
        }
        self.classe_atual = None;
    }

    pub(crate) fn generate_construtor(&mut self, construtor: &'a ast::ConstrutorClasse) {
        let classe_nome = self.classe_atual.as_ref().unwrap().clone();
        let namespace = classe_nome.rsplit_once('.').map_or("", |(ns, _)| ns);
        let total_params = construtor.parametros.len();
        let nome_ctor = format!("{0}::construtor${1}", classe_nome, total_params).replace('.', "_");

        let tipo_retorno_llvm = "void".to_string();

        let mut params_llvm = Vec::new();
        let self_type = self.map_type_to_llvm_ptr(&ast::Tipo::Classe(classe_nome.clone()));
        params_llvm.push(format!("{0} %param.self", self_type));

        for param in &construtor.parametros {
            let tipo_param_resolvido = self.resolve_type(&param.tipo, namespace);
            let tipo_param_llvm = self.map_type_to_llvm_arg(&tipo_param_resolvido);
            params_llvm.push(format!("{0} %param.{1}", tipo_param_llvm, param.nome));
        }

        let mut old_body = self.body.clone();
        let old_vars = self.variables.clone();
        self.body = String::new();
        self.variables.clear();

        self.body.push_str(&format!(
            "define {0} @\"{1}\"({2}) {{ \n",
            tipo_retorno_llvm,
            nome_ctor,
            params_llvm.join(", ")
        ));
        self.body.push_str("entry:\n");

        // Aloca e armazena self
        let self_ptr_reg = "%var.self".to_string();
        self.body.push_str(&format!(
            "  {0} = alloca {1}, align 8\n",
            self_ptr_reg, self_type
        ));
        self.body.push_str(&format!(
            "  store {0} %param.self, {0}* {1}\n",
            self_type, self_ptr_reg
        ));
        self.variables.insert(
            "self".to_string(),
            (self_ptr_reg, ast::Tipo::Classe(classe_nome.clone())),
        );

        // Parâmetros do construtor
        self.setup_parameters(&construtor.parametros);

        // Se houver chamada explícita ao construtor da classe base, emita-a antes do corpo
        if let Some(args_pai) = &construtor.chamada_pai {
            // Descobre a classe base (FQN)
            let classe_decl_atual = self
                .type_checker
                .classes
                .get(&classe_nome)
                .expect("Declaração da classe atual não encontrada");
            if let Some(nome_base_simples) = &classe_decl_atual.classe_pai {
                let base_name = match nome_base_simples {
                    ast::Tipo::Classe(n) => n.as_str(),
                    ast::Tipo::Aplicado { nome, .. } => nome.as_str(),
                    _ => "",
                };
                let parent_fqn = self.type_checker.resolver_nome_classe(base_name, namespace);

                if let Some(parent_decl) = self.type_checker.classes.get(&parent_fqn) {
                    // Seleciona o melhor construtor do pai com base em argumentos fornecidos + defaults
                    let mut escolhido: Option<&ast::ConstrutorClasse> = None;
                    let mut melhor_total = 0usize;
                    for ctor in &parent_decl.construtores {
                        let total = ctor.parametros.len();
                        let obrig = ctor
                            .parametros
                            .iter()
                            .filter(|p| p.valor_padrao.is_none())
                            .count();
                        let fornecidos = args_pai.len();
                        if fornecidos >= obrig && fornecidos <= total {
                            if total >= melhor_total {
                                melhor_total = total;
                                escolhido = Some(ctor);
                            }
                        }
                    }

                    if let Some(ctor_pai) = escolhido {
                        // Prepara lista final de argumentos (com defaults quando necessário)
                        let fornecidos = args_pai.len();
                        let mut final_args: Vec<(String, ast::Tipo)> = Vec::new();
                        for (idx, param) in ctor_pai.parametros.iter().enumerate() {
                            if idx < fornecidos {
                                final_args.push(self.generate_expressao(&args_pai[idx]));
                            } else if let Some(def_expr) = &param.valor_padrao {
                                final_args.push(self.generate_expressao(def_expr));
                            } else {
                                panic!(
                                    "Argumento obrigatório ausente para parâmetro '{}' do construtor base de '{}'",
                                    param.nome, parent_fqn
                                );
                            }
                        }

                        // Carrega 'self' atual e faz bitcast para ponteiro do tipo da classe base
                        let (self_alloca, self_tipo) = self
                            .variables
                            .get("self")
                            .cloned()
                            .expect("Variável self não encontrada no construtor");
                        let self_loaded = self.get_unique_temp_name();
                        let self_ptr_ty = self.map_type_to_llvm_ptr(&self_tipo);
                        self.body.push_str(&format!(
                            "  {0} = load {1}, {1}* {2}\n",
                            self_loaded, self_ptr_ty, self_alloca
                        ));

                        let base_ptr_ty =
                            self.map_type_to_llvm_ptr(&ast::Tipo::Classe(parent_fqn.clone()));
                        let self_as_base = self.get_unique_temp_name();
                        self.body.push_str(&format!(
                            "  {0} = bitcast {1} {2} to {3}\n",
                            self_as_base, self_ptr_ty, self_loaded, base_ptr_ty
                        ));

                        // Monta chamada ao construtor base
                        let func_name =
                            format!("{0}::construtor${1}", parent_fqn, ctor_pai.parametros.len())
                                .replace('.', "_");

                        let mut args_llvm = Vec::new();
                        args_llvm.push(format!("{0} {1}", base_ptr_ty, self_as_base));
                        for (reg, ty) in &final_args {
                            let llvm_ty = self.map_type_to_llvm_arg(ty);
                            args_llvm.push(format!("{0} {1}", llvm_ty, reg));
                        }
                        self.body.push_str(&format!(
                            "  call void @\"{0}\"({1})\n",
                            func_name,
                            args_llvm.join(", ")
                        ));
                    }
                }
            }
        }

        // Corpo do construtor
        for comando in &construtor.corpo {
            self.generate_comando(comando);
        }

        // Retorno implícito
        let last_instruction = self.body.trim().lines().last().unwrap_or("").trim();
        if !last_instruction.starts_with("ret") && !last_instruction.starts_with("unreachable") {
            self.body.push_str("  ret void\n");
        }

        self.body.push_str("}\n");
        old_body.push_str(&self.body);
        self.body = old_body;
        self.variables = old_vars;
    }

    pub(crate) fn generate_funcao(&mut self, func: &'a ast::DeclaracaoFuncao, namespace: &str) {
        let nome_funcao = self
            .type_checker
            .resolver_nome_funcao(&func.nome, namespace)
            .replace('.', "_");
        let tipo_retorno_resolvido = self.resolve_type(
            &func.tipo_retorno.clone().unwrap_or(ast::Tipo::Vazio),
            namespace,
        );
        let tipo_retorno_llvm = self.map_type_to_llvm_arg(&tipo_retorno_resolvido);

        let mut params_llvm = Vec::new();
        for param in &func.parametros {
            let tipo_param_resolvido = self.resolve_type(&param.tipo, namespace);
            let tipo_param_llvm = self.map_type_to_llvm_arg(&tipo_param_resolvido);
            params_llvm.push(format!("{0} %param.{1}", tipo_param_llvm, param.nome));
        }

        let mut old_body = self.body.clone();
        let old_vars = self.variables.clone();
        self.body = String::new();
        self.variables.clear();

        self.body.push_str(&format!(
            "define {0} @\"{1}\"({2}) {{ \n",
            tipo_retorno_llvm,
            nome_funcao,
            params_llvm.join(", ")
        ));
        self.body.push_str("entry:\n");

        self.setup_parameters(&func.parametros);

        for comando in &func.corpo {
            self.generate_comando(comando);
        }

        let last_instruction = self.body.trim().lines().last().unwrap_or("").trim();
        if !last_instruction.starts_with("ret") && !last_instruction.starts_with("unreachable") {
            if func.tipo_retorno.is_none() || func.tipo_retorno == Some(ast::Tipo::Vazio) {
                self.body.push_str("  ret void\n");
            } else {
                self.body.push_str(&format!(
                    "  unreachable ; A função '{0}' deve ter um retorno\n",
                    func.nome
                ));
            }
        }

        self.body.push_str("}\n");
        old_body.push_str(&self.body);

        self.body = old_body;
        self.variables = old_vars;
    }

    pub(crate) fn generate_metodo(&mut self, metodo: &'a ast::MetodoClasse) {
        let classe_nome = self.classe_atual.as_ref().unwrap();
        let namespace = classe_nome.rsplit_once('.').map_or("", |(ns, _)| ns);
        let nome_metodo = format!("{0}::{1}", classe_nome, metodo.nome).replace('.', "_");

        let tipo_retorno_resolvido = self.resolve_type(
            &metodo.tipo_retorno.clone().unwrap_or(ast::Tipo::Vazio),
            namespace,
        );
        let tipo_retorno_llvm = self.map_type_to_llvm_arg(&tipo_retorno_resolvido);

        let mut params_llvm = Vec::new();
        let self_type = self.map_type_to_llvm_ptr(&ast::Tipo::Classe(classe_nome.clone()));
        params_llvm.push(format!("{0} %param.self", self_type));

        for param in &metodo.parametros {
            let tipo_param_resolvido = self.resolve_type(&param.tipo, namespace);
            let tipo_param_llvm = self.map_type_to_llvm_arg(&tipo_param_resolvido);
            params_llvm.push(format!("{0} %param.{1}", tipo_param_llvm, param.nome));
        }

        let mut old_body = self.body.clone();
        let old_vars = self.variables.clone();
        self.body = String::new();
        self.variables.clear();

        self.body.push_str(&format!(
            "define {0} @\"{1}\"({2}) {{ \n",
            tipo_retorno_llvm,
            nome_metodo,
            params_llvm.join(", ")
        ));
        self.body.push_str("entry:\n");

        let self_ptr_reg = "%var.self".to_string();
        self.body.push_str(&format!(
            "  {0} = alloca {1}, align 8\n",
            self_ptr_reg, self_type
        ));
        self.body.push_str(&format!(
            "  store {0} %param.self, {0}* {1}\n",
            self_type, self_ptr_reg
        ));
        self.variables.insert(
            "self".to_string(),
            (self_ptr_reg, ast::Tipo::Classe(classe_nome.clone())),
        );

        self.setup_parameters(&metodo.parametros);

        for comando in &metodo.corpo {
            self.generate_comando(comando);
        }

        let last_instruction = self.body.trim().lines().last().unwrap_or("").trim();
        if !last_instruction.starts_with("ret") && !last_instruction.starts_with("unreachable") {
            if metodo.tipo_retorno.is_none() || metodo.tipo_retorno == Some(ast::Tipo::Vazio) {
                self.body.push_str("  ret void\n");
            } else {
                self.body.push_str(&format!(
                    "  unreachable ; O método '{0}' deve ter um retorno\n",
                    metodo.nome
                ));
            }
        }

        self.body.push_str("}\n");
        old_body.push_str(&self.body);
        self.body = old_body;
        self.variables = old_vars;
    }

    pub(crate) fn get_classes_in_ast(&self) -> std::collections::HashSet<String> {
        let mut local_classes = std::collections::HashSet::new();
        for ns in &self.programa.namespaces {
            for decl in &ns.declaracoes {
                if let ast::Declaracao::DeclaracaoClasse(c) = decl {
                    local_classes.insert(format!("{}.{}", ns.nome, c.nome));
                }
            }
        }
        for decl in &self.programa.declaracoes {
            if let ast::Declaracao::DeclaracaoClasse(c) = decl {
                local_classes.insert(c.nome.clone());
            }
        }
        local_classes
    }

}
