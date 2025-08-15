use crate::ast;
use std::collections::{HashMap, HashSet};
use std::fmt;

impl fmt::Display for ast::Expressao {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ast::Expressao::Identificador(s) => write!(f, "{}", s),
            ast::Expressao::Este => write!(f, "este"),
            _ => write!(f, "<expressao>"),
        }
    }
}

fn get_expr_name(expr: &ast::Expressao) -> Option<String> {
    match expr {
        ast::Expressao::Identificador(s) => Some(s.clone()),
        ast::Expressao::Este => Some("este".to_string()),
        _ => None,
    }
}

/// O gerador de código para o alvo Bytecode.
pub struct BytecodeGenerator<'a> {
    programa: &'a ast::Programa,
    type_checker: &'a crate::type_checker::VerificadorTipos<'a>,
    namespace_path: String,
    bytecode_instructions: Vec<String>,
    props_por_classe: HashMap<String, Vec<String>>,
    construtor_params_por_classe: HashMap<String, Vec<String>>,
    current_class_name: Option<String>,
    // Parâmetros locais do método/construtor atual (para desambiguar nome igual a propriedade)
    current_params: Option<HashSet<String>>,
}

impl<'a> BytecodeGenerator<'a> {
    fn spawn_child(&self) -> Self {
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
    fn get_class_declaration(&self, class_name: &str) -> Option<&'a ast::DeclaracaoClasse> {
        self.type_checker.classes.get(class_name).copied()
    }

    fn qual(&self, local: &str) -> String {
        if self.namespace_path.is_empty() {
            local.to_owned()
        } else {
            format!("{}.{}", self.namespace_path, local)
        }
    }

    pub fn new(
        programa: &'a ast::Programa,
        type_checker: &'a crate::type_checker::VerificadorTipos,
    ) -> Self {
        Self {
            programa,
            type_checker,
            namespace_path: String::new(),
            bytecode_instructions: Vec::new(),
            props_por_classe: HashMap::new(),
            construtor_params_por_classe: HashMap::new(),
            current_class_name: None,
            current_params: None,
        }
    }

    fn is_string_expr(expr: &ast::Expressao) -> bool {
        use ast::{Expressao as E, OperadorAritmetico as OA};
        match expr {
            E::Texto(_) | E::StringInterpolada(_) => true,
            E::Aritmetica(OA::Soma, l, r) => Self::is_string_expr(l) || Self::is_string_expr(r),
            _ => false,
        }
    }

    fn gerar_construtor(&mut self, ctor: &ast::ConstrutorClasse, nome_classe: &str) {
        let sub_prog = ast::Programa {
            usings: vec![],
            namespaces: vec![],
            declaracoes: vec![ast::Declaracao::Comando(ast::Comando::Bloco(
                ctor.corpo.clone(),
            ))],
        };
        let mut sub = BytecodeGenerator {
            programa: &sub_prog,
            type_checker: self.type_checker,
            namespace_path: self.namespace_path.clone(),
            bytecode_instructions: Vec::new(),
            props_por_classe: self.props_por_classe.clone(),
            construtor_params_por_classe: self.construtor_params_por_classe.clone(),
            current_class_name: Some(nome_classe.to_string()),
            current_params: Some(
                ctor.parametros
                    .iter()
                    .map(|p| p.nome.clone())
                    .collect::<HashSet<String>>(),
            ),
        };
        let corpo = sub.generate();
        let mut corpo_com_defaults = Vec::new();
        if let Some(base_args) = &ctor.chamada_pai {
            let mut temp_gen = self.spawn_child();
            for arg in base_args {
                temp_gen.generate_expressao(arg);
            }
            corpo_com_defaults.extend(temp_gen.bytecode_instructions);
            corpo_com_defaults.push(format!("CALL_BASE_CONSTRUCTOR {}", base_args.len()));
        }

        for p in &ctor.parametros {
            if let Some(default_expr) = &p.valor_padrao {
                let mut temp_gen = self.spawn_child();
                temp_gen.generate_expressao(default_expr);
                corpo_com_defaults.push(format!(
                    "SET_DEFAULT {} {}",
                    p.nome,
                    temp_gen.bytecode_instructions.join(" ")
                ));
            }
        }
        corpo_com_defaults.extend(corpo);
        let corpo = corpo_com_defaults;
        let params: Vec<String> = ctor
            .parametros
            .iter()
            .map(|p| {
                let mut param_str = p.nome.clone();
                if let Some(default_expr) = &p.valor_padrao {
                    param_str.push_str(&format!("={}", default_expr));
                }
                param_str
            })
            .collect();

        self.bytecode_instructions.push(format!(
            "DEFINE_METHOD {} {} {} {}",
            nome_classe,
            "construtor",
            corpo.len(),
            params.join(" ")
        ));
        self.bytecode_instructions.extend(corpo);
    }

    fn generate_declaracao(&mut self, declaracao: &ast::Declaracao) {
        match declaracao {
            // ===== namespace =====
            ast::Declaracao::DeclaracaoNamespace(ns) => {
                let new_path = if self.namespace_path.is_empty() {
                    ns.nome.clone()
                } else {
                    format!("{}.{}", self.namespace_path, ns.nome)
                };
                let sub_prog = ast::Programa {
                    usings: vec![],
                    namespaces: vec![],
                    declaracoes: ns.declaracoes.clone(),
                };
                let mut sub = BytecodeGenerator {
                    programa: &sub_prog,
                    type_checker: self.type_checker,
                    namespace_path: new_path,
                    bytecode_instructions: Vec::new(),
                    props_por_classe: self.props_por_classe.clone(),
                    construtor_params_por_classe: self.construtor_params_por_classe.clone(),
                    current_class_name: None,
                    current_params: None,
                };
                self.bytecode_instructions.extend(sub.generate());
            }

            // Reconhece e processa a declaração de classe
            ast::Declaracao::DeclaracaoClasse(classe_def) => {
                let full_class_name = self.qual(&classe_def.nome);
                let parent_class_name =
                    classe_def
                        .classe_pai
                        .as_ref()
                        .map_or("NULO".to_string(), |p| {
                            self.type_checker
                                .resolver_nome_classe(p, &self.namespace_path)
                        });

                let mut all_props = self
                    .props_por_classe
                    .get(&parent_class_name)
                    .cloned()
                    .unwrap_or_default();
                all_props.extend(classe_def.propriedades.iter().map(|p| p.nome.clone()));
                all_props.extend(classe_def.campos.iter().map(|c| c.nome.clone()));
                self.props_por_classe
                    .insert(full_class_name.clone(), all_props.clone());

                // Utilize vírgula como separador para evitar que "split_whitespace" quebre o token na carga do interpretador
                let props_str = all_props.join(",");

                // Coleta informações do primeiro construtor (se existir) para exportar metadados
                let (params_str, base_args_str) =
                    if let Some(ctor) = classe_def.construtores.first() {
                        let params: Vec<String> =
                            ctor.parametros.iter().map(|p| p.nome.clone()).collect();
                        let base_args: Vec<String> = ctor
                            .chamada_pai
                            .as_ref()
                            .map(|args| args.iter().filter_map(get_expr_name).collect())
                            .unwrap_or_else(Vec::new);
                        (params.join(","), base_args.join(","))
                    } else {
                        (String::new(), String::new())
                    };

                // Monta o campo combinado separado por '|': propriedades|params|baseArgs|corpo (vazio)
                let meta_str = format!("{}|{}|{}|", props_str, params_str, base_args_str);

                self.bytecode_instructions.push(format!(
                    "DEFINE_CLASS {} {} {}",
                    full_class_name, parent_class_name, meta_str
                ));

                for ctor in &classe_def.construtores {
                    self.gerar_construtor(ctor, &full_class_name);
                }

                for metodo in &classe_def.metodos {
                    if metodo.eh_abstrato {
                        continue; // não gera corpo nem entrada para métodos abstratos
                    }
                    self.gerar_metodo(metodo, &full_class_name);
                }

                // Marca o fim da declaração da classe
                self.bytecode_instructions.push("END_CLASS".to_string());

                // ===== Inicializadores de propriedades/campos estáticos =====
                for campo in &classe_def.campos {
                    if campo.eh_estatica {
                        if let Some(expr) = &campo.valor_inicial {
                            // Gera código para empilhar valor inicial
                            let mut temp_gen = self.spawn_child();
                            temp_gen.generate_expressao(expr);
                            self.bytecode_instructions
                                .extend(temp_gen.bytecode_instructions);
                            // Executa atribuição no tempo de inicialização
                            self.bytecode_instructions.push(format!(
                                "SET_STATIC_PROPERTY {} {}",
                                full_class_name, campo.nome
                            ));
                        }
                    }
                }
                for prop in &classe_def.propriedades {
                    if prop.eh_estatica {
                        if let Some(expr) = &prop.valor_inicial {
                            let mut temp_gen = self.spawn_child();
                            temp_gen.generate_expressao(expr);
                            self.bytecode_instructions
                                .extend(temp_gen.bytecode_instructions);
                            self.bytecode_instructions.push(format!(
                                "SET_STATIC_PROPERTY {} {}",
                                full_class_name, prop.nome
                            ));
                        }
                    }
                }
            }

            ast::Declaracao::DeclaracaoFuncao(func_def) => {
                // a) monta AST temporário com corpo
                let sub_programa = ast::Programa {
                    usings: vec![],
                    namespaces: vec![],
                    declaracoes: vec![ast::Declaracao::Comando(ast::Comando::Bloco(
                        func_def.corpo.clone(),
                    ))],
                };

                // b) gera corpo
                let mut sub = BytecodeGenerator {
                    programa: &sub_programa,
                    type_checker: self.type_checker,
                    namespace_path: self.namespace_path.clone(),
                    bytecode_instructions: Vec::new(),
                    props_por_classe: self.props_por_classe.clone(),
                    construtor_params_por_classe: self.construtor_params_por_classe.clone(),
                    current_class_name: None,
                    current_params: None,
                };
                let mut corpo = sub.generate(); // inclui HALT
                if !matches!(corpo.last(), Some(op) if op == "RETURN") {
                    corpo.push("LOAD_CONST_NULL".to_string());
                    corpo.push("RETURN".to_string());
                }

                // c) cabeçalho DEFINE_FUNCTION
                let params: Vec<String> =
                    func_def.parametros.iter().map(|p| p.nome.clone()).collect();
                // let full_fn = self.type_checker.resolver_nome_funcao(&func_def.nome, &self.namespace_path);
                let full_fn = self.qual(&func_def.nome);
                self.bytecode_instructions.push(format!(
                    "DEFINE_FUNCTION {} {} {}",
                    full_fn,
                    corpo.len(),
                    params.join(" ")
                ));

                self.bytecode_instructions.extend(corpo);
            }

            // Mantém o comportamento para comandos
            ast::Declaracao::Comando(cmd) => {
                self.generate_comando(cmd);
            }

            // Ignora outras declarações por enquanto
            _ => { /* Fazer nada ou adicionar tratamento para outros comandos */ }
        }
    }

    fn gerar_metodo(&mut self, metodo: &ast::MetodoClasse, nome_classe: &str) {
        let sub_prog = ast::Programa {
            usings: vec![],
            namespaces: vec![],
            declaracoes: vec![ast::Declaracao::Comando(ast::Comando::Bloco(
                metodo.corpo.clone(),
            ))],
        };

        let mut sub = BytecodeGenerator {
            programa: &sub_prog,
            type_checker: self.type_checker,
            namespace_path: self.namespace_path.clone(),
            bytecode_instructions: Vec::new(),
            props_por_classe: self.props_por_classe.clone(),
            construtor_params_por_classe: self.construtor_params_por_classe.clone(),
            current_class_name: Some(nome_classe.to_string()),
            current_params: Some(
                metodo
                    .parametros
                    .iter()
                    .map(|p| p.nome.clone())
                    .collect::<HashSet<String>>(),
            ),
        };
        let mut corpo = sub.generate();

        if !matches!(corpo.last(), Some(op) if op == "RETURN") {
            corpo.push("LOAD_CONST_NULL".to_string());
            corpo.push("RETURN".to_string());
        }

        let mut corpo_com_defaults = Vec::new();
        for p in &metodo.parametros {
            if let Some(default_expr) = &p.valor_padrao {
                let mut temp_gen = self.spawn_child();
                temp_gen.generate_expressao(default_expr);
                corpo_com_defaults.push(format!(
                    "SET_DEFAULT {} {}",
                    p.nome,
                    temp_gen.bytecode_instructions.join(" ")
                ));
            }
        }
        corpo_com_defaults.extend(corpo);
        let corpo = corpo_com_defaults;

        let params: Vec<String> = metodo
            .parametros
            .iter()
            .map(|p| {
                let mut param_str = p.nome.clone();
                if let Some(default_expr) = &p.valor_padrao {
                    param_str.push_str(&format!("={}", default_expr));
                }
                param_str
            })
            .collect();
        self.bytecode_instructions.push(format!(
            "DEFINE_METHOD {} {} {} {}",
            nome_classe,
            metodo.nome,
            corpo.len(),
            params.join(" ")
        ));
        self.bytecode_instructions.extend(corpo);
    }

