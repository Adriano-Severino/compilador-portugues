use crate::ast;
use crate::type_checker;
use std::collections::HashMap;

/// O gerador de código para o alvo LLVM IR.
pub struct LlvmGenerator<'a> {
    programa: &'a ast::Programa,
    type_checker: &'a mut type_checker::VerificadorTipos<'a>,
    header: String,
    body: String,
    string_counter: usize,
    temp_counter: usize,
    /// Mantém o controle de variáveis locais e seus ponteiros de memória.
    variables: HashMap<String, (String, ast::Tipo)>,
    namespace_path: String,
    /// Mantém o controle da classe que está sendo processada no momento.
    classe_atual: Option<String>,
}

impl<'a> LlvmGenerator<'a> {
    pub fn new(programa: &'a ast::Programa, type_checker: &'a mut type_checker::VerificadorTipos<'a>) -> Self {
        Self {
            programa,
            type_checker,
            header: String::new(),
            body: String::new(),
            string_counter: 0,
            temp_counter: 0,
            variables: HashMap::new(),
            namespace_path: String::new(),
            classe_atual: None,
        }
    }

    pub fn generate(&mut self) -> String {
        self.prepare_header();
        self.define_all_structs();

        // Gera definições de funções e classes.
        for declaracao in &self.programa.declaracoes {
            match declaracao {
                ast::Declaracao::DeclaracaoFuncao(func) if func.nome != "Principal" => {
                    self.generate_funcao(func, "");
                }
                ast::Declaracao::DeclaracaoClasse(class) => {
                    self.generate_classe_definitions(class, "");
                }
                _ => {}
            }
        }
        for ns in &self.programa.namespaces {
            self.generate_namespace_definitions(ns);
        }

        // Gera a função `main`, que conterá o código da função `Principal`.
        self.body.push_str("define i32 @main() {
");
        self.body.push_str("entry:
");

        if let Some(principal_func) = self.find_principal_function() {
            self.namespace_path = self.get_function_namespace(principal_func).unwrap_or_default();
            for comando in &principal_func.corpo {
                self.generate_comando(comando);
            }
        }

        self.body.push_str("  ret i32 0
");
        self.body.push_str("}
");

        format!("{}{}", self.header, self.body)
    }
    
    fn find_principal_function(&self) -> Option<&'a ast::DeclaracaoFuncao> {
        for decl in &self.programa.declaracoes {
            if let ast::Declaracao::DeclaracaoFuncao(func) = decl {
                if func.nome == "Principal" {
                    return Some(func);
                }
            }
        }
        for ns in &self.programa.namespaces {
            for decl in &ns.declaracoes {
                 if let ast::Declaracao::DeclaracaoFuncao(func) = decl {
                    if func.nome == "Principal" {
                        return Some(func);
                    }
                }
            }
        }
        None
    }
    
    fn get_function_namespace(&self, func_to_find: &ast::DeclaracaoFuncao) -> Option<String> {
        for ns in &self.programa.namespaces {
            for decl in &ns.declaracoes {
                if let ast::Declaracao::DeclaracaoFuncao(func) = decl {
                    if std::ptr::eq(func, func_to_find) {
                        return Some(ns.nome.clone());
                    }
                }
            }
        }
        None
    }


    fn define_all_structs(&mut self) {
        let mut all_classes = Vec::new();
        for ns in &self.programa.namespaces {
            for decl in &ns.declaracoes {
                if let ast::Declaracao::DeclaracaoClasse(class) = decl {
                    let fqn = format!("{}.{}", ns.nome, class.nome);
                    all_classes.push(fqn);
                }
            }
        }
        for decl in &self.programa.declaracoes {
            if let ast::Declaracao::DeclaracaoClasse(class) = decl {
                all_classes.push(class.nome.clone());
            }
        }

        for fqn in all_classes {
            self.define_struct(fqn.as_str());
        }
    }

    fn define_struct(&mut self, fqn: &str) {
        let mut field_types_llvm = Vec::new();
        if let Some(resolved_info) = self.type_checker.resolved_classes.get(fqn) {
            let mut all_fields: Vec<(&String, &ast::Tipo)> = resolved_info.fields.iter().map(|f| (&f.nome, &f.tipo)).collect();
            all_fields.extend(resolved_info.properties.iter().map(|p| (&p.nome, &p.tipo)));
            
            // TODO: Ordenar os campos de forma consistente, talvez alfabeticamente.
            // all_fields.sort_by_key(|a| a.0);

            for (_, tipo) in all_fields {
                field_types_llvm.push(self.map_type_to_llvm_storage(tipo));
            }
        }

        let struct_body = field_types_llvm.join(", ");
        let sanitized_fqn = fqn.replace('.', "_");
        let struct_def = format!("%class.{} = type {{ {} }}
", sanitized_fqn, struct_body);
        self.header.push_str(&struct_def);
    }

    fn generate_namespace_definitions(&mut self, ns: &'a ast::DeclaracaoNamespace) {
        let old_namespace = self.namespace_path.clone();
        self.namespace_path = if old_namespace.is_empty() {
            ns.nome.clone()
        } else {
            format!("{}.{}", old_namespace, ns.nome)
        };

        for decl in &ns.declaracoes {
            match decl {
                ast::Declaracao::DeclaracaoFuncao(func) if func.nome != "Principal" => {
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
    
    fn generate_classe_definitions(&mut self, class: &'a ast::DeclaracaoClasse, namespace: &str) {
        let fqn = if namespace.is_empty() {
            class.nome.clone()
        } else {
            format!("{}.{}", namespace, class.nome)
        };
        self.classe_atual = Some(fqn);
        for metodo in &class.metodos {
            self.generate_metodo(metodo);
        }
        self.classe_atual = None;
    }


    fn prepare_header(&mut self) {
        self.header.push_str("target triple = \"x86_64-pc-windows-msvc\"
");
        self.header.push_str("declare i32 @printf(i8*, ...)
");
        self.header.push_str("declare i8* @malloc(i64)
");
        self.header.push_str("declare i32 @sprintf(i8*, i8*, ...)
");
        self.header.push_str("declare i64 @strlen(i8*)
");
        self.header.push_str("declare void @llvm.memcpy.p0i8.p0i8.i64(i8* nocapture writeonly, i8* nocapture readonly, i64, i1 immarg)
");
        self.header.push_str("@.println_fmt = private unnamed_addr constant [4 x i8] c\"%s\\0A\\00\", align 1
");
        self.header.push_str("@.int_fmt = private unnamed_addr constant [3 x i8] c\"%d\\00\", align 1
");
    }

    fn setup_parameters(&mut self, params: &[ast::Parametro]) {
        for param in params {
            let ptr_reg = format!("%var.{}", param.nome);
            let var_type = param.tipo.clone();
            let llvm_type = self.map_type_to_llvm_storage(&var_type);
            let align = self.get_type_alignment(&var_type);

            self.body.push_str(&format!("  {} = alloca {}, align {}
", ptr_reg, llvm_type, align));
            
            let param_reg = format!("%param.{}", param.nome);
            self.body.push_str(&format!("  store {} {}, {}* {}
", llvm_type, param_reg, llvm_type, ptr_reg));
            
            self.variables.insert(param.nome.to_string(), (ptr_reg, var_type));
        }
    }

    fn get_type_alignment(&self, var_type: &ast::Tipo) -> u32 {
        match var_type {
            ast::Tipo::Inteiro => 4,
            ast::Tipo::Texto => 8,
            ast::Tipo::Booleano => 1,
            ast::Tipo::Classe(_) => 8,
            _ => 8,
        }
    }

    fn generate_comando(&mut self, comando: &ast::Comando) {
        match comando {
            ast::Comando::DeclaracaoVar(nome, expr) => {
                let (value_reg, value_type) = self.generate_expressao(expr);
                self.declare_and_store_variable(nome, value_type, &value_reg);
            }
            ast::Comando::DeclaracaoVariavel(tipo, nome, Some(expr)) => {
                let (value_reg, _) = self.generate_expressao(expr);
                self.declare_and_store_variable(nome, tipo.clone(), &value_reg);
            }
            ast::Comando::Imprima(expr) => {
                let (value_reg, value_type) = self.generate_expressao(expr);
                let final_value_reg = self.ensure_string(&value_reg, &value_type);
                self.body.push_str(&format!(
                    "  call i32 (i8*, ...) @printf(i8* getelementptr inbounds ([4 x i8], [4 x i8]* @.println_fmt, i32 0, i32 0), i8* {})
",
                    final_value_reg
                ));
            }
            ast::Comando::Bloco(comandos) => {
                for cmd in comandos {
                    self.generate_comando(cmd);
                }
            }
            ast::Comando::Atribuicao(nome, expr) => {
                let (value_reg, value_type) = self.generate_expressao(expr);
                self.store_variable(nome, &value_type, &value_reg);
            }
            ast::Comando::Expressao(expr) => {
                self.generate_expressao(expr);
            }
            ast::Comando::Enquanto(cond, body) => {
                let loop_cond_label = self.get_unique_label("loop.cond");
                let loop_body_label = self.get_unique_label("loop.body");
                let loop_end_label = self.get_unique_label("loop.end");

                self.body.push_str(&format!("  br label %{}\n", loop_cond_label));
                self.body.push_str(&format!("{}:\n", loop_cond_label));

                let (cond_reg, _) = self.generate_expressao(cond);
                self.body.push_str(&format!("  br i1 {}, label %{}, label %{}\n", cond_reg, loop_body_label, loop_end_label));

                self.body.push_str(&format!("{}:\n", loop_body_label));
                self.generate_comando(body);
                self.body.push_str(&format!("  br label %{}\n", loop_cond_label));

                self.body.push_str(&format!("{}:\n", loop_end_label));
            }
            ast::Comando::Se(cond, then_block, else_block) => {
                let (cond_reg, _) = self.generate_expressao(cond);
                let then_label = self.get_unique_label("then");
                let else_label = self.get_unique_label("else");
                let end_label = self.get_unique_label("end");

                let has_else = else_block.is_some();
                let final_else_label = if has_else { else_label.clone() } else { end_label.clone() };

                self.body.push_str(&format!("  br i1 {}, label %{}, label %{}\n", cond_reg, then_label, final_else_label));

                self.body.push_str(&format!("{}:\n", then_label));
                self.generate_comando(then_block);
                self.body.push_str(&format!("  br label %{}\n", end_label));

                if let Some(else_cmd) = else_block {
                    self.body.push_str(&format!("{}:\n", else_label));
                    self.generate_comando(else_cmd);
                    self.body.push_str(&format!("  br label %{}\n", end_label));
                }

                self.body.push_str(&format!("{}:\n", end_label));
            }
             ast::Comando::Retorne(expr) => {
                if let Some(e) = expr {
                    let (reg, tipo) = self.generate_expressao(e);
                    let llvm_type = self.map_type_to_llvm_arg(&tipo);
                    self.body.push_str(&format!("  ret {} {}\n", llvm_type, reg));
                } else {
                    self.body.push_str("  ret void\n");
                }
            }
            _ => panic!("Comando não suportado para geração de LLVM IR: {:?}", comando),
        }
    }

    fn generate_funcao(&mut self, func: &ast::DeclaracaoFuncao, namespace: &str) {
        let nome_funcao = self.type_checker.resolver_nome_funcao(&func.nome, namespace);
        let tipo_retorno_llvm = self.map_type_to_llvm_arg(&func.tipo_retorno.clone().unwrap_or(ast::Tipo::Vazio));

        let mut params_llvm = Vec::new();
        for param in &func.parametros {
            let tipo_param_llvm = self.map_type_to_llvm_arg(&param.tipo);
            params_llvm.push(format!("{} %param.{}", tipo_param_llvm, param.nome));
        }

        let old_body = self.body.clone();
        let old_vars = self.variables.clone();
        self.body = String::new();
        self.variables.clear();

        self.body.push_str(&format!("define {} @{}({}) {{\n", tipo_retorno_llvm, nome_funcao, params_llvm.join(", ")));
        self.body.push_str("entry:\n");

        self.setup_parameters(&func.parametros);

        for comando in &func.corpo {
            self.generate_comando(comando);
        }

        // Se a função não tiver um comando de retorno explícito, adicione um.
        if !self.body.trim_end().ends_with("ret") {
             if func.tipo_retorno == Some(ast::Tipo::Vazio) || func.tipo_retorno.is_none() {
                self.body.push_str("  ret void\n");
            } else {
                // Funções que não são void devem ter um `ret` explícito.
                // O verificador de tipos deve garantir isso.
                 self.body.push_str(&format!("  unreachable ; A função '{}' deve ter um retorno\n", func.nome));
            }
        }

        self.body.push_str("}
");
        self.header.push_str(&self.body);

        self.body = old_body;
        self.variables = old_vars;
    }

    fn generate_metodo(&mut self, metodo: &ast::MetodoClasse) {
        let classe_nome = self.classe_atual.as_ref().unwrap();
        let nome_metodo = format!("{}::{}", classe_nome, metodo.nome).replace('.', "_");
        let tipo_retorno_llvm = self.map_type_to_llvm_arg(&metodo.tipo_retorno.clone().unwrap_or(ast::Tipo::Vazio));

        let mut params_llvm = Vec::new();
        let self_type = self.map_type_to_llvm_ptr(&ast::Tipo::Classe(classe_nome.clone()));
        params_llvm.push(format!("{} %param.self", self_type));

        for param in &metodo.parametros {
            let tipo_param_llvm = self.map_type_to_llvm_arg(&param.tipo);
            params_llvm.push(format!("{} %param.{}", tipo_param_llvm, param.nome));
        }

        let old_body = self.body.clone();
        let old_vars = self.variables.clone();
        self.body = String::new();
        self.variables.clear();

        self.body.push_str(&format!("define {} @\"{}\"({}) {{\n", tipo_retorno_llvm, nome_metodo, params_llvm.join(", ")));
        self.body.push_str("entry:\n");

        // Armazena 'self'
        let self_ptr_reg = "%var.self".to_string();
        self.body.push_str(&format!("  {} = alloca {}, align 8\n", self_ptr_reg, self_type));
        self.body.push_str(&format!("  store {} %param.self, {}* {}\n", self_type, self_type, self_ptr_reg));
        self.variables.insert("self".to_string(), (self_ptr_reg, ast::Tipo::Classe(classe_nome.clone())));

        self.setup_parameters(&metodo.parametros);

        for comando in &metodo.corpo {
            self.generate_comando(comando);
        }

        if !self.body.trim_end().ends_with("ret") {
            if metodo.tipo_retorno.is_none() || metodo.tipo_retorno == Some(ast::Tipo::Vazio) {
                self.body.push_str("  ret void\n");
            } else {
                self.body.push_str(&format!("  unreachable ; O método '{}' deve ter um retorno\n", metodo.nome));
            }
        }

        self.body.push_str("}
");
        self.header.push_str(&self.body);
        self.body = old_body;
        self.variables = old_vars;
    }

    fn declare_and_store_variable(&mut self, name: &str, var_type: ast::Tipo, value_reg: &str) {
        let ptr_reg = format!("%var.{}", name);
        let llvm_type = self.map_type_to_llvm_storage(&var_type);
        let align = self.get_type_alignment(&var_type);
        
        self.body.push_str(&format!("  {} = alloca {}, align {}\n", ptr_reg, llvm_type, align));
        self.body.push_str(&format!("  store {} {}, {}* {}\n", llvm_type, value_reg, llvm_type, ptr_reg));
        
        self.variables.insert(name.to_string(), (ptr_reg, var_type));
    }
    
    fn get_member_ptr(&mut self, obj_ptr_reg: &str, class_name: &str, member_name: &str) -> (String, ast::Tipo) {
        let resolved_info = self.type_checker.resolved_classes.get(class_name)
            .unwrap_or_else(|| panic!("Classe '{}' não encontrada.", class_name));

        let mut current_index = 0;
        if let Some(pos) = resolved_info.fields.iter().position(|f| f.nome == member_name) {
            let field = &resolved_info.fields[pos];
            let member_type = field.tipo.clone();
            let member_index = current_index + pos;
            
            let member_ptr_reg = self.get_unique_temp_name();
            let struct_type = self.map_type_to_llvm_storage(&ast::Tipo::Classe(class_name.to_string()));
            let obj_ptr_type = self.map_type_to_llvm_ptr(&ast::Tipo::Classe(class_name.to_string()));

            self.body.push_str(&format!(
                "  {} = getelementptr inbounds {}, {} {}, i32 0, i32 {}\n",
                member_ptr_reg, struct_type, obj_ptr_type, obj_ptr_reg, member_index
            ));
            return (member_ptr_reg, member_type);
        }
        current_index += resolved_info.fields.len();
        
        if let Some(pos) = resolved_info.properties.iter().position(|p| p.nome == member_name) {
            let prop = &resolved_info.properties[pos];
            let member_type = prop.tipo.clone();
            let member_index = current_index + pos;

            let member_ptr_reg = self.get_unique_temp_name();
            let struct_type = self.map_type_to_llvm_storage(&ast::Tipo::Classe(class_name.to_string()));
            let obj_ptr_type = self.map_type_to_llvm_ptr(&ast::Tipo::Classe(class_name.to_string()));

            self.body.push_str(&format!(
                "  {} = getelementptr inbounds {}, {} {}, i32 0, i32 {}\n",
                member_ptr_reg, struct_type, obj_ptr_type, obj_ptr_reg, member_index
            ));
            return (member_ptr_reg, member_type);
        }

        panic!("Membro '{}' não encontrado na classe '{}'", member_name, class_name);
    }

    fn store_variable(&mut self, name: &str, _value_type: &ast::Tipo, value_reg: &str) {
        // É uma variável local?
        if let Some((ptr_reg, var_type)) = self.variables.get(name) {
            let llvm_type = self.map_type_to_llvm_storage(var_type);
            self.body.push_str(&format!("  store {} {}, {}* {}\n", llvm_type, value_reg, llvm_type, ptr_reg));
            return;
        }

        // É um membro da classe atual?
        if let Some(class_name) = self.classe_atual.clone() {
             if self.type_checker.is_member_of_class(&class_name, name) {
                let (self_ptr, _) = self.variables.get("self").unwrap().clone();
                let loaded_self_ptr = self.get_unique_temp_name();
                let self_type_ptr = self.map_type_to_llvm_ptr(&ast::Tipo::Classe(class_name.clone()));
                self.body.push_str(&format!("  {} = load {}, {}* {}\n", loaded_self_ptr, self_type_ptr, self_type_ptr, self_ptr));

                let (member_ptr_reg, member_type) = self.get_member_ptr(&loaded_self_ptr, &class_name, name);
                let llvm_type = self.map_type_to_llvm_storage(&member_type);
                self.body.push_str(&format!("  store {} {}, {}* {}\n", llvm_type, value_reg, llvm_type, member_ptr_reg));
                return;
            }
        }
        
        panic!("Atribuição a variável não declarada '{}'", name);
    }


    fn generate_expressao(&mut self, expr: &ast::Expressao) -> (String, ast::Tipo) {
        match expr {
            ast::Expressao::Inteiro(n) => (n.to_string(), ast::Tipo::Inteiro),
            ast::Expressao::Texto(s) => (self.create_global_string(s), ast::Tipo::Texto),
            ast::Expressao::Booleano(b) => (if *b { "1" } else { "0" }.to_string(), ast::Tipo::Booleano),
            ast::Expressao::Identificador(name) => self.load_variable(name),
            ast::Expressao::Aritmetica(op, esq, dir) => {
                let (left_reg, left_type) = self.generate_expressao(esq);
                let (right_reg, right_type) = self.generate_expressao(dir);

                if left_type == ast::Tipo::Texto || right_type == ast::Tipo::Texto {
                    let left_str = self.ensure_string(&left_reg, &left_type);
                    let right_str = self.ensure_string(&right_reg, &right_type);
                    return (self.concatenate_strings(&left_str, &right_str), ast::Tipo::Texto);
                }
                
                let op_code = match op {
                    ast::OperadorAritmetico::Soma => "add",
                    ast::OperadorAritmetico::Subtracao => "sub",
                    ast::OperadorAritmetico::Multiplicacao => "mul",
                    ast::OperadorAritmetico::Divisao => "sdiv",
                    ast::OperadorAritmetico::Modulo => "srem",
                };

                let result_reg = self.get_unique_temp_name();
                self.body.push_str(&format!("  {} = {} i32 {}, {}\n", result_reg, op_code, left_reg, right_reg));
                (result_reg, ast::Tipo::Inteiro)
            }
            ast::Expressao::NovoObjeto(nome_classe, _argumentos) => {
                let fqn = self.type_checker.resolver_nome_classe(nome_classe, &self.namespace_path);
                let struct_type = self.map_type_to_llvm_storage(&ast::Tipo::Classe(fqn.clone()));
                let struct_ptr_type = self.map_type_to_llvm_ptr(&ast::Tipo::Classe(fqn.clone()));

                let size_temp_reg = self.get_unique_temp_name();
                self.body.push_str(&format!("  {} = getelementptr inbounds {}, {} null, i32 1\n", size_temp_reg, struct_type, struct_ptr_type));
                
                let size_reg = self.get_unique_temp_name();
                self.body.push_str(&format!("  {} = ptrtoint {} {} to i64\n", size_reg, struct_ptr_type, size_temp_reg));

                let malloc_ptr_reg = self.get_unique_temp_name();
                self.body.push_str(&format!("  {} = call i8* @malloc(i64 {}\n", malloc_ptr_reg, size_reg));

                let obj_ptr_reg = self.get_unique_temp_name();
                self.body.push_str(&format!("  {} = bitcast i8* {} to {}\n", obj_ptr_reg, malloc_ptr_reg, struct_ptr_type));
                
                // TODO: Chamar construtor
                (obj_ptr_reg, ast::Tipo::Classe(fqn))
            }
            ast::Expressao::Chamada(nome_funcao, argumentos) => {
                let fqn = self.type_checker.resolver_nome_funcao(nome_funcao, &self.namespace_path);
                let return_type = self.type_checker.get_function_return_type(&fqn, "").unwrap_or(ast::Tipo::Vazio);
                let return_type_llvm = self.map_type_to_llvm_arg(&return_type);

                let mut arg_regs = Vec::new();
                for arg in argumentos {
                    let (arg_reg, arg_type) = self.generate_expressao(arg);
                    let llvm_type = self.map_type_to_llvm_arg(&arg_type);
                    arg_regs.push(format!("{} {}", llvm_type, arg_reg));
                }
                let args_str = arg_regs.join(", ");

                if return_type == ast::Tipo::Vazio {
                    self.body.push_str(&format!("  call {} @{}({})
", return_type_llvm, fqn, args_str));
                    ("".to_string(), return_type)
                } else {
                    let result_reg = self.get_unique_temp_name();
                    self.body.push_str(&format!("  {} = call {} @{}({})
", result_reg, return_type_llvm, fqn, args_str));
                    (result_reg, return_type)
                }
            }
            ast::Expressao::ChamadaMetodo(obj_expr, metodo_nome, argumentos) => {
                let (obj_reg, obj_type) = self.generate_expressao(obj_expr);
                let class_name = match obj_type {
                    ast::Tipo::Classe(ref name) => name.clone(),
                    _ => panic!("Chamada de método em algo que não é um objeto."),
                };
                
                let resolved_method = self.type_checker.resolved_classes.get(&class_name)
                    .and_then(|c| c.methods.get(metodo_nome))
                    .unwrap_or_else(|| panic!("Método '{}' não encontrado na classe '{}'", metodo_nome, class_name));

                let return_type = resolved_method.tipo_retorno.clone().unwrap_or(ast::Tipo::Vazio);
                let return_type_llvm = self.map_type_to_llvm_arg(&return_type);
                
                let fqn_method = format!("{}::{}", class_name, metodo_nome).replace('.', "_");

                let mut arg_regs = Vec::new();
                let obj_ptr_type = self.map_type_to_llvm_ptr(&obj_type);
                arg_regs.push(format!("{} {}", obj_ptr_type, obj_reg)); // self

                for arg in argumentos {
                    let (arg_reg, arg_type) = self.generate_expressao(arg);
                    let llvm_type = self.map_type_to_llvm_arg(&arg_type);
                    arg_regs.push(format!("{} {}", llvm_type, arg_reg));
                }
                let args_str = arg_regs.join(", ");

                if return_type == ast::Tipo::Vazio {
                    self.body.push_str(&format!("  call {} @\"{}\"(({})
", return_type_llvm, fqn_method, args_str));
                    ("".to_string(), return_type)
                } else {
                    let result_reg = self.get_unique_temp_name();
                    self.body.push_str(&format!("  {} = call {} @\"{}\"(({})
", result_reg, return_type_llvm, fqn_method, args_str));
                    (result_reg, return_type)
                }
            }
            ast::Expressao::Comparacao(op, esq, dir) => {
                let (left_reg, left_type) = self.generate_expressao(esq);
                let (right_reg, _) = self.generate_expressao(dir);

                let op_str = match op {
                    ast::OperadorComparacao::Igual => "eq",
                    ast::OperadorComparacao::Diferente => "ne",
                    ast::OperadorComparacao::Menor => "slt",
                    ast::OperadorComparacao::MaiorQue => "sgt",
                    ast::OperadorComparacao::MenorIgual => "sle",
                    ast::OperadorComparacao::MaiorIgual => "sge",
                };

                let result_reg = self.get_unique_temp_name();
                let llvm_type = self.map_type_to_llvm_arg(&left_type);
                self.body.push_str(&format!("  {} = icmp {} {} {}, {}\n", result_reg, op_str, llvm_type, left_reg, right_reg));
                (result_reg, ast::Tipo::Booleano)
            }
            ast::Expressao::StringInterpolada(partes) => {
                let mut result_reg = self.create_global_string("");
                for parte in partes {
                    let part_reg = match parte {
                        ast::PartStringInterpolada::Texto(texto) => self.create_global_string(texto),
                        ast::PartStringInterpolada::Expressao(expr) => {
                            let (expr_reg, expr_type) = self.generate_expressao(expr);
                            self.ensure_string(&expr_reg, &expr_type)
                        }
                    };
                    result_reg = self.concatenate_strings(&result_reg, &part_reg);
                }
                (result_reg, ast::Tipo::Texto)
            }
            ast::Expressao::AcessoMembro(obj_expr, membro_nome) => {
                let (obj_reg, obj_type) = self.generate_expressao(obj_expr);
                let class_name = match obj_type {
                    ast::Tipo::Classe(name) => name,
                    _ => panic!("Acesso de membro em algo que não é uma classe: {:?}", obj_type),
                };
                let (member_ptr_reg, member_type) = self.get_member_ptr(&obj_reg, &class_name, membro_nome);
                
                let loaded_reg = self.get_unique_temp_name();
                let llvm_type = self.map_type_to_llvm_storage(&member_type);
                let llvm_ptr_type = self.map_type_to_llvm_ptr(&member_type);
                self.body.push_str(&format!("  {} = load {}, {} {}\n", loaded_reg, llvm_type, llvm_ptr_type, member_ptr_reg));
                (loaded_reg, member_type)
            }
            ast::Expressao::Este => self.load_variable("self"),
            _ => panic!("Expressão não suportada: {:?}", expr),
        }
    }

    fn load_variable(&mut self, name: &str) -> (String, ast::Tipo) {
        // É uma variável local?
        if let Some((ptr_reg, var_type)) = self.variables.get(name).cloned() {
            let loaded_reg = self.get_unique_temp_name();
            let llvm_type = self.map_type_to_llvm_storage(&var_type);
            let llvm_ptr_type = self.map_type_to_llvm_ptr(&var_type);
            self.body.push_str(&format!("  {} = load {}, {} {}\n", loaded_reg, llvm_type, llvm_ptr_type, ptr_reg));
            return (loaded_reg, var_type);
        }

        // É um membro da classe atual?
        if let Some(class_name) = self.classe_atual.clone() {
            if self.type_checker.is_member_of_class(&class_name, name) {
                let (self_ptr, _) = self.variables.get("self").unwrap().clone();
                let loaded_self_ptr = self.get_unique_temp_name();
                let self_type_ptr = self.map_type_to_llvm_ptr(&ast::Tipo::Classe(class_name.clone()));
                self.body.push_str(&format!("  {} = load {}, {}* {}\n", loaded_self_ptr, self_type_ptr, self_type_ptr, self_ptr));

                let (member_ptr_reg, member_type) = self.get_member_ptr(&loaded_self_ptr, &class_name, name);
                let loaded_reg = self.get_unique_temp_name();
                let llvm_type = self.map_type_to_llvm_storage(&member_type);
                let llvm_ptr_type = self.map_type_to_llvm_ptr(&member_type);
                self.body.push_str(&format!("  {} = load {}, {} {}\n", loaded_reg, llvm_type, llvm_ptr_type, member_ptr_reg));
                return (loaded_reg, member_type);
            }
        }
        
        panic!("Variável ou membro de classe não declarado: '{}'", name);
    }

    fn ensure_string(&mut self, reg: &str, tipo: &ast::Tipo) -> String {
        match tipo {
            ast::Tipo::Texto => reg.to_string(),
            ast::Tipo::Inteiro => self.convert_int_to_string(reg),
            ast::Tipo::Booleano => {
                let true_str = self.create_global_string("verdadeiro");
                let false_str = self.create_global_string("falso");
                let result_reg = self.get_unique_temp_name();
                self.body.push_str(&format!("  {} = select i1 {}, i8* {}, i8* {}\n", result_reg, reg, true_str, false_str));
                result_reg
            }
            _ => self.create_global_string("[valor não textual]"),
        }
    }

    fn convert_int_to_string(&mut self, int_reg: &str) -> String {
        let buffer = self.get_unique_temp_name();
        self.body.push_str(&format!("  {} = alloca [21 x i8], align 1\n", buffer));
        let buffer_ptr = self.get_unique_temp_name();
        self.body.push_str(&format!("  {} = getelementptr inbounds [21 x i8], [21 x i8]* {}, i32 0, i32 0\n", buffer_ptr, buffer));
        
        let format_specifier_ptr = self.get_unique_temp_name();
        self.body.push_str(&format!("  {} = getelementptr inbounds [3 x i8], [3 x i8]* @.int_fmt, i32 0, i32 0\n", format_specifier_ptr));

        self.body.push_str(&format!("  call i32 (i8*, i8*, ...) @sprintf(i8* {}, i8* {}, i32 {})
", buffer_ptr, format_specifier_ptr, int_reg));
        buffer_ptr
    }

    fn concatenate_strings(&mut self, str1_reg: &str, str2_reg: &str) -> String {
        let len1_reg = self.get_unique_temp_name();
        self.body.push_str(&format!("  {} = call i64 @strlen(i8* {})
", len1_reg, str1_reg));
        
        let len2_reg = self.get_unique_temp_name();
        self.body.push_str(&format!("  {} = call i64 @strlen(i8* {})
", len2_reg, str2_reg));
        
        let total_len_reg = self.get_unique_temp_name();
        self.body.push_str(&format!("  {} = add i64 {}, {}\n", total_len_reg, len1_reg, len2_reg));
        
        let alloc_size_reg = self.get_unique_temp_name();
        self.body.push_str(&format!("  {} = add i64 {}, 1\n", alloc_size_reg, total_len_reg));
        
        let buffer_reg = self.get_unique_temp_name();
        self.body.push_str(&format!("  {} = call i8* @malloc(i64 {})
", buffer_reg, alloc_size_reg));
        
        // Concatenação manual
        let dest_ptr1 = self.get_unique_temp_name();
        self.body.push_str(&format!("  {} = getelementptr i8, i8* {}, i64 0\n", dest_ptr1, buffer_reg));
        self.body.push_str(&format!("  call void @llvm.memcpy.p0i8.p0i8.i64(i8* align 1 {}, i8* align 1 {}, i64 {}, i1 false)\n", dest_ptr1, str1_reg, len1_reg));

        let dest_ptr2 = self.get_unique_temp_name();
        self.body.push_str(&format!("  {} = getelementptr i8, i8* {}, i64 {}\n", dest_ptr2, buffer_reg, len1_reg));
        self.body.push_str(&format!("  call void @llvm.memcpy.p0i8.p0i8.i64(i8* align 1 {}, i8* align 1 {}, i64 {}, i1 false)\n", dest_ptr2, str2_reg, len2_reg));

        let null_terminator_ptr = self.get_unique_temp_name();
        self.body.push_str(&format!("  {} = getelementptr i8, i8* {}, i64 {}\n", null_terminator_ptr, buffer_reg, total_len_reg));
        self.body.push_str(&format!("  store i8 0, i8* {}\n", null_terminator_ptr));

        buffer_reg
    }

    fn create_global_string(&mut self, text: &str) -> String {
        let str_len = text.len() + 1;
        let str_name = format!("@.str.{}", self.string_counter);
        self.string_counter += 1;
        let sanitized_text = text.replace('\\', "\\").replace('\n', "\\n").replace('"', "\"");
        self.header.push_str(&format!("{} = private unnamed_addr constant [{} x i8] c\"{}\\00\", align 1
", str_name, str_len, sanitized_text));
        
        let ptr_register = self.get_unique_temp_name();
        self.body.push_str(&format!("  {} = getelementptr inbounds [{} x i8], [{} x i8]* {}, i32 0, i32 0\n", ptr_register, str_len, str_len, str_name));
        ptr_register
    }

    fn get_unique_temp_name(&mut self) -> String {
        let name = format!("%tmp.{}", self.temp_counter);
        self.temp_counter += 1;
        name
    }

    fn get_unique_label(&mut self, prefix: &str) -> String {
        let label = format!("{}.{}", prefix, self.temp_counter);
        self.temp_counter += 1;
        label
    }

    /// Mapeia um tipo da linguagem para um tipo LLVM para armazenamento (ex: em `alloca`).
    fn map_type_to_llvm_storage(&self, tipo: &ast::Tipo) -> String {
        match tipo {
            ast::Tipo::Inteiro => "i32".to_string(),
            ast::Tipo::Texto => "i8*".to_string(),
            ast::Tipo::Booleano => "i1".to_string(),
            ast::Tipo::Vazio => "void".to_string(),
            ast::Tipo::Classe(_) => self.map_type_to_llvm_ptr(tipo),
            _ => panic!("Tipo LLVM não mapeado para armazenamento: {:?}", tipo),
        }
    }
    
    /// Mapeia um tipo da linguagem para um tipo de ponteiro LLVM.
    fn map_type_to_llvm_ptr(&self, tipo: &ast::Tipo) -> String {
         match tipo {
            ast::Tipo::Inteiro => "i32*".to_string(),
            ast::Tipo::Texto => "i8**".to_string(),
            ast::Tipo::Booleano => "i1*".to_string(),
            ast::Tipo::Classe(name) => {
                let sanitized_name = name.replace('.', "_");
                format!("%class.{}*", sanitized_name)
            }
            _ => panic!("Não é possível criar um ponteiro para o tipo: {:?}", tipo),
        }
    }

    /// Mapeia um tipo da linguagem para um tipo LLVM para argumentos de função/retorno.
    fn map_type_to_llvm_arg(&self, tipo: &ast::Tipo) -> String {
        match tipo {
            ast::Tipo::Inteiro => "i32".to_string(),
            ast::Tipo::Texto => "i8*".to_string(),
            ast::Tipo::Booleano => "i1".to_string(),
            ast::Tipo::Vazio => "void".to_string(),
            ast::Tipo::Classe(_) => self.map_type_to_llvm_ptr(tipo),
            _ => panic!("Tipo LLVM não mapeado para argumento: {:?}", tipo),
        }
    }
}