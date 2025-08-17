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
    // Mapa: FQN da classe -> lista ordenada de (nome_metodo, FQN_declarante)
    vtables: HashMap<String, Vec<(String, String)>>,
    // Índices rápidos: FQN -> (metodo -> índice)
    vtable_index: HashMap<String, HashMap<String, usize>>,
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
            vtables: HashMap::new(),
            vtable_index: HashMap::new(),
        }
    }

    pub fn generate(&mut self) -> String {
        self.prepare_header();
        // Constrói vtables antes de definir structs
        self.build_all_vtables();
        self.define_all_structs();
        // Define tipos para interfaces como structs mínimos para uso em assinaturas
        self.define_all_interface_structs();
        self.define_all_vtable_globals();
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
        if let Some(fqn) = self.find_principal_function_fqn() {
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

    fn define_all_interface_structs(&mut self) {
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

    fn find_principal_function_fqn(&self) -> Option<String> {
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

    fn define_all_structs(&mut self) {
        let mut fqns: Vec<_> = self.resolved_classes.keys().collect();
        fqns.sort(); // Ordena para garantir uma ordem consistente.

        for fqn in fqns {
            self.define_struct(fqn.as_str());
        }
    }

    fn define_struct(&mut self, fqn: &str) {
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
                let base_name = match nome_base_simples {
                    ast::Tipo::Classe(n) => n.as_str(),
                    ast::Tipo::Aplicado { nome, .. } => nome.as_str(),
                    _ => "",
                };
                let parent_fqn = self
                    .type_checker
                    .resolver_nome_classe(base_name, namespace);

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
        self.header.push_str("declare i32 @scanf(i8*, ...)\n");
        self.header.push_str("declare i8* @malloc(i64)\n");
        self.header
            .push_str("declare i32 @sprintf(i8*, i8*, ...)\n");
        self.header.push_str("declare i64 @strlen(i8*)\n");
        self.header.push_str("declare void @llvm.memcpy.p0i8.p0i8.i64(i8* nocapture writeonly, i8* nocapture readonly, i64, i1 immarg)\n");
        self.header
            .push_str("declare void @llvm.memset.p0i8.i64(i8*, i8, i64, i1)\n");
        // Estrutura genérica de array: { i32 len, i8* data }
        self.header.push_str("%array = type { i32, i8* }\n");
        self.header.push_str(
            "@.println_fmt = private unnamed_addr constant [4 x i8] c\"%s\\0A\\00\", align 1\n",
        );
        self.header
            .push_str("@.int_fmt = private unnamed_addr constant [3 x i8] c\"%d\\00\", align 1\n");
        self.header.push_str(
            "@.float_fmt = private unnamed_addr constant [3 x i8] c\"%f\\00\", align 1\n",
        );
        self.header.push_str(
            "@.double_fmt = private unnamed_addr constant [3 x i8] c\"%f\\00\", align 1\n",
        );
        self.header
            .push_str("@.empty_str = private unnamed_addr constant [1 x i8] c\"\\00\", align 1\n");
        // Formato para ler uma linha inteira (até CR/LF), consumindo finais de linha
        // "%255[^\r\n]%*[\r\n]" em C; em IR usamos escapes hex: \0D (CR) e \0A (LF)
        self.header.push_str("@.scanline_fmt = private unnamed_addr constant [16 x i8] c\"%255[^\\0D\\0A]%*[\\0D\\0A]\\00\", align 1\n");
        self.header.push_str("@.oob_msg = private unnamed_addr constant [23 x i8] c\"Indice fora dos limites\", align 1\n");
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

    fn generate_comando(&mut self, comando: &ast::Comando) {
        match comando {
            ast::Comando::DeclaracaoVar(nome, expr) => {
                let (value_reg, value_type) = self.generate_expressao(expr);
                self.declare_and_store_variable(nome, value_type.clone(), value_type, &value_reg);
            }
            ast::Comando::DeclaracaoVariavel(tipo, nome, Some(expr)) => {
                let (value_reg, value_type) = self.generate_expressao(expr);
                let tipo_resolvido = self.resolve_type(tipo, &self.namespace_path);
                self.declare_and_store_variable(nome, tipo_resolvido, value_type, &value_reg);
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
            ast::Comando::AtribuirIndice(alvo, idx, val) => {
                // Gera: arr_ptr, idx, val; verifica limites e faz store
                let (arr_reg, arr_tipo) = self.generate_expressao(alvo);
                let (idx_reg, _idx_tipo) = self.generate_expressao(idx);
                let (val_reg, val_tipo) = self.generate_expressao(val);
                let elem_tipo = match arr_tipo {
                    ast::Tipo::Lista(boxed) => *boxed,
                    _ => panic!("Atribuição por índice requer array, obtido: {:?}", arr_tipo),
                };
                let (data_ptr, len_reg) = self.get_array_data_and_len(&arr_reg);
                // Bounds check: idx < 0 || idx >= len
                let neg = self.get_unique_temp_name();
                self.body
                    .push_str(&format!("  {0} = icmp slt i32 {1}, 0\n", neg, idx_reg));
                let ge = self.get_unique_temp_name();
                self.body.push_str(&format!(
                    "  {0} = icmp sge i32 {1}, {2}\n",
                    ge, idx_reg, len_reg
                ));
                let oob = self.get_unique_temp_name();
                self.body
                    .push_str(&format!("  {0} = or i1 {1}, {2}\n", oob, neg, ge));
                let ok_label = self.get_unique_label("idx.ok");
                let oob_label = self.get_unique_label("idx.oob");
                let end_label = self.get_unique_label("idx.end");
                self.body.push_str(&format!(
                    "  br i1 {0}, label %{1}, label %{2}\n",
                    oob, oob_label, ok_label
                ));
                // oob path
                self.body.push_str(&format!("{0}:\n", oob_label));
                let msg_ptr = self.get_unique_temp_name();
                self.body.push_str(&format!(
                    "  {0} = getelementptr inbounds [23 x i8], [23 x i8]* @.oob_msg, i32 0, i32 0\n",
                    msg_ptr
                ));
                self.body.push_str(&format!(
                    "  call i32 (i8*, ...) @printf(i8* getelementptr inbounds ([4 x i8], [4 x i8]* @.println_fmt, i32 0, i32 0), i8* {0})\n",
                    msg_ptr
                ));
                self.body.push_str(&format!("  br label %{0}\n", end_label));
                // ok path
                self.body.push_str(&format!("{0}:\n", ok_label));
                let elem_ptr_t = self.map_type_to_llvm_arg(&elem_tipo);
                let casted = self.get_unique_temp_name();
                self.body.push_str(&format!(
                    "  {0} = bitcast i8* {1} to {2}*\n",
                    casted, data_ptr, elem_ptr_t
                ));
                let slot = self.get_unique_temp_name();
                self.body.push_str(&format!(
                    "  {0} = getelementptr inbounds {1}, {1}* {2}, i32 {3}\n",
                    slot, elem_ptr_t, casted, idx_reg
                ));
                let coerced = self.ensure_value_type(&val_reg, &val_tipo, &elem_tipo);
                let elem_store_ty = self.map_type_to_llvm_storage(&elem_tipo);
                self.body.push_str(&format!(
                    "  store {0} {1}, {0}* {2}\n",
                    elem_store_ty, coerced, slot
                ));
                self.body.push_str(&format!("  br label %{0}\n", end_label));
                self.body.push_str(&format!("{0}:\n", end_label));
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
                        // Descobrir tipo declarado da propriedade
                        let member_type = self
                            .infer_member_type(&fqn, prop_nome)
                            .unwrap_or(value_type.clone());
                        let coerced = self.ensure_value_type(&value_reg, &value_type, &member_type);
                        let ty = self.map_type_to_llvm_storage(&member_type);
                        let sym = self.static_global_symbol(&fqn, prop_nome);
                        self.body
                            .push_str(&format!("  store {0} {1}, {0}* {2}\n", ty, coerced, sym));
                        return;
                    }
                }

                // Caso instância
                let (value_reg, value_type) = self.generate_expressao(val_expr);
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
                let coerced = self.ensure_value_type(&value_reg, &value_type, &member_type);
                self.body.push_str(&format!(
                    "  store {0} {1}, {2} {3}\n",
                    llvm_type,
                    coerced,
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

    fn declare_and_store_variable(
        &mut self,
        name: &str,
        var_type: ast::Tipo,
        value_type: ast::Tipo,
        value_reg: &str,
    ) {
        let ptr_reg = format!("%var.{0}", name);
        let llvm_type = self.map_type_to_llvm_storage(&var_type);
        let align = self.get_type_alignment(&var_type);

        self.body.push_str(&format!(
            "  {0} = alloca {1}, align {2}\n",
            ptr_reg, llvm_type, align
        ));
        let coerced = self.ensure_value_type(value_reg, &value_type, &var_type);
        self.body.push_str(&format!(
            "  store {0} {1}, {0}* {2}\n",
            llvm_type, coerced, ptr_reg
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

        // Índice 0 é o vptr; campos começam em 1
        let mut current_index = 1;
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

    fn store_variable(&mut self, name: &str, value_type: &ast::Tipo, value_reg: &str) {
        if let Some((ptr_reg, var_type)) = self.variables.get(name).cloned() {
            let llvm_type = self.map_type_to_llvm_storage(&var_type);
            let coerced = self.ensure_value_type(value_reg, value_type, &var_type);
            self.body.push_str(&format!(
                "  store {0} {1}, {0}* {2}\n",
                llvm_type, coerced, ptr_reg
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
                let coerced = self.ensure_value_type(value_reg, value_type, &member_type);
                self.body.push_str(&format!(
                    "  store {0} {1}, {2} {3}\n",
                    llvm_type,
                    coerced,
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
            ast::Expressao::FlutuanteLiteral(s) => {
                // Remover sufixo f/F e emitir constante float (f32) via fptrunc de double literal
                let raw = s.trim_end_matches('f').trim_end_matches('F');
                let val: f64 = raw.parse().expect("literal flutuante inválido");
                let dbl = format!("{:.6e}", val); // LLVM aceita notação científica
                let tmp = self.get_unique_temp_name();
                self.body
                    .push_str(&format!("  {0} = fptrunc double {1} to float\n", tmp, dbl));
                (tmp, ast::Tipo::Flutuante)
            }
            ast::Expressao::DuploLiteral(s) => {
                // Número de ponto flutuante sem sufixo: tratar como double, em notação científica
                let val: f64 = s.parse().expect("literal duplo inválido");
                let dbl = format!("{:.6e}", val);
                (dbl, ast::Tipo::Duplo)
            }
            ast::Expressao::Decimal(s) => {
                // Armazena decimal como string (removendo sufixo 'm' se presente)
                let printed = s.trim_end_matches('m').trim_end_matches('M').to_string();
                (self.create_global_string(&printed), ast::Tipo::Decimal)
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

                // Promover para o tipo comum e emitir operação correta (inteiro vs float/double)
                use ast::Tipo::*;
                let result_tipo = match (left_type.clone(), right_type.clone()) {
                    (Duplo, _) | (_, Duplo) => Duplo,
                    (Flutuante, _) | (_, Flutuante) => Flutuante,
                    _ => Inteiro,
                };
                let (l, r, llvm_op, llvm_ty) = match result_tipo {
                    Inteiro => {
                        let op_code = match op {
                            ast::OperadorAritmetico::Soma => "add",
                            ast::OperadorAritmetico::Subtracao => "sub",
                            ast::OperadorAritmetico::Multiplicacao => "mul",
                            ast::OperadorAritmetico::Divisao => "sdiv",
                            ast::OperadorAritmetico::Modulo => "srem",
                        };
                        (left_reg, right_reg, op_code.to_string(), "i32".to_string())
                    }
                    Flutuante => {
                        let l = self.ensure_float(&left_reg, &left_type);
                        let r = self.ensure_float(&right_reg, &right_type);
                        let op_code = match op {
                            ast::OperadorAritmetico::Soma => "fadd",
                            ast::OperadorAritmetico::Subtracao => "fsub",
                            ast::OperadorAritmetico::Multiplicacao => "fmul",
                            ast::OperadorAritmetico::Divisao => "fdiv",
                            ast::OperadorAritmetico::Modulo => "frem",
                        };
                        (l, r, op_code.to_string(), "float".to_string())
                    }
                    Duplo => {
                        let l = self.ensure_double(&left_reg, &left_type);
                        let r = self.ensure_double(&right_reg, &right_type);
                        let op_code = match op {
                            ast::OperadorAritmetico::Soma => "fadd",
                            ast::OperadorAritmetico::Subtracao => "fsub",
                            ast::OperadorAritmetico::Multiplicacao => "fmul",
                            ast::OperadorAritmetico::Divisao => "fdiv",
                            ast::OperadorAritmetico::Modulo => "frem",
                        };
                        (l, r, op_code.to_string(), "double".to_string())
                    }
                    _ => unreachable!(),
                };
                let result_reg = self.get_unique_temp_name();
                self.body.push_str(&format!(
                    "  {0} = {1} {2} {3}, {4}\n",
                    result_reg, llvm_op, llvm_ty, l, r
                ));
                (result_reg, result_tipo)
            }
            ast::Expressao::NovoObjeto(nome_classe, argumentos) => {
                let fqn = self
                    .type_checker
                    .resolver_nome_classe(nome_classe, &self.namespace_path);
                // Bloquear instanciação de classe abstrata
                if let Some(class_decl) = self.type_checker.classes.get(&fqn) {
                    if class_decl.eh_abstrata {
                        panic!("Não é possível instanciar classe abstrata: {}", fqn);
                    }
                }
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

                // Inicializa o ponteiro de vtable no primeiro campo
                if let Some(vt_len) = self.vtables.get(&fqn).map(|v| v.len()) {
                    let vt_sym = self.vtable_global_symbol(&fqn);
                    // Obter ponteiro para o primeiro elemento da vtable (i8**)
                    let vt_elem0 = self.get_unique_temp_name();
                    self.body.push_str(&format!(
                        "  {0} = getelementptr inbounds [{1} x i8*], [{1} x i8*]* {2}, i32 0, i32 0\n",
                        vt_elem0,
                        vt_len,
                        vt_sym
                    ));
                    // Escreve vt pointer no primeiro campo da struct
                    let vptr_ptr = self.get_unique_temp_name();
                    self.body.push_str(&format!(
                        "  {0} = bitcast {1} {2} to i8***\n",
                        vptr_ptr, struct_ptr_type, obj_ptr_reg
                    ));
                    self.body.push_str(&format!(
                        "  store i8** {0}, i8*** {1}\n",
                        vt_elem0, vptr_ptr
                    ));
                }

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
            ast::Expressao::ListaLiteral(items) => {
                // Infere tipo de elemento a partir do primeiro item (assumindo homogêneo)
                let (elem0_reg, elem0_tipo) = self.generate_expressao(&items[0]);
                let elem_ty_arg = self.map_type_to_llvm_arg(&elem0_tipo);

                // sizeof(T):
                let gep = self.get_unique_temp_name();
                self.body.push_str(&format!(
                    "  {0} = getelementptr inbounds {1}, {1}* null, i32 1\n",
                    gep, elem_ty_arg
                ));
                let sizeof_t = self.get_unique_temp_name();
                self.body.push_str(&format!(
                    "  {0} = ptrtoint {1} {2} to i64\n",
                    sizeof_t,
                    format!("{0}*", elem_ty_arg),
                    gep
                ));

                // total size = len * sizeof(T)
                let len = items.len();
                let total_size = self.get_unique_temp_name();
                self.body.push_str(&format!(
                    "  {0} = mul i64 {1}, {2}\n",
                    total_size, sizeof_t, len
                ));
                let data_i8 = self.get_unique_temp_name();
                self.body.push_str(&format!(
                    "  {0} = call i8* @malloc(i64 {1})\n",
                    data_i8, total_size
                ));
                // Escrever elementos
                let data_typed = self.get_unique_temp_name();
                self.body.push_str(&format!(
                    "  {0} = bitcast i8* {1} to {2}*\n",
                    data_typed, data_i8, elem_ty_arg
                ));
                // store o primeiro
                let coerced0 = self.ensure_value_type(&elem0_reg, &elem0_tipo, &elem0_tipo);
                let slot0 = self.get_unique_temp_name();
                self.body.push_str(&format!(
                    "  {0} = getelementptr inbounds {1}, {1}* {2}, i32 0\n",
                    slot0, elem_ty_arg, data_typed
                ));
                let elem_store_ty = self.map_type_to_llvm_storage(&elem0_tipo);
                self.body.push_str(&format!(
                    "  store {0} {1}, {0}* {2}\n",
                    elem_store_ty, coerced0, slot0
                ));
                for (i, it) in items.iter().enumerate().skip(1) {
                    let (r, t) = self.generate_expressao(it);
                    let coerced = self.ensure_value_type(&r, &t, &elem0_tipo);
                    let slot = self.get_unique_temp_name();
                    self.body.push_str(&format!(
                        "  {0} = getelementptr inbounds {1}, {1}* {2}, i32 {3}\n",
                        slot, elem_ty_arg, data_typed, i
                    ));
                    self.body.push_str(&format!(
                        "  store {0} {1}, {0}* {2}\n",
                        elem_store_ty, coerced, slot
                    ));
                }

                // Aloca e preenche header %array
                let array_size_gep = self.get_unique_temp_name();
                self.body.push_str("  ");
                self.body.push_str(&format!(
                    "{0} = getelementptr inbounds %array, %array* null, i32 1\n",
                    array_size_gep
                ));
                let array_size = self.get_unique_temp_name();
                self.body.push_str(&format!(
                    "  {0} = ptrtoint %array* {1} to i64\n",
                    array_size, array_size_gep
                ));
                let array_mem = self.get_unique_temp_name();
                self.body.push_str(&format!(
                    "  {0} = call i8* @malloc(i64 {1})\n",
                    array_mem, array_size
                ));
                let array_ptr = self.get_unique_temp_name();
                self.body.push_str(&format!(
                    "  {0} = bitcast i8* {1} to %array*\n",
                    array_ptr, array_mem
                ));
                // campos: [0] len, [1] data
                let len_ptr = self.get_unique_temp_name();
                self.body.push_str(&format!(
                    "  {0} = getelementptr inbounds %array, %array* {1}, i32 0, i32 0\n",
                    len_ptr, array_ptr
                ));
                self.body
                    .push_str(&format!("  store i32 {0}, i32* {1}\n", len, len_ptr));
                let data_ptr_ptr = self.get_unique_temp_name();
                self.body.push_str(&format!(
                    "  {0} = getelementptr inbounds %array, %array* {1}, i32 0, i32 1\n",
                    data_ptr_ptr, array_ptr
                ));
                self.body.push_str(&format!(
                    "  store i8* {0}, i8** {1}\n",
                    data_i8, data_ptr_ptr
                ));

                (array_ptr, ast::Tipo::Lista(Box::new(elem0_tipo)))
            }
            ast::Expressao::AcessoIndice(obj, idx) => {
                let (arr_reg, arr_tipo) = self.generate_expressao(obj);
                let (idx_reg, _idx_tipo) = self.generate_expressao(idx);
                let elem_tipo = match arr_tipo.clone() {
                    ast::Tipo::Lista(boxed) => *boxed,
                    _ => panic!("Acesso por índice requer array, obtido: {:?}", arr_tipo),
                };
                let (data_ptr, len_reg) = self.get_array_data_and_len(&arr_reg);
                // Bounds
                let neg = self.get_unique_temp_name();
                self.body
                    .push_str(&format!("  {0} = icmp slt i32 {1}, 0\n", neg, idx_reg));
                let ge = self.get_unique_temp_name();
                self.body.push_str(&format!(
                    "  {0} = icmp sge i32 {1}, {2}\n",
                    ge, idx_reg, len_reg
                ));
                let oob = self.get_unique_temp_name();
                self.body
                    .push_str(&format!("  {0} = or i1 {1}, {2}\n", oob, neg, ge));
                let ok_label = self.get_unique_label("idx.ok");
                let oob_label = self.get_unique_label("idx.oob");
                let end_label = self.get_unique_label("idx.end");
                self.body.push_str(&format!(
                    "  br i1 {0}, label %{1}, label %{2}\n",
                    oob, oob_label, ok_label
                ));
                // oob
                self.body.push_str(&format!("{0}:\n", oob_label));
                let msg_ptr = self.get_unique_temp_name();
                self.body.push_str(&format!(
                    "  {0} = getelementptr inbounds [23 x i8], [23 x i8]* @.oob_msg, i32 0, i32 0\n",
                    msg_ptr
                ));
                self.body.push_str(&format!(
                    "  call i32 (i8*, ...) @printf(i8* getelementptr inbounds ([4 x i8], [4 x i8]* @.println_fmt, i32 0, i32 0), i8* {0})\n",
                    msg_ptr
                ));
                // valor padrão
                let default_reg = self.zero_value_of(&elem_tipo);
                self.body.push_str(&format!("  br label %{0}\n", end_label));
                // ok
                self.body.push_str(&format!("{0}:\n", ok_label));
                let elem_ty_arg = self.map_type_to_llvm_arg(&elem_tipo);
                let casted = self.get_unique_temp_name();
                self.body.push_str(&format!(
                    "  {0} = bitcast i8* {1} to {2}*\n",
                    casted, data_ptr, elem_ty_arg
                ));
                let slot = self.get_unique_temp_name();
                self.body.push_str(&format!(
                    "  {0} = getelementptr inbounds {1}, {1}* {2}, i32 {3}\n",
                    slot, elem_ty_arg, casted, idx_reg
                ));
                let loaded = self.get_unique_temp_name();
                let elem_store_ty = self.map_type_to_llvm_storage(&elem_tipo);
                self.body.push_str(&format!(
                    "  {0} = load {1}, {1}* {2}\n",
                    loaded, elem_store_ty, slot
                ));
                let phi = self.get_unique_temp_name();
                // phi do resultado
                self.body.push_str(&format!(
                    "  br label %{0}\n{0}:\n  {1} = phi {2} [ {3}, %{4} ], [ {5}, %{6} ]\n",
                    end_label,
                    phi,
                    self.map_type_to_llvm_arg(&elem_tipo),
                    default_reg,
                    oob_label,
                    loaded,
                    ok_label
                ));
                (phi, elem_tipo)
            }
            ast::Expressao::Chamada(nome_funcao, argumentos) => {
                let fqn_func_name = self
                    .type_checker
                    .resolver_nome_funcao(nome_funcao, &self.namespace_path);

                // Intrínsecos no backend LLVM: EscreverLinha(...) e LerLinha()
                // - EscreverLinha: converte todos argumentos para string, concatena e imprime via printf com newline.
                // - LerLinha: retorna string vazia por enquanto (somente geração de IR é necessária nos testes).
                let short_name = fqn_func_name
                    .rsplit('.')
                    .next()
                    .unwrap_or(fqn_func_name.as_str());
                if short_name == "EscreverLinha" {
                    // Converte args para i8* e concatena
                    let mut partes: Vec<String> = Vec::new();
                    for arg in argumentos {
                        let (areg, atype) = self.generate_expressao(arg);
                        let as_str = self.ensure_string(&areg, &atype);
                        partes.push(as_str);
                    }

                    // Gera string final (ou vazia)
                    let final_ptr = if partes.is_empty() {
                        let empty_ptr = self.get_unique_temp_name();
                        self.body.push_str(&format!(
                            "  {0} = getelementptr inbounds [1 x i8], [1 x i8]* @.empty_str, i32 0, i32 0\n",
                            empty_ptr
                        ));
                        empty_ptr
                    } else {
                        let mut acc = partes[0].clone();
                        for p in partes.iter().skip(1) {
                            acc = self.concatenate_strings(&acc, p);
                        }
                        acc
                    };

                    // printf("%s\n", final_ptr)
                    self.body.push_str(&format!(
                        "  call i32 (i8*, ...) @printf(i8* getelementptr inbounds ([4 x i8], [4 x i8]* @.println_fmt, i32 0, i32 0), i8* {0})\n",
                        final_ptr
                    ));
                    return ("".to_string(), ast::Tipo::Vazio);
                }
                if short_name == "LerLinha" {
                    // Implementação real usando scanf("%255[^\r\n]%*[\r\n]", buffer)
                    // 1) Aloca um buffer local [256 x i8]
                    let buf_alloca = self.get_unique_temp_name();
                    self.body
                        .push_str(&format!("  {0} = alloca [256 x i8], align 1\n", buf_alloca));
                    // 2) GEP para i8* do início
                    let buf_ptr = self.get_unique_temp_name();
                    self.body.push_str(&format!(
                        "  {0} = getelementptr inbounds [256 x i8], [256 x i8]* {1}, i32 0, i32 0\n",
                        buf_ptr, buf_alloca
                    ));
                    // 3) scanf no buffer
                    self.body.push_str(&format!(
                        "  call i32 (i8*, ...) @scanf(i8* getelementptr inbounds ([16 x i8], [16 x i8]* @.scanline_fmt, i32 0, i32 0), i8* {0})\n",
                        buf_ptr
                    ));
                    // 4) Retorna i8* para o buffer
                    return (buf_ptr, ast::Tipo::Texto);
                }
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
                    });

                let func = match func.or_else(|| {
                    self.programa.namespaces.iter().find_map(|ns| {
                        ns.declaracoes.iter().find_map(|d| match d {
                            ast::Declaracao::DeclaracaoFuncao(f)
                                if self.type_checker.resolver_nome_funcao(&f.nome, &ns.nome)
                                    == fqn_func_name =>
                            {
                                Some(f)
                            }
                            _ => None,
                        })
                    })
                }) {
                    Some(f) => f,
                    None => panic!(
                        "Função '{}' não encontrada nem como intrínseca nem no código do usuário",
                        fqn_func_name
                    ),
                };
                let return_type_decl = func.tipo_retorno.clone().unwrap_or(ast::Tipo::Vazio);
                let return_type = self.resolve_type(&return_type_decl, &self.namespace_path);
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
                // Suporte a intrínsecos: tamanho()/comprimento() em listas e textos
                if (metodo_nome == "tamanho" || metodo_nome == "comprimento")
                    && argumentos.is_empty()
                {
                    match obj_type.clone() {
                        ast::Tipo::Lista(_) => {
                            let (_data, len_reg) = self.get_array_data_and_len(&obj_reg);
                            return (len_reg, ast::Tipo::Inteiro);
                        }
                        ast::Tipo::Texto => {
                            let safe = self.get_safe_string_ptr(&obj_reg);
                            let len64 = self.get_unique_temp_name();
                            self.body.push_str(&format!(
                                "  {0} = call i64 @strlen(i8* {1})\n",
                                len64, safe
                            ));
                            let len32 = self.get_unique_temp_name();
                            self.body
                                .push_str(&format!("  {0} = trunc i64 {1} to i32\n", len32, len64));
                            return (len32, ast::Tipo::Inteiro);
                        }
                        _ => {}
                    }
                }

                let class_name = match obj_type {
                    ast::Tipo::Classe(ref name) => name.clone(),
                    _ => panic!(
                        "Chamada de método em algo que não é um objeto. metodo='{}' obj_type={:?} obj_expr={:?}",
                        metodo_nome,
                        obj_type,
                        obj_expr
                    ),
                };

                let fqn_class_name = self
                    .type_checker
                    .resolver_nome_classe(&class_name, &self.namespace_path);
                // Determina se é virtual (tem índice de vtable)
                let vtable_idx_opt = self
                    .vtable_index
                    .get(&fqn_class_name)
                    .and_then(|m| m.get(metodo_nome).cloned());

                // Resolve tipo de retorno pela classe estática
                let resolved_method = self
                    .resolved_classes
                    .get(&fqn_class_name)
                    .and_then(|c| c.methods.get(metodo_nome))
                    .cloned()
                    .unwrap_or_else(|| {
                        panic!(
                            "Método '{}' não encontrado em '{}'",
                            metodo_nome, fqn_class_name
                        )
                    });
                let return_type = resolved_method
                    .tipo_retorno
                    .clone()
                    .unwrap_or(ast::Tipo::Vazio);
                let return_type_llvm = self.map_type_to_llvm_arg(&return_type);

                // Prepara argumentos
                let obj_ptr_type = self.map_type_to_llvm_ptr(&obj_type);
                let mut args_llvm_sig: Vec<String> = Vec::new();
                let mut args_values: Vec<(String, ast::Tipo)> = Vec::new();
                args_llvm_sig.push(obj_ptr_type.clone());
                args_values.push((obj_reg.clone(), obj_type.clone()));
                for arg in argumentos {
                    let (arg_reg, arg_type) = self.generate_expressao(arg);
                    args_llvm_sig.push(self.map_type_to_llvm_arg(&arg_type));
                    args_values.push((arg_reg, arg_type));
                }

                if let Some(vt_index) = vtable_idx_opt {
                    // Chamada indireta via vtable
                    // Carrega vptr do objeto
                    let vptr_ptr = self.get_unique_temp_name();
                    self.body.push_str(&format!(
                        "  {0} = bitcast {1} {2} to i8***\n",
                        vptr_ptr, obj_ptr_type, obj_reg
                    ));
                    let vptr = self.get_unique_temp_name();
                    self.body
                        .push_str(&format!("  {0} = load i8**, i8*** {1}\n", vptr, vptr_ptr));
                    // Acessa slot da vtable
                    let slot_ptr = self.get_unique_temp_name();
                    self.body.push_str(&format!(
                        "  {0} = getelementptr inbounds i8*, i8** {1}, i32 {2}\n",
                        slot_ptr, vptr, vt_index
                    ));
                    let fn_i8 = self.get_unique_temp_name();
                    self.body
                        .push_str(&format!("  {0} = load i8*, i8** {1}\n", fn_i8, slot_ptr));
                    // Monta o tipo de função esperado: ret (Tself, args...)*
                    let fn_ty = format!("{0} ({1})*", return_type_llvm, args_llvm_sig.join(", "));
                    let fn_typed = self.get_unique_temp_name();
                    self.body.push_str(&format!(
                        "  {0} = bitcast i8* {1} to {2}\n",
                        fn_typed, fn_i8, fn_ty
                    ));
                    // Chamada indireta
                    let args_vals: Vec<String> = args_values
                        .iter()
                        .map(|(reg, ty)| format!("{0} {1}", self.map_type_to_llvm_arg(ty), reg))
                        .collect();
                    let call_sig = args_vals.join(", ");
                    if return_type == ast::Tipo::Vazio {
                        self.body.push_str(&format!(
                            "  call {0} {1}({2})\n",
                            return_type_llvm, fn_typed, call_sig
                        ));
                        ("".to_string(), return_type)
                    } else {
                        let result_reg = self.get_unique_temp_name();
                        self.body.push_str(&format!(
                            "  {0} = call {1} {2}({3})\n",
                            result_reg, return_type_llvm, fn_typed, call_sig
                        ));
                        (result_reg, return_type)
                    }
                } else {
                    // Não-virtual: chamada direta
                    let declaring_class = self
                        .get_declaring_class_of_method(resolved_method)
                        .unwrap_or_else(|| fqn_class_name.clone());
                    let fqn_method =
                        format!("{0}::{1}", declaring_class, metodo_nome).replace('.', "_");
                    let args_vals: Vec<String> =
                        std::iter::once((obj_reg.clone(), obj_type.clone()))
                            .chain(args_values.into_iter())
                            .map(|(reg, ty)| {
                                format!("{0} {1}", self.map_type_to_llvm_arg(&ty), reg)
                            })
                            .collect();
                    let args_str = args_vals.join(", ");
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
            }
            ast::Expressao::Comparacao(op, esq, dir) => {
                let (mut left_reg, left_type) = self.generate_expressao(esq);
                let (mut right_reg, right_type) = self.generate_expressao(dir);
                use ast::Tipo::*;
                let result_reg = self.get_unique_temp_name();
                match (left_type.clone(), right_type.clone()) {
                    (Inteiro | Booleano | Enum(_), Inteiro | Booleano | Enum(_)) => {
                        let op_str = match op {
                            ast::OperadorComparacao::Igual => "eq",
                            ast::OperadorComparacao::Diferente => "ne",
                            ast::OperadorComparacao::Menor => "slt",
                            ast::OperadorComparacao::MaiorQue => "sgt",
                            ast::OperadorComparacao::MenorIgual => "sle",
                            ast::OperadorComparacao::MaiorIgual => "sge",
                        };
                        self.body.push_str(&format!(
                            "  {0} = icmp {1} i32 {2}, {3}\n",
                            result_reg, op_str, left_reg, right_reg
                        ));
                    }
                    (Duplo, _) | (_, Duplo) => {
                        left_reg = self.ensure_double(&left_reg, &left_type);
                        right_reg = self.ensure_double(&right_reg, &right_type);
                        let pred = match op {
                            ast::OperadorComparacao::Igual => "oeq",
                            ast::OperadorComparacao::Diferente => "one",
                            ast::OperadorComparacao::Menor => "olt",
                            ast::OperadorComparacao::MaiorQue => "ogt",
                            ast::OperadorComparacao::MenorIgual => "ole",
                            ast::OperadorComparacao::MaiorIgual => "oge",
                        };
                        self.body.push_str(&format!(
                            "  {0} = fcmp {1} double {2}, {3}\n",
                            result_reg, pred, left_reg, right_reg
                        ));
                    }
                    (Flutuante, _) | (_, Flutuante) => {
                        left_reg = self.ensure_float(&left_reg, &left_type);
                        right_reg = self.ensure_float(&right_reg, &right_type);
                        let pred = match op {
                            ast::OperadorComparacao::Igual => "oeq",
                            ast::OperadorComparacao::Diferente => "one",
                            ast::OperadorComparacao::Menor => "olt",
                            ast::OperadorComparacao::MaiorQue => "ogt",
                            ast::OperadorComparacao::MenorIgual => "ole",
                            ast::OperadorComparacao::MaiorIgual => "oge",
                        };
                        self.body.push_str(&format!(
                            "  {0} = fcmp {1} float {2}, {3}\n",
                            result_reg, pred, left_reg, right_reg
                        ));
                    }
                    _ => panic!(
                        "Comparação não suportada entre tipos: {:?} e {:?}",
                        left_type, right_type
                    ),
                }
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
                    // Se for enumeração, emitir o valor inteiro da posição do membro
                    let fqn_enum = self
                        .type_checker
                        .resolver_nome_enum(class_ident, &self.namespace_path);
                    if let Some(en) = self.type_checker.enums.get(&fqn_enum) {
                        if let Some(idx) = en.valores.iter().position(|v| v == membro_nome) {
                            return (idx.to_string(), ast::Tipo::Enum(fqn_enum));
                        }
                    }
                }
                // Caso instância: agora podemos avaliar o objeto
                let (obj_reg, obj_type) = self.generate_expressao(obj_expr);
                // Propriedade especial: tamanho/comprimento em arrays e textos
                if membro_nome == "tamanho" || membro_nome == "comprimento" {
                    match obj_type {
                        ast::Tipo::Lista(_) => {
                            let (_data, len_reg) = self.get_array_data_and_len(&obj_reg);
                            return (len_reg, ast::Tipo::Inteiro);
                        }
                        ast::Tipo::Texto => {
                            let safe = self.get_safe_string_ptr(&obj_reg);
                            let len64 = self.get_unique_temp_name();
                            self.body.push_str(&format!(
                                "  {0} = call i64 @strlen(i8* {1})\n",
                                len64, safe
                            ));
                            let len32 = self.get_unique_temp_name();
                            self.body
                                .push_str(&format!("  {0} = trunc i64 {1} to i32\n", len32, len64));
                            return (len32, ast::Tipo::Inteiro);
                        }
                        _ => {}
                    }
                }
                // obj_reg e obj_type já calculados acima
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
            ast::Tipo::Decimal => self.get_safe_string_ptr(reg),
            ast::Tipo::Inteiro => self.convert_int_to_string(reg),
            ast::Tipo::Enum(_) => self.convert_int_to_string(reg),
            ast::Tipo::Flutuante => self.convert_float_to_string(reg),
            ast::Tipo::Duplo => self.convert_double_to_string(reg),
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

    // Garante que o valor esteja no tipo esperado (apenas numéricos básicos por enquanto)
    fn ensure_value_type(&mut self, reg: &str, from: &ast::Tipo, to: &ast::Tipo) -> String {
        use ast::Tipo::*;
        match (from, to) {
            (f, t) if f == t => reg.to_string(),
            (Inteiro, Flutuante) => {
                let tmp = self.get_unique_temp_name();
                self.body
                    .push_str(&format!("  {0} = sitofp i32 {1} to float\n", tmp, reg));
                tmp
            }
            (Inteiro, Duplo) => {
                let tmp = self.get_unique_temp_name();
                self.body
                    .push_str(&format!("  {0} = sitofp i32 {1} to double\n", tmp, reg));
                tmp
            }
            (Flutuante, Duplo) => {
                let tmp = self.get_unique_temp_name();
                self.body
                    .push_str(&format!("  {0} = fpext float {1} to double\n", tmp, reg));
                tmp
            }
            (Duplo, Flutuante) => {
                let tmp = self.get_unique_temp_name();
                self.body
                    .push_str(&format!("  {0} = fptrunc double {1} to float\n", tmp, reg));
                tmp
            }
            _ => reg.to_string(),
        }
    }

    fn convert_float_to_string(&mut self, f_reg: &str) -> String {
        let buffer = self.get_unique_temp_name();
        self.body
            .push_str(&format!("  {0} = alloca [64 x i8], align 1\n", buffer));
        let buffer_ptr = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {0} = getelementptr inbounds [64 x i8], [64 x i8]* {1}, i32 0, i32 0\n",
            buffer_ptr, buffer
        ));
        let fmt_ptr = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {0} = getelementptr inbounds [3 x i8], [3 x i8]* @.float_fmt, i32 0, i32 0\n",
            fmt_ptr
        ));
        let as_double = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {0} = fpext float {1} to double\n",
            as_double, f_reg
        ));
        self.body.push_str(&format!(
            "  call i32 (i8*, i8*, ...) @sprintf(i8* {0}, i8* {1}, double {2})\n",
            buffer_ptr, fmt_ptr, as_double
        ));
        buffer_ptr
    }

    fn convert_double_to_string(&mut self, d_reg: &str) -> String {
        let buffer = self.get_unique_temp_name();
        self.body
            .push_str(&format!("  {0} = alloca [64 x i8], align 1\n", buffer));
        let buffer_ptr = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {0} = getelementptr inbounds [64 x i8], [64 x i8]* {1}, i32 0, i32 0\n",
            buffer_ptr, buffer
        ));
        let fmt_ptr = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {0} = getelementptr inbounds [3 x i8], [3 x i8]* @.double_fmt, i32 0, i32 0\n",
            fmt_ptr
        ));
        self.body.push_str(&format!(
            "  call i32 (i8*, i8*, ...) @sprintf(i8* {0}, i8* {1}, double {2})\n",
            buffer_ptr, fmt_ptr, d_reg
        ));
        buffer_ptr
    }

    fn ensure_float(&mut self, reg: &str, tipo: &ast::Tipo) -> String {
        match tipo {
            ast::Tipo::Flutuante => reg.to_string(),
            ast::Tipo::Inteiro => {
                let tmp = self.get_unique_temp_name();
                self.body
                    .push_str(&format!("  {0} = sitofp i32 {1} to float\n", tmp, reg));
                tmp
            }
            ast::Tipo::Duplo => {
                let tmp = self.get_unique_temp_name();
                self.body
                    .push_str(&format!("  {0} = fptrunc double {1} to float\n", tmp, reg));
                tmp
            }
            _ => panic!("Conversão para float não suportada: {:?}", tipo),
        }
    }

    fn ensure_double(&mut self, reg: &str, tipo: &ast::Tipo) -> String {
        match tipo {
            ast::Tipo::Duplo => reg.to_string(),
            ast::Tipo::Inteiro => {
                let tmp = self.get_unique_temp_name();
                self.body
                    .push_str(&format!("  {0} = sitofp i32 {1} to double\n", tmp, reg));
                tmp
            }
            ast::Tipo::Flutuante => {
                let tmp = self.get_unique_temp_name();
                self.body
                    .push_str(&format!("  {0} = fpext float {1} to double\n", tmp, reg));
                tmp
            }
            _ => panic!("Conversão para double não suportada: {:?}", tipo),
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
            ast::Tipo::Aplicado { nome, args: _ } => {
                let fqn_class = self.type_checker.resolver_nome_classe(nome, namespace);
                ast::Tipo::Classe(fqn_class)
            }
            other => other.clone(),
        }
    }

    fn map_type_to_llvm_storage(&self, tipo: &ast::Tipo) -> String {
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

    fn map_type_to_llvm_ptr(&self, tipo: &ast::Tipo) -> String {
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
                let sanitized_name = nome.replace('.', "_");
                format!("%class.{0}*", sanitized_name)
            }
            ast::Tipo::Lista(_) => "%array**".to_string(),
            _ => panic!("Não é possível criar um ponteiro para o tipo: {:?}", tipo),
        }
    }

    fn map_type_to_llvm_arg(&self, tipo: &ast::Tipo) -> String {
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
    fn vtable_global_symbol(&self, fqn_class: &str) -> String {
        format!("@.vtable.{}", fqn_class.replace('.', "_"))
    }

    fn build_all_vtables(&mut self) {
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
            self.vtable_index.insert(fqn.clone(), index);
            self.vtables.insert(fqn, entries);
        }
    }

    fn compute_vtable_for(&self, fqn: &str) -> Vec<(String, String)> {
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

    fn define_all_vtable_globals(&mut self) {
        let mut fqns: Vec<_> = self.vtables.keys().cloned().collect();
        fqns.sort();
        for fqn in fqns {
            let entries = self.vtables.get(&fqn).cloned().unwrap_or_default();
            let sym = self.vtable_global_symbol(&fqn);
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

    fn get_namespace_from_fqn(&self, full: &str) -> String {
        full.rsplit_once('.')
            .map(|(ns, _)| ns.to_string())
            .unwrap_or_default()
    }

    fn get_namespace_from_full_name(&self, full: &str) -> String {
        self.get_namespace_from_fqn(full)
    }

    // Helpers para arrays
    fn get_array_data_and_len(&mut self, arr_ptr_reg: &str) -> (String, String) {
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

    fn zero_value_of(&mut self, tipo: &ast::Tipo) -> String {
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