    fn gerar_metodo_estatico(&mut self, metodo: &ast::MetodoClasse, nome_classe: &str) {
        let sub_prog = ast::Programa {
            usings: vec![],
            namespaces: vec![],
            declaracoes: vec![ast::Declaracao::Comando(ast::Comando::Bloco(
                metodo.corpo.clone(),
            ))],
        };

        let mut sub = BytecodeGenerator {
            programa: &sub_prog,
            type_checker: self.type_checker,
            namespace_path: self.namespace_path.clone(),
            bytecode_instructions: Vec::new(),
            props_por_classe: self.props_por_classe.clone(),
            construtor_params_por_classe: self.construtor_params_por_classe.clone(),
            current_class_name: Some(nome_classe.to_string()),
            current_params: Some(
                metodo
                    .parametros
                    .iter()
                    .map(|p| p.nome.clone())
                    .collect::<HashSet<String>>(),
            ),
        };
        let mut corpo = sub.generate();

        if !matches!(corpo.last(), Some(op) if op == "RETURN") {
            corpo.push("LOAD_CONST_NULL".to_string());
            corpo.push("RETURN".to_string());
        }

        let mut corpo_com_defaults = Vec::new();
        for p in &metodo.parametros {
            if let Some(default_expr) = &p.valor_padrao {
                let mut temp_gen = self.spawn_child();
                temp_gen.generate_expressao(default_expr);
                corpo_com_defaults.push(format!(
                    "SET_DEFAULT {} {}",
                    p.nome,
                    temp_gen.bytecode_instructions.join(" ")
                ));
            }
        }
        corpo_com_defaults.extend(corpo);
        let corpo = corpo_com_defaults;

        let params: Vec<String> = metodo
            .parametros
            .iter()
            .map(|p| {
                let mut param_str = p.nome.clone();
                if let Some(default_expr) = &p.valor_padrao {
                    param_str.push_str(&format!("={}", default_expr));
                }
                param_str
            })
            .collect();
        self.bytecode_instructions.push(format!(
            "DEFINE_STATIC_METHOD {} {} {} {}",
            nome_classe,
            metodo.nome,
            corpo.len(),
            params.join(" ")
        ));
        self.bytecode_instructions.extend(corpo);
    }

    pub fn generate(&mut self) -> Vec<String> {
        // Itera sobre as declarações no nível raiz do programa
        for declaracao in &self.programa.declaracoes {
            self.generate_declaracao(declaracao);
        }

        // Também processa namespaces de primeiro nível
        for namespace in &self.programa.namespaces {
            // Cria gerador dedicado com o caminho do namespace
            let mut sub = BytecodeGenerator {
                programa: &ast::Programa {
                    usings: vec![],
                    namespaces: vec![],
                    declaracoes: namespace.declaracoes.clone(),
                },
                type_checker: self.type_checker,
                namespace_path: namespace.nome.clone(),
                bytecode_instructions: Vec::new(),
                props_por_classe: self.props_por_classe.clone(),
                construtor_params_por_classe: self.construtor_params_por_classe.clone(),
                current_class_name: None,
                current_params: None,
            };
            self.bytecode_instructions.extend(sub.generate());
        }

        std::mem::take(&mut self.bytecode_instructions)
    }

