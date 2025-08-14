use crate::ast;
use crate::type_checker;
use std::collections::HashMap;

/// O gerador de código para o alvo LLVM IR.
pub struct LlvmGenerator<'a> {
    programa: &'a ast::Programa,
    type_checker: &'a type_checker::VerificadorTipos<'a>,
    resolved_classes: &'a HashMap<String, type_checker::ResolvedClassInfo<'a>>,
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
    pub fn new(
        programa: &'a ast::Programa,
        type_checker: &'a type_checker::VerificadorTipos<'a>,
        resolved_classes: &'a HashMap<String, type_checker::ResolvedClassInfo<'a>>,
    ) -> Self {
        Self {
            programa,
            type_checker,
            resolved_classes,
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
        self.define_static_globals();

        // Gera definições de funções e classes.
        for declaracao in &self.programa.declaracoes {
            match declaracao {
                ast::Declaracao::DeclaracaoFuncao(func) => {
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

        // Gera a função `main`: executa comandos globais e, se existir, chama `Principal`.
        let mut old_body = self.body.clone();
        let old_vars = self.variables.clone();
        self.body = String::new();
        self.variables.clear();

        self.body.push_str("define i32 @main() {\n");
        self.body.push_str("entry:\n");

        // Comandos globais (top-level) no namespace raiz
        for decl in &self.programa.declaracoes {
            if let ast::Declaracao::Comando(cmd) = decl {
                self.generate_comando(cmd);
            }
        }

        // Se existir uma função Principal, chama-a ao final
        if let Some(principal_func) = self.find_principal_function() {
            let fqn = self
                .type_checker
                .resolver_nome_funcao(&principal_func.nome, &self.namespace_path);
            self.body
                .push_str(&format!("  call void @\"{}\"()\n", fqn.replace(".", "_")));
        }

        self.body.push_str("  ret i32 0\n");
        self.body.push_str("}\n");

        // Anexa e restaura
        old_body.push_str(&self.body);
        self.body = old_body;
        self.variables = old_vars;

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

    fn define_all_structs(&mut self) {
        let mut fqns: Vec<_> = self.resolved_classes.keys().collect();
        fqns.sort(); // Ordena para garantir uma ordem consistente.

        for fqn in fqns {
            self.define_struct(fqn.as_str());
        }
    }

    fn define_struct(&mut self, fqn: &str) {
        let mut field_types_llvm = Vec::new();
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

    fn generate_namespace_definitions(&mut self, ns: &'a ast::DeclaracaoNamespace) {
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

    fn define_static_globals(&mut self) {
        // Varre todas as classes (globais e em namespaces) e cria globais LLVM para membros estáticos com inicialização simples
        // Suporta: inteiro/booleano; demais tipos usam zeroinitializer
        fn process_class<'a>(
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

    fn static_global_symbol(&self, fqn_class: &str, member: &str) -> String {
        let suffix = format!(".static.{}.{}", fqn_class.replace('.', "_"), member);
        let mut s = String::from("@");
        s.push_str(&suffix);
        s
    }

    fn const_llvm_init_for_expr(
        &mut self,
        expr: &ast::Expressao,
        expected_type: &ast::Tipo,
    ) -> Option<(String, ast::Tipo)> {
        match (expr, expected_type) {
            (ast::Expressao::Inteiro(n), ast::Tipo::Inteiro) => {
                Some((n.to_string(), ast::Tipo::Inteiro))
            }
            (ast::Expressao::Booleano(b), ast::Tipo::Booleano) => Some((
                (if *b { "1" } else { "0" }).to_string(),
                ast::Tipo::Booleano,
            )),
            // Para outros tipos, pode exigir inicialização dinâmica; retornar None para zeroinitializer
            _ => None,
        }
    }

    fn infer_member_type(&self, fqn_class: &str, member: &str) -> Option<ast::Tipo> {
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

    fn generate_classe_definitions(&mut self, class: &'a ast::DeclaracaoClasse, namespace: &str) {
        let fqn = if namespace.is_empty() {
            class.nome.clone()
        } else {
            format!("{}.{}", namespace, class.nome)
        };
        self.classe_atual = Some(fqn);
        // Métodos
        for metodo in &class.metodos {
            self.generate_metodo(metodo);
        }
        // Construtores
        for construtor in &class.construtores {
            self.generate_construtor(construtor);
        }
        self.classe_atual = None;
    }

    fn generate_construtor(&mut self, construtor: &'a ast::ConstrutorClasse) {
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
                let parent_fqn = self
                    .type_checker
                    .resolver_nome_classe(nome_base_simples, namespace);

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

    fn prepare_header(&mut self) {
        self.header
            .push_str("target triple = \"x86_64-pc-windows-msvc\"\n");
        self.header.push_str("declare i32 @printf(i8*, ...)\n");
        self.header.push_str("declare i8* @malloc(i64)\n");
        self.header
            .push_str("declare i32 @sprintf(i8*, i8*, ...)\n");
        self.header.push_str("declare i64 @strlen(i8*)\n");
        self.header.push_str("declare void @llvm.memcpy.p0i8.p0i8.i64(i8* nocapture writeonly, i8* nocapture readonly, i64, i1 immarg)\n");
        self.header
            .push_str("declare void @llvm.memset.p0i8.i64(i8*, i8, i64, i1)\n");
        self.header.push_str(
            "@.println_fmt = private unnamed_addr constant [4 x i8] c\"%s\\0A\\00\", align 1\n",
        );
        self.header
            .push_str("@.int_fmt = private unnamed_addr constant [3 x i8] c\"%d\\00\", align 1\n");
        self.header
            .push_str("@.empty_str = private unnamed_addr constant [1 x i8] c\"\\00\", align 1\n");
    }

    fn setup_parameters(&mut self, params: &[ast::Parametro]) {
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
                let tipo_resolvido = self.resolve_type(tipo, &self.namespace_path);
                self.declare_and_store_variable(nome, tipo_resolvido, &value_reg);
            }
            ast::Comando::Imprima(expr) => {
                let (value_reg, value_type) = self.generate_expressao(expr);
                let final_value_reg = self.ensure_string(&value_reg, &value_type);
                self.body.push_str(&format!(
                    "  call i32 (i8*, ...) @printf(i8* getelementptr inbounds ([4 x i8], [4 x i8]* @.println_fmt, i32 0, i32 0), i8* {0})\n",
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

                self.body
                    .push_str(&format!("  br label %{0}\n", loop_cond_label));
                self.body.push_str(&format!("{0}:\n", loop_cond_label));

                let (cond_reg, _) = self.generate_expressao(cond);
                self.body.push_str(&format!(
                    "  br i1 {0}, label %{1}, label %{2}\n",
                    cond_reg, loop_body_label, loop_end_label
                ));

                self.body.push_str(&format!("{0}:\n", loop_body_label));
                self.generate_comando(body);
                self.body
                    .push_str(&format!("  br label %{0}\n", loop_cond_label));

                self.body.push_str(&format!("{0}:\n", loop_end_label));
            }
            ast::Comando::Se(cond, then_block, else_block) => {
                let (cond_reg, _) = self.generate_expressao(cond);
                let then_label = self.get_unique_label("then");
                let else_label = self.get_unique_label("else");
                let end_label = self.get_unique_label("end");

                let has_else = else_block.is_some();
                let final_else_label = if has_else {
                    else_label.clone()
                } else {
                    end_label.clone()
                };

                self.body.push_str(&format!(
                    "  br i1 {0}, label %{1}, label %{2}\n",
                    cond_reg, then_label, final_else_label
                ));

                self.body.push_str(&format!("{0}:\n", then_label));
                self.generate_comando(then_block);
                self.body.push_str(&format!("  br label %{0}\n", end_label));

                if let Some(else_cmd) = else_block {
                    self.body.push_str(&format!("{0}:\n", else_label));
                    self.generate_comando(else_cmd);
                    self.body.push_str(&format!("  br label %{0}\n", end_label));
                }

                self.body.push_str(&format!("{0}:\n", end_label));
            }
            ast::Comando::Retorne(expr) => {
                if let Some(e) = expr {
                    let (reg, tipo) = self.generate_expressao(e);
                    let llvm_type = self.map_type_to_llvm_arg(&tipo);
                    self.body
                        .push_str(&format!("  ret {0} {1}\n", llvm_type, reg));
                } else {
                    self.body.push_str("  ret void\n");
                }
            }
            ast::Comando::AtribuirPropriedade(obj_expr, prop_nome, val_expr) => {
                // Suporte a membro estático: objeto pode ser identificador de classe
                if let ast::Expressao::Identificador(class_ident) = &**obj_expr {
                    let fqn = self
                        .type_checker
                        .resolver_nome_classe(class_ident, &self.namespace_path);
                    if self.type_checker.classes.contains_key(&fqn) {
                        // Trata como propriedade estática
                        let (value_reg, value_type) = self.generate_expressao(val_expr);
                        let ty = self.map_type_to_llvm_storage(&value_type);
                        let sym = self.static_global_symbol(&fqn, prop_nome);
                        self.body
                            .push_str(&format!("  store {0} {1}, {0}* {2}\n", ty, value_reg, sym));
                        return;
                    }
                }

                // Caso instância
                let (value_reg, _value_type) = self.generate_expressao(val_expr);
                let (obj_ptr_reg, obj_type) = self.generate_expressao(obj_expr);
                let class_name = match obj_type {
                    ast::Tipo::Classe(name) => name,
                    _ => panic!(
                        "Atribuição de propriedade em algo que não é uma classe: {:?}",
                        obj_type
                    ),
                };
                let (member_ptr_reg, member_type) =
                    self.get_member_ptr(&obj_ptr_reg, &class_name, prop_nome);
                let llvm_type = self.map_type_to_llvm_storage(&member_type);
                self.body.push_str(&format!(
                    "  store {0} {1}, {2} {3}\n",
                    llvm_type,
                    value_reg,
                    self.map_type_to_llvm_ptr(&member_type),
                    member_ptr_reg
                ));
            }
            ast::Comando::ChamarMetodo(obj_expr, metodo_nome, argumentos) => {
                self.generate_expressao(&ast::Expressao::ChamadaMetodo(
                    obj_expr.clone(),
                    metodo_nome.clone(),
                    argumentos.clone(),
                ));
            }
            _ => panic!(
                "Comando não suportado para geração de LLVM IR: {:?}",
                comando
            ),
        }
    }

    fn generate_funcao(&mut self, func: &'a ast::DeclaracaoFuncao, namespace: &str) {
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

    fn generate_metodo(&mut self, metodo: &'a ast::MetodoClasse) {
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

    fn declare_and_store_variable(&mut self, name: &str, var_type: ast::Tipo, value_reg: &str) {
        let ptr_reg = format!("%var.{0}", name);
        let llvm_type = self.map_type_to_llvm_storage(&var_type);
        let align = self.get_type_alignment(&var_type);

        self.body.push_str(&format!(
            "  {0} = alloca {1}, align {2}\n",
            ptr_reg, llvm_type, align
        ));
        self.body.push_str(&format!(
            "  store {0} {1}, {0}* {2}\n",
            llvm_type, value_reg, ptr_reg
        ));

        self.variables.insert(name.to_string(), (ptr_reg, var_type));
    }

    fn get_member_ptr(
        &mut self,
        obj_ptr_reg: &str,
        class_name: &str,
        member_name: &str,
    ) -> (String, ast::Tipo) {
        let fqn_class_name = self
            .type_checker
            .resolver_nome_classe(class_name, &self.namespace_path);
        let resolved_info = self
            .resolved_classes
            .get(&fqn_class_name)
            .unwrap_or_else(|| panic!("Classe '{}' não encontrada.", fqn_class_name));

        let mut current_index = 0;
        if let Some(pos) = resolved_info
            .fields
            .iter()
            .position(|f| f.nome == member_name)
        {
            let field = &resolved_info.fields[pos];
            let member_type = field.tipo.clone();
            let member_index = current_index + pos;

            let member_ptr_reg = self.get_unique_temp_name();
            let sanitized_class_name = fqn_class_name.replace('.', "_");
            let struct_type = format!("%class.{0}", sanitized_class_name);
            let obj_ptr_type = format!("%class.{0}*", sanitized_class_name);
            self.body.push_str(&format!(
                "  {0} = getelementptr inbounds {1}, {2} {3}, i32 0, i32 {4}\n",
                member_ptr_reg, struct_type, obj_ptr_type, obj_ptr_reg, member_index
            ));
            return (member_ptr_reg, member_type);
        }
        current_index += resolved_info.fields.len();

        if let Some(pos) = resolved_info
            .properties
            .iter()
            .position(|p| p.nome == member_name)
        {
            let prop = &resolved_info.properties[pos];
            let member_type = prop.tipo.clone();
            let member_index = current_index + pos;

            let member_ptr_reg = self.get_unique_temp_name();
            let sanitized_class_name = fqn_class_name.replace('.', "_");
            let struct_type = format!("%class.{0}", sanitized_class_name);
            let obj_ptr_type = format!("%class.{0}*", sanitized_class_name);
            self.body.push_str(&format!(
                "  {0} = getelementptr inbounds {1}, {2} {3}, i32 0, i32 {4}\n",
                member_ptr_reg, struct_type, obj_ptr_type, obj_ptr_reg, member_index
            ));
            return (member_ptr_reg, member_type);
        }

        panic!(
            "Membro '{}' não encontrado na classe '{}'",
            member_name, class_name
        );
    }

    // Encontra o FQN da classe onde um método foi originalmente declarado.
    // Necessário para herança: quando chamamos um método herdado (não sobrescrito),
    // o símbolo LLVM existente é o da classe base (ex.: Animal::apresentar), não da derivada.
    fn get_declaring_class_of_method(&self, metodo_ref: &'a ast::MetodoClasse) -> Option<String> {
        for (class_name, class_decl) in &self.type_checker.classes {
            if class_decl
                .metodos
                .iter()
                .any(|m| std::ptr::eq(m, metodo_ref))
            {
                return Some(class_name.clone());
            }
        }
        None
    }

    fn store_variable(&mut self, name: &str, _value_type: &ast::Tipo, value_reg: &str) {
        if let Some((ptr_reg, var_type)) = self.variables.get(name) {
            let llvm_type = self.map_type_to_llvm_storage(var_type);
            self.body.push_str(&format!(
                "  store {0} {1}, {0}* {2}\n",
                llvm_type, value_reg, ptr_reg
            ));
            return;
        }

        if let Some(class_name) = self.classe_atual.clone() {
            if self.resolved_classes.get(&class_name).map_or(false, |c| {
                c.fields.iter().any(|f| f.nome == name)
                    || c.properties.iter().any(|p| p.nome == name)
            }) {
                let (self_ptr_reg, self_type) = self.variables.get("self").unwrap().clone();
                let loaded_self_ptr = self.get_unique_temp_name();
                let self_ptr_type = self.map_type_to_llvm_ptr(&self_type);

                self.body.push_str(&format!(
                    "  {0} = load {1}, {1}* {2}\n",
                    loaded_self_ptr, self_ptr_type, self_ptr_reg
                ));

                let (member_ptr_reg, member_type) =
                    self.get_member_ptr(&loaded_self_ptr, &class_name, name);
                let llvm_type = self.map_type_to_llvm_storage(&member_type);
                self.body.push_str(&format!(
                    "  store {0} {1}, {2} {3}\n",
                    llvm_type,
                    value_reg,
                    self.map_type_to_llvm_ptr(&member_type),
                    member_ptr_reg
                ));
                return;
            }
        }

        panic!("Atribuição a variável não declarada '{}'", name);
    }

    fn generate_expressao(&mut self, expr: &ast::Expressao) -> (String, ast::Tipo) {
        match expr {
            ast::Expressao::Inteiro(n) => (n.to_string(), ast::Tipo::Inteiro),
            ast::Expressao::Texto(s) => (self.create_global_string(s), ast::Tipo::Texto),
            ast::Expressao::Booleano(b) => {
                (if *b { "1" } else { "0" }.to_string(), ast::Tipo::Booleano)
            }
            ast::Expressao::Identificador(name) => self.load_variable(name),
            ast::Expressao::Aritmetica(op, esq, dir) => {
                let (left_reg, left_type) = self.generate_expressao(esq);
                let (right_reg, right_type) = self.generate_expressao(dir);

                if left_type == ast::Tipo::Texto || right_type == ast::Tipo::Texto {
                    let left_str = self.ensure_string(&left_reg, &left_type);
                    let right_str = self.ensure_string(&right_reg, &right_type);
                    return (
                        self.concatenate_strings(&left_str, &right_str),
                        ast::Tipo::Texto,
                    );
                }

                let op_code = match op {
                    ast::OperadorAritmetico::Soma => "add",
                    ast::OperadorAritmetico::Subtracao => "sub",
                    ast::OperadorAritmetico::Multiplicacao => "mul",
                    ast::OperadorAritmetico::Divisao => "sdiv",
                    ast::OperadorAritmetico::Modulo => "srem",
                };

                let result_reg = self.get_unique_temp_name();
                self.body.push_str(&format!(
                    "  {0} = {1} i32 {2}, {3}\n",
                    result_reg, op_code, left_reg, right_reg
                ));
                (result_reg, ast::Tipo::Inteiro)
            }
            ast::Expressao::NovoObjeto(nome_classe, argumentos) => {
                let fqn = self
                    .type_checker
                    .resolver_nome_classe(nome_classe, &self.namespace_path);
                let sanitized_fqn = fqn.replace('.', "_");
                let struct_type = format!("%class.{0}", sanitized_fqn);
                let struct_ptr_type = format!("%class.{0}*", sanitized_fqn);

                let size_temp_reg = self.get_unique_temp_name();
                self.body.push_str(&format!(
                    "  {0} = getelementptr inbounds {1}, {2} null, i32 1\n",
                    size_temp_reg, struct_type, struct_ptr_type
                ));

                let size_reg = self.get_unique_temp_name();
                self.body.push_str(&format!(
                    "  {0} = ptrtoint {1} {2} to i64\n",
                    size_reg, struct_ptr_type, size_temp_reg
                ));

                let malloc_ptr_reg = self.get_unique_temp_name();
                self.body.push_str(&format!(
                    "  {0} = call i8* @malloc(i64 {1})\n",
                    malloc_ptr_reg, size_reg
                ));

                // Inicializa a memória alocada com zeros.
                self.body.push_str(&format!(
                    "  call void @llvm.memset.p0i8.i64(i8* align 1 {0}, i8 0, i64 {1}, i1 false)\n",
                    malloc_ptr_reg, size_reg
                ));

                let obj_ptr_reg = self.get_unique_temp_name();
                self.body.push_str(&format!(
                    "  {0} = bitcast i8* {1} to {2}\n",
                    obj_ptr_reg, malloc_ptr_reg, struct_ptr_type
                ));

                // Chama um construtor: seleciona pelo número de argumentos (com suporte a defaults)
                if let Some(class_decl) = self.type_checker.classes.get(&fqn) {
                    // Encontrar melhor construtor compatível
                    let mut escolhido: Option<&ast::ConstrutorClasse> = None;
                    let mut melhor_total = 0usize;
                    for ctor in &class_decl.construtores {
                        let total = ctor.parametros.len();
                        let obrig = ctor
                            .parametros
                            .iter()
                            .filter(|p| p.valor_padrao.is_none())
                            .count();
                        let fornecidos = argumentos.len();
                        if fornecidos >= obrig && fornecidos <= total {
                            if total >= melhor_total {
                                melhor_total = total;
                                escolhido = Some(ctor);
                            }
                        }
                    }
                    if let Some(ctor) = escolhido {
                        // Monta lista final de argumentos (preenche defaults se necessário)
                        let mut final_args: Vec<(String, ast::Tipo)> = Vec::new();
                        let fornecidos = argumentos.len();
                        for (idx, param) in ctor.parametros.iter().enumerate() {
                            if idx < fornecidos {
                                final_args.push(self.generate_expressao(&argumentos[idx]));
                            } else {
                                if let Some(def_expr) = &param.valor_padrao {
                                    final_args.push(self.generate_expressao(def_expr));
                                } else {
                                    panic!("Argumento obrigatório ausente para parâmetro '{}' do construtor de '{}'", param.nome, fqn);
                                }
                            }
                        }

                        // Chamada ao construtor LLVM
                        let func_name = format!("{0}::construtor${1}", fqn, ctor.parametros.len())
                            .replace('.', "_");
                        let mut args_llvm = Vec::new();
                        // self primeiro
                        args_llvm.push(format!("{0} {1}", struct_ptr_type, obj_ptr_reg));
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

                (obj_ptr_reg, ast::Tipo::Classe(fqn))
            }
            ast::Expressao::Chamada(nome_funcao, argumentos) => {
                let fqn_func_name = self
                    .type_checker
                    .resolver_nome_funcao(nome_funcao, &self.namespace_path);
                let func = self
                    .programa
                    .declaracoes
                    .iter()
                    .find_map(|d| match d {
                        ast::Declaracao::DeclaracaoFuncao(f)
                            if self
                                .type_checker
                                .resolver_nome_funcao(&f.nome, &self.namespace_path)
                                == fqn_func_name =>
                        {
                            Some(f)
                        }
                        _ => None,
                    })
                    .or_else(|| {
                        self.programa.namespaces.iter().find_map(|ns| {
                            ns.declaracoes.iter().find_map(|d| match d {
                                ast::Declaracao::DeclaracaoFuncao(f)
                                    if self
                                        .type_checker
                                        .resolver_nome_funcao(&f.nome, &ns.nome)
                                        == fqn_func_name =>
                                {
                                    Some(f)
                                }
                                _ => None,
                            })
                        })
                    })
                    .unwrap();
                let return_type = func.tipo_retorno.clone().unwrap_or(ast::Tipo::Vazio);
                let return_type_llvm = self.map_type_to_llvm_arg(&return_type);

                let mut arg_regs = Vec::new();
                for arg in argumentos {
                    let (arg_reg, arg_type) = self.generate_expressao(arg);
                    let llvm_type = self.map_type_to_llvm_arg(&arg_type);
                    arg_regs.push(format!("{0} {1}", llvm_type, arg_reg));
                }
                let args_str = arg_regs.join(", ");
                let sanitized_func_name = fqn_func_name.replace('.', "_");

                if return_type == ast::Tipo::Vazio {
                    self.body.push_str(&format!(
                        "  call {0} @\"{1}\"({2})\n",
                        return_type_llvm, sanitized_func_name, args_str
                    ));
                    ("".to_string(), return_type)
                } else {
                    let result_reg = self.get_unique_temp_name();
                    self.body.push_str(&format!(
                        "  {0} = call {1} @\"{2}\"({3})\n",
                        result_reg, return_type_llvm, sanitized_func_name, args_str
                    ));
                    (result_reg, return_type)
                }
            }
            ast::Expressao::ChamadaMetodo(obj_expr, metodo_nome, argumentos) => {
                let (obj_reg, obj_type) = self.generate_expressao(obj_expr);
                let class_name = match obj_type {
                    ast::Tipo::Classe(ref name) => name.clone(),
                    _ => panic!("Chamada de método em algo que não é um objeto."),
                };

                let fqn_class_name = self
                    .type_checker
                    .resolver_nome_classe(&class_name, &self.namespace_path);

                let resolved_method = self
                    .resolved_classes
                    .get(&fqn_class_name)
                    .and_then(|c| c.methods.get(metodo_nome))
                    .unwrap_or_else(|| {
                        panic!(
                            "Método '{}' não encontrado na classe '{}'",
                            metodo_nome, fqn_class_name
                        )
                    });

                let return_type = resolved_method
                    .tipo_retorno
                    .clone()
                    .unwrap_or(ast::Tipo::Vazio);
                let return_type_llvm = self.map_type_to_llvm_arg(&return_type);
                // Usa a classe declaradora para construir o símbolo correto (suporta métodos herdados)
                let declaring_class = self
                    .get_declaring_class_of_method(resolved_method)
                    .unwrap_or_else(|| fqn_class_name.clone());
                let fqn_method =
                    format!("{0}::{1}", declaring_class, metodo_nome).replace('.', "_");

                let mut arg_regs = Vec::new();
                let obj_ptr_type = self.map_type_to_llvm_ptr(&obj_type);
                arg_regs.push(format!("{0} {1}", obj_ptr_type, obj_reg));

                for arg in argumentos {
                    let (arg_reg, arg_type) = self.generate_expressao(arg);
                    let llvm_type = self.map_type_to_llvm_arg(&arg_type);
                    arg_regs.push(format!("{0} {1}", llvm_type, arg_reg));
                }
                let args_str = arg_regs.join(", ");

                if return_type == ast::Tipo::Vazio {
                    self.body.push_str(&format!(
                        "  call void @\"{0}\"({1})\n",
                        fqn_method, args_str
                    ));
                    ("".to_string(), return_type)
                } else {
                    let result_reg = self.get_unique_temp_name();
                    self.body.push_str(&format!(
                        "  {0} = call {1} @\"{2}\"({3})\n",
                        result_reg, return_type_llvm, fqn_method, args_str
                    ));
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
                self.body.push_str(&format!(
                    "  {0} = icmp {1} {2} {3}, {4}\n",
                    result_reg, op_str, llvm_type, left_reg, right_reg
                ));
                (result_reg, ast::Tipo::Booleano)
            }
            ast::Expressao::StringInterpolada(partes) => {
                let mut result_reg = self.create_global_string("");
                for parte in partes {
                    let part_reg = match parte {
                        ast::PartStringInterpolada::Texto(texto) => {
                            self.create_global_string(texto)
                        }
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
                // Se o objeto é um identificador de classe, trata acesso a membro estático
                if let ast::Expressao::Identificador(class_ident) = &**obj_expr {
                    let fqn = self
                        .type_checker
                        .resolver_nome_classe(class_ident, &self.namespace_path);
                    if self.type_checker.classes.contains_key(&fqn) {
                        // Carrega a partir do global estático
                        // Descobre o tipo do membro pelos metadados de classe resolvidos
                        let member_type = self
                            .infer_member_type(&fqn, membro_nome)
                            .unwrap_or(ast::Tipo::Inteiro);
                        let ty = self.map_type_to_llvm_storage(&member_type);
                        let sym = self.static_global_symbol(&fqn, membro_nome);
                        let loaded_reg = self.get_unique_temp_name();
                        self.body.push_str(&format!(
                            "  {0} = load {1}, {1}* {2}\n",
                            loaded_reg, ty, sym
                        ));
                        return (loaded_reg, member_type);
                    }
                }

                // Caso instância
                let (obj_reg, obj_type) = self.generate_expressao(obj_expr);
                let class_name = match obj_type {
                    ast::Tipo::Classe(name) => name,
                    _ => panic!(
                        "Acesso de membro em algo que não é uma classe: {:?}",
                        obj_type
                    ),
                };
                let (member_ptr_reg, member_type) =
                    self.get_member_ptr(&obj_reg, &class_name, membro_nome);
                let loaded_reg = self.get_unique_temp_name();
                let llvm_type = self.map_type_to_llvm_storage(&member_type);
                let llvm_ptr_type = self.map_type_to_llvm_ptr(&member_type);
                self.body.push_str(&format!(
                    "\n  {0} = load {1}, {2} {3}\n",
                    loaded_reg, llvm_type, llvm_ptr_type, member_ptr_reg
                ));
                (loaded_reg, member_type)
            }
            ast::Expressao::Este => self.load_variable("self"),
            _ => panic!("Expressão não suportada: {:?}", expr),
        }
    }

    fn load_variable(&mut self, name: &str) -> (String, ast::Tipo) {
        if let Some((ptr_reg, var_type)) = self.variables.get(name).cloned() {
            let loaded_reg = self.get_unique_temp_name();
            let llvm_type = self.map_type_to_llvm_storage(&var_type);
            let llvm_ptr_type = self.map_type_to_llvm_ptr(&var_type);
            self.body.push_str(&format!(
                "\n  {0} = load {1}, {2} {3}\n",
                loaded_reg, llvm_type, llvm_ptr_type, ptr_reg
            ));
            return (loaded_reg, var_type);
        }

        if let Some(class_name) = self.classe_atual.clone() {
            if self.resolved_classes.get(&class_name).map_or(false, |c| {
                c.fields.iter().any(|f| f.nome == name)
                    || c.properties.iter().any(|p| p.nome == name)
            }) {
                let (self_ptr_reg, self_type) = self.variables.get("self").unwrap().clone();
                let loaded_self_ptr = self.get_unique_temp_name();
                let self_ptr_type = self.map_type_to_llvm_ptr(&self_type);

                self.body.push_str(&format!(
                    "\n  {0} = load {1}, {1}* {2}\n",
                    loaded_self_ptr, self_ptr_type, self_ptr_reg
                ));

                let (member_ptr_reg, member_type) =
                    self.get_member_ptr(&loaded_self_ptr, &class_name, name);
                let loaded_reg = self.get_unique_temp_name();
                let llvm_type = self.map_type_to_llvm_storage(&member_type);
                let llvm_ptr_type = self.map_type_to_llvm_ptr(&member_type);
                self.body.push_str(&format!(
                    "\n  {0} = load {1}, {2} {3}\n",
                    loaded_reg, llvm_type, llvm_ptr_type, member_ptr_reg
                ));
                return (loaded_reg, member_type);
            }
        }

        panic!("Variável ou membro de classe não declarado: '{}'", name);
    }

    fn get_safe_string_ptr(&mut self, reg: &str) -> String {
        let is_null_reg = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {0} = icmp eq i8* {1}, null\n",
            is_null_reg, reg
        ));

        let empty_str_ptr = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {0} = getelementptr inbounds [1 x i8], [1 x i8]* @.empty_str, i32 0, i32 0\n",
            empty_str_ptr
        ));

        let result_reg = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {0} = select i1 {1}, i8* {2}, i8* {3}\n",
            result_reg, is_null_reg, empty_str_ptr, reg
        ));
        result_reg
    }

    fn ensure_string(&mut self, reg: &str, tipo: &ast::Tipo) -> String {
        match tipo {
            ast::Tipo::Texto => self.get_safe_string_ptr(reg),
            ast::Tipo::Inteiro => self.convert_int_to_string(reg),
            ast::Tipo::Booleano => {
                let true_str = self.create_global_string("verdadeiro");
                let false_str = self.create_global_string("falso");
                let result_reg = self.get_unique_temp_name();
                self.body.push_str(&format!(
                    "  {0} = select i1 {1}, i8* {2}, i8* {3}\n",
                    result_reg, reg, true_str, false_str
                ));
                result_reg
            }
            _ => self.create_global_string("[valor não textual]"),
        }
    }

    fn convert_int_to_string(&mut self, int_reg: &str) -> String {
        let buffer = self.get_unique_temp_name();
        self.body
            .push_str(&format!("  {0} = alloca [21 x i8], align 1\n", buffer));
        let buffer_ptr = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {0} = getelementptr inbounds [21 x i8], [21 x i8]* {1}, i32 0, i32 0\n",
            buffer_ptr, buffer
        ));

        let format_specifier_ptr = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {0} = getelementptr inbounds [3 x i8], [3 x i8]* @.int_fmt, i32 0, i32 0\n",
            format_specifier_ptr
        ));

        self.body.push_str(&format!(
            "  call i32 (i8*, i8*, ...) @sprintf(i8* {0}, i8* {1}, i32 {2})\n",
            buffer_ptr, format_specifier_ptr, int_reg
        ));
        buffer_ptr
    }

    fn concatenate_strings(&mut self, str1_reg: &str, str2_reg: &str) -> String {
        let safe_str1 = self.get_safe_string_ptr(str1_reg);
        let safe_str2 = self.get_safe_string_ptr(str2_reg);

        let len1_reg = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {0} = call i64 @strlen(i8* {1})\n",
            len1_reg, safe_str1
        ));

        let len2_reg = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {0} = call i64 @strlen(i8* {1})\n",
            len2_reg, safe_str2
        ));

        let total_len_reg = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {0} = add i64 {1}, {2}\n",
            total_len_reg, len1_reg, len2_reg
        ));

        let alloc_size_reg = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {0} = add i64 {1}, 1\n",
            alloc_size_reg, total_len_reg
        ));

        let buffer_reg = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {0} = call i8* @malloc(i64 {1})\n",
            buffer_reg, alloc_size_reg
        ));

        let dest_ptr1 = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {0} = getelementptr i8, i8* {1}, i64 0\n",
            dest_ptr1, buffer_reg
        ));
        self.body.push_str(&format!("  call void @llvm.memcpy.p0i8.p0i8.i64(i8* align 1 {0}, i8* align 1 {1}, i64 {2}, i1 false)\n", dest_ptr1, safe_str1, len1_reg));

        let dest_ptr2 = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {0} = getelementptr i8, i8* {1}, i64 {2}\n",
            dest_ptr2, buffer_reg, len1_reg
        ));
        self.body.push_str(&format!("  call void @llvm.memcpy.p0i8.p0i8.i64(i8* align 1 {0}, i8* align 1 {1}, i64 {2}, i1 false)\n", dest_ptr2, safe_str2, len2_reg));

        let null_terminator_ptr = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {0} = getelementptr i8, i8* {1}, i64 {2}\n",
            null_terminator_ptr, buffer_reg, total_len_reg
        ));
        self.body
            .push_str(&format!("  store i8 0, i8* {0}\n", null_terminator_ptr));

        buffer_reg
    }

    fn create_global_string(&mut self, text: &str) -> String {
        let str_len = text.len() + 1;
        let str_name = format!("@.str.{0}", self.string_counter);
        self.string_counter += 1;
        let sanitized_text = text
            .replace('\\', "\\")
            .replace('\n', "\0A")
            .replace('"', "\"");
        self.header.push_str(&format!(
            "{0} = private unnamed_addr constant [{1} x i8] c\"{2}\\00\", align 1\n",
            str_name, str_len, sanitized_text
        ));

        let ptr_register = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {0} = getelementptr inbounds [{1} x i8], [{1} x i8]* {2}, i32 0, i32 0\n",
            ptr_register, str_len, str_name
        ));
        ptr_register
    }

    fn get_unique_temp_name(&mut self) -> String {
        let name = format!("%tmp.{0}", self.temp_counter);
        self.temp_counter += 1;
        name
    }

    fn get_unique_label(&mut self, prefix: &str) -> String {
        let label = format!("{0}.{1}", prefix, self.temp_counter);
        self.temp_counter += 1;
        label
    }

    fn resolve_type(&self, tipo: &ast::Tipo, namespace: &str) -> ast::Tipo {
        if let ast::Tipo::Classe(unresolved_name) = tipo {
            let fqn = self
                .type_checker
                .resolver_nome_classe(unresolved_name, namespace);
            ast::Tipo::Classe(fqn)
        } else {
            tipo.clone()
        }
    }

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

    fn map_type_to_llvm_ptr(&self, tipo: &ast::Tipo) -> String {
        match tipo {
            ast::Tipo::Inteiro => "i32*".to_string(),
            ast::Tipo::Texto => "i8**".to_string(),
            ast::Tipo::Booleano => "i1*".to_string(),
            ast::Tipo::Classe(name) => {
                let sanitized_name = name.replace('.', "_");
                format!("%class.{0}*", sanitized_name)
            }
            _ => panic!("Não é possível criar um ponteiro para o tipo: {:?}", tipo),
        }
    }

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