    // Altera a assinatura para `&mut self` e remove o retorno Vec<String>
    fn generate_comando(&mut self, comando: &ast::Comando) {
        match comando {
            ast::Comando::DeclaracaoVar(nome, expr) => {
                self.generate_expressao(expr);
                self.bytecode_instructions
                    .push(format!("STORE_VAR {}", nome));
            }
            ast::Comando::DeclaracaoVariavel(_, nome, Some(expr)) => {
                self.generate_expressao(expr);
                self.bytecode_instructions
                    .push(format!("STORE_VAR {}", nome));
            }
            ast::Comando::Atribuicao(nome, expr) => {
                let mut is_prop = false;
                if let Some(class_name) = &self.current_class_name {
                    if let Some(class_info) = self.type_checker.classes.get(class_name) {
                        // Verifica as propriedades da classe atual e das classes pai
                        let mut current_class = Some(*class_info);
                        while let Some(class_decl) = current_class {
                            if class_decl.propriedades.iter().any(|p| p.nome == *nome)
                                || class_decl.campos.iter().any(|f| f.nome == *nome)
                            {
                                is_prop = true;
                                break;
                            }
                            current_class =
                                class_decl.classe_pai.as_ref().and_then(|parent_name| {
                                    self.type_checker.classes.get(parent_name).copied()
                                });
                        }
                    }
                }

                if is_prop {
                    self.bytecode_instructions
                        .push(format!("LOAD_VAR {}", "este")); // Empilha 'este'
                    self.generate_expressao(expr); // Empilha o valor
                    self.bytecode_instructions
                        .push(format!("SET_PROPERTY {}", nome));
                    self.bytecode_instructions.push("POP".to_string()); // Remove o objeto da pilha
                } else {
                    self.generate_expressao(expr);
                    self.bytecode_instructions
                        .push(format!("STORE_VAR {}", nome));
                }
            }
            ast::Comando::Imprima(expr) => {
                self.generate_expressao(expr);
                self.bytecode_instructions.push("PRINT".to_string());
            }
            ast::Comando::Bloco(comandos) => {
                // Adicionado: Bloco de comandos
                for cmd in comandos {
                    self.generate_comando(cmd);
                }
            }

            // Adicionado: Comando 'enquanto'
            ast::Comando::Enquanto(condicao, corpo) => {
                let loop_start_ip = self.bytecode_instructions.len(); // Ponto de início do loop

                self.generate_expressao(condicao); // Gera código para a condição
                let jump_if_false_placeholder_ip = self.bytecode_instructions.len();
                self.bytecode_instructions
                    .push("JUMP_IF_FALSE 0".to_string()); // Placeholder para o salto para o final do loop

                self.generate_comando(corpo); // Gera código para o corpo do loop

                self.bytecode_instructions
                    .push(format!("JUMP {}", loop_start_ip)); // Salta de volta para o início da condição

                let loop_end_ip = self.bytecode_instructions.len(); // Ponto final do loop
                                                                    // Patching: Atualiza a instrução JUMP_IF_FALSE com o endereço real
                self.bytecode_instructions[jump_if_false_placeholder_ip] =
                    format!("JUMP_IF_FALSE {}", loop_end_ip);
            }

            // Adicionado: Comando 'se'
            ast::Comando::Se(condicao, bloco_if, bloco_else) => {
                self.generate_expressao(condicao);
                let jump_if_false_placeholder = self.bytecode_instructions.len();
                self.bytecode_instructions
                    .push("JUMP_IF_FALSE 0".to_string());

                self.generate_comando(bloco_if);

                if let Some(else_bloco) = bloco_else {
                    let jump_to_end_placeholder = self.bytecode_instructions.len();
                    self.bytecode_instructions.push("JUMP 0".to_string());

                    let else_start_pos = self.bytecode_instructions.len();
                    self.bytecode_instructions[jump_if_false_placeholder] =
                        format!("JUMP_IF_FALSE {}", else_start_pos);

                    self.generate_comando(else_bloco);

                    let end_pos = self.bytecode_instructions.len();
                    self.bytecode_instructions[jump_to_end_placeholder] =
                        format!("JUMP {}", end_pos);
                } else {
                    let end_pos = self.bytecode_instructions.len();
                    self.bytecode_instructions[jump_if_false_placeholder] =
                        format!("JUMP_IF_FALSE {}", end_pos);
                }
            }

            ast::Comando::CriarObjeto(var_nome, classe, argumentos_chamada) => {
                let nome_completo = self
                    .type_checker
                    .resolver_nome_classe(classe, &self.namespace_path);

                // Bloquear instanciação de classes abstratas no bytecode
                if let Some(cl_decl) = self.get_class_declaration(&nome_completo) {
                    if cl_decl.eh_abstrata {
                        panic!(
                            "Não é possível instanciar classe abstrata: {}",
                            nome_completo
                        );
                    }
                }

                for arg in argumentos_chamada {
                    self.generate_expressao(arg);
                }

                self.bytecode_instructions.push(format!(
                    "NEW_OBJECT {} {}",
                    nome_completo,
                    argumentos_chamada.len()
                ));
                self.bytecode_instructions
                    .push(format!("STORE_VAR {}", var_nome));
            }

            ast::Comando::AtribuirPropriedade(objeto_expr, prop_nome, expr) => {
                self.generate_expressao(objeto_expr); // 1. Empilha o objeto
                self.generate_expressao(expr); // 2. Empilha o valor
                self.bytecode_instructions
                    .push(format!("SET_PROPERTY {}", prop_nome)); // 3. Executa a atribuição
                self.bytecode_instructions.push("POP".to_string()); // 4. Remove o objeto da pilha
            }
            ast::Comando::AtribuirIndice(alvo, idx, expr) => {
                // pilha: alvo, índice, valor
                self.generate_expressao(alvo);
                self.generate_expressao(idx);
                self.generate_expressao(expr);
                self.bytecode_instructions.push("SET_INDEX".to_string());
            }

            ast::Comando::ChamarMetodo(objeto_expr, metodo, argumentos) => {
                if let ast::Expressao::Identificador(ident) = &**objeto_expr {
                    let full_class_name = self
                        .type_checker
                        .resolver_nome_classe(ident, &self.namespace_path);
                    if self.type_checker.is_static_class(&full_class_name) {
                        // Static method call
                        for arg in argumentos {
                            self.generate_expressao(arg);
                        }
                        self.bytecode_instructions.push(format!(
                            "CALL_STATIC_METHOD {} {} {}",
                            full_class_name,
                            metodo,
                            argumentos.len()
                        ));
                        self.bytecode_instructions.push("POP".to_string());
                        return;
                    }
                }

                // Instance method call
                self.generate_expressao(objeto_expr);
                for arg in argumentos {
                    self.generate_expressao(arg);
                }
                let instrucao = format!("CALL_METHOD {} {}", metodo, argumentos.len());
                self.bytecode_instructions.push(instrucao);
                self.bytecode_instructions.push("POP".to_string());
            }

            ast::Comando::Retorne(expr_opt) => {
                if let Some(expr) = expr_opt {
                    self.generate_expressao(expr);
                } else {
                    self.bytecode_instructions
                        .push("LOAD_CONST_NULL".to_string());
                }
                self.bytecode_instructions.push("RETURN".to_string());
            }

            ast::Comando::Expressao(e) => {
                self.generate_expressao(e);
            }

            // Para outros comandos não implementados, remova a linha de comentário e implemente se necessário
            _ => { /* Fazer nada ou adicionar tratamento para outros comandos */ }
        }
    }

    fn generate_expressao(&mut self, expr: &ast::Expressao) {
        match expr {
            ast::Expressao::Texto(s) => self
                .bytecode_instructions
                .push(format!("LOAD_CONST_STR {}", s)),
            ast::Expressao::Inteiro(n) => self
                .bytecode_instructions
                .push(format!("LOAD_CONST_INT {}", n)),
            ast::Expressao::Booleano(b) => self
                .bytecode_instructions
                .push(format!("LOAD_CONST_BOOL {}", b)),
            // Suporte a literais flutuante e duplo
            ast::Expressao::FlutuanteLiteral(lit) => {
                let s = lit.trim_end_matches('f').trim_end_matches('F');
                self.bytecode_instructions
                    .push(format!("LOAD_CONST_FLOAT {}", s));
            }
            ast::Expressao::DuploLiteral(lit) => {
                self.bytecode_instructions
                    .push(format!("LOAD_CONST_DOUBLE {}", lit));
            }
            ast::Expressao::Decimal(lit) => {
                let s = lit.trim_end_matches('m');
                self.bytecode_instructions
                    .push(format!("LOAD_CONST_DECIMAL {}", s))
            }
            ast::Expressao::Identificador(nome) => {
                // Se o identificador é um parâmetro/variável local do método/construtor, priorizar variável
                let is_local = self
                    .current_params
                    .as_ref()
                    .map(|ps| ps.contains(nome))
                    .unwrap_or(false);
                if let Some(class_name) = &self.current_class_name {
                    if let Some(class_info) = self.type_checker.classes.get(class_name) {
                        let mut current_class = Some(*class_info);
                        while let Some(class_decl) = current_class {
                            if class_decl.propriedades.iter().any(|p| p.nome == *nome)
                                || class_decl.campos.iter().any(|f| f.nome == *nome)
                            {
                                // Somente acessar como propriedade se NÃO houver variável local com o mesmo nome
                                if !is_local {
                                    self.bytecode_instructions
                                        .push(format!("LOAD_VAR {}", "este"));
                                    self.bytecode_instructions
                                        .push(format!("GET_PROPERTY {}", nome));
                                    return;
                                } else {
                                    break; // há variável local; cair para LOAD_VAR nome
                                }
                            }
                            current_class =
                                class_decl.classe_pai.as_ref().and_then(|parent_name| {
                                    self.type_checker.classes.get(parent_name).copied()
                                });
                        }
                    }
                }
                self.bytecode_instructions
                    .push(format!("LOAD_VAR {}", nome));
            }

            ast::Expressao::Este => {
                // empilha o objeto atual do método
                self.bytecode_instructions
                    .push(format!("LOAD_VAR {}", "este"));
            }

            ast::Expressao::AcessoMembro(obj_expr, membro) => {
                if let ast::Expressao::Identificador(class_name) = &**obj_expr {
                    let full_class_name = self
                        .type_checker
                        .resolver_nome_classe(class_name, &self.namespace_path);
                    // if self.type_checker.is_static_class(&full_class_name) {
                    if self.type_checker.is_static_class(&full_class_name) {
                        // Acesso a membro estático
                        self.bytecode_instructions.push(format!(
                            "GET_STATIC_PROPERTY {} {}",
                            full_class_name, membro
                        ));
                        return;
                    }
                    // Enumeração: emite o índice do membro como inteiro
                    let fqn_enum = self
                        .type_checker
                        .resolver_nome_enum(class_name, &self.namespace_path);
                    if let Some(en) = self.type_checker.enums.get(&fqn_enum) {
                        if let Some(idx) = en.valores.iter().position(|v| v == membro) {
                            self.bytecode_instructions
                                .push(format!("LOAD_CONST_INT {}", idx));
                            return;
                        }
                    }
                }

                // Acesso a membro de instância
                self.generate_expressao(obj_expr);
                if membro == "tamanho" {
                    self.bytecode_instructions.push("GET_LENGTH".to_string());
                } else {
                    self.bytecode_instructions
                        .push(format!("GET_PROPERTY {}", membro));
                }
            }

            // Expressão para criar um novo objeto
            ast::Expressao::NovoObjeto(classe_nome, argumentos) => {
                let nome_completo = self
                    .type_checker
                    .resolver_nome_classe(classe_nome, &self.namespace_path);

                if let Some(class_decl) = self.get_class_declaration(&nome_completo) {
                    if class_decl.eh_abstrata {
                        panic!(
                            "Não é possível instanciar classe abstrata: {}",
                            nome_completo
                        );
                    }
                }

                let mut final_args_count = 0;
                if let Some(class_decl) = self.get_class_declaration(&nome_completo) {
                    if let Some(constructor) = class_decl.construtores.first() {
                        let mut arg_idx = 0;
                        for param in &constructor.parametros {
                            if let Some(arg_expr) = argumentos.get(arg_idx) {
                                self.generate_expressao(arg_expr);
                                arg_idx += 1;
                            } else if let Some(default_val_expr) = &param.valor_padrao {
                                self.generate_expressao(default_val_expr);
                            } else {
                                self.bytecode_instructions
                                    .push("LOAD_CONST_NULL".to_string());
                            }
                            final_args_count += 1;
                        }
                    } else {
                        for arg in argumentos {
                            self.generate_expressao(arg);
                            final_args_count += 1;
                        }
                    }
                } else {
                    for arg in argumentos {
                        self.generate_expressao(arg);
                        final_args_count += 1;
                    }
                }

                self.bytecode_instructions
                    .push(format!("NEW_OBJECT {} {}", nome_completo, final_args_count));
            }

            // Modificado: Operadores Aritméticos - Distinguir concatenação de soma numérica
            ast::Expressao::Aritmetica(op, esq, dir) => {
                self.generate_expressao(esq);
                self.generate_expressao(dir);
                match op {
                    ast::OperadorAritmetico::Soma => {
                        self.bytecode_instructions.push("ADD".to_string());
                    }
                    ast::OperadorAritmetico::Subtracao => {
                        self.bytecode_instructions.push("SUB".to_string())
                    }
                    ast::OperadorAritmetico::Multiplicacao => {
                        self.bytecode_instructions.push("MUL".to_string())
                    }
                    ast::OperadorAritmetico::Divisao => {
                        self.bytecode_instructions.push("DIV".to_string())
                    }
                    ast::OperadorAritmetico::Modulo => {
                        self.bytecode_instructions.push("MOD".to_string())
                    }
                }
            }

            ast::Expressao::ListaLiteral(itens) => {
                for e in itens {
                    self.generate_expressao(e);
                }
                self.bytecode_instructions
                    .push(format!("NEW_ARRAY {}", itens.len()));
            }

            ast::Expressao::AcessoIndice(obj, idx) => {
                self.generate_expressao(obj);
                self.generate_expressao(idx);
                self.bytecode_instructions.push("GET_INDEX".to_string());
            }

            // Adicionado: Operadores de Comparação
            ast::Expressao::Comparacao(op, esq, dir) => {
                self.generate_expressao(esq);
                self.generate_expressao(dir);
                match op {
                    ast::OperadorComparacao::Igual => {
                        self.bytecode_instructions.push("COMPARE_EQ".to_string())
                    }
                    ast::OperadorComparacao::Diferente => {
                        self.bytecode_instructions.push("COMPARE_NE".to_string())
                    }
                    ast::OperadorComparacao::Menor => {
                        self.bytecode_instructions.push("COMPARE_LT".to_string())
                    }
                    ast::OperadorComparacao::MaiorQue => {
                        self.bytecode_instructions.push("COMPARE_GT".to_string())
                    }
                    ast::OperadorComparacao::MenorIgual => {
                        self.bytecode_instructions.push("COMPARE_LE".to_string())
                    }
                    ast::OperadorComparacao::MaiorIgual => {
                        self.bytecode_instructions.push("COMPARE_GE".to_string())
                    }
                }
            }

            // Adicionado: Operadores Unários
            ast::Expressao::Unario(op, expr) => {
                self.generate_expressao(expr);
                match op {
                    ast::OperadorUnario::NegacaoLogica => {
                        self.bytecode_instructions.push("NEGATE_BOOL".to_string())
                    }
                    ast::OperadorUnario::NegacaoNumerica => {
                        self.bytecode_instructions.push("NEGATE_INT".to_string())
                    }
                }
            }

            ast::Expressao::StringInterpolada(partes) => {
                // Empilha cada pedaço (texto ou expressão)
                for parte in partes {
                    match parte {
                        ast::PartStringInterpolada::Texto(s) => {
                            self.bytecode_instructions
                                .push(format!("LOAD_CONST_STR {}", s));
                        }
                        ast::PartStringInterpolada::Expressao(e) => {
                            self.generate_expressao(e);
                        }
                    }
                }
                // Concatena tudo; resultado fica no topo da pilha
                self.bytecode_instructions
                    .push(format!("CONCAT {}", partes.len()));
            }

            ast::Expressao::Chamada(nome_funcao, argumentos) => {
                for arg in argumentos {
                    self.generate_expressao(arg);
                }
                let nome_completo = self
                    .type_checker
                    .resolver_nome_funcao(nome_funcao, &self.namespace_path);
                self.bytecode_instructions.push(format!(
                    "CALL_FUNCTION {} {}",
                    nome_completo,
                    argumentos.len()
                ));
            }

            ast::Expressao::ChamadaMetodo(objeto_expr, nome_metodo, argumentos) => {
                // Instance method call
                self.generate_expressao(objeto_expr);
                for arg in argumentos {
                    self.generate_expressao(arg);
                }
                let instrucao = format!("CALL_METHOD {} {}", nome_metodo, argumentos.len());
                self.bytecode_instructions.push(instrucao);
            }

            // Para outras expressões não implementadas, remova a linha de comentário e implemente se necessário
            _ => { /* Fazer nada ou adicionar tratamento para outras expressões */ }
        }
    }
}
