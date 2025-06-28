use crate::ast;
use std::collections::HashMap;

/// O gerador de código para o alvo Bytecode.
pub struct BytecodeGenerator<'a> {
    programa: &'a ast::Programa,
    type_checker: &'a crate::type_checker::VerificadorTipos<'a>,
    namespace_path: String,
    bytecode_instructions: Vec<String>,
    em_metodo: bool,
    props_por_classe: HashMap<String, Vec<String>>,
}

impl<'a> BytecodeGenerator<'a> {
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
        em_metodo: bool,
    ) -> Self {
        Self {
            programa,
            type_checker,
            namespace_path: String::new(),
            bytecode_instructions: Vec::new(),
            em_metodo,
            props_por_classe: HashMap::new(),
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
                    em_metodo: false,
                    props_por_classe: self.props_por_classe.clone(),
                };
                self.bytecode_instructions.extend(sub.generate());
            }

            // ✅ Reconhece e processa a declaração de classe
            ast::Declaracao::DeclaracaoClasse(classe_def) => {
                // ------------- 1. coleta as propriedades (campos + props) -------------
                let mut propriedades: Vec<String> = classe_def
                    .propriedades
                    .iter()
                    .map(|p| p.nome.clone())
                    .chain(classe_def.campos.iter().map(|c| c.nome.clone()))
                    .collect();

                if let Some(pai) = &classe_def.classe_pai {
                    if let Some(props_pai) = self.props_por_classe.get(pai) {
                        propriedades = props_pai
                            .clone()
                            .into_iter()
                            .chain(propriedades.into_iter())
                            .collect();
                    }
                }

                let props_str = propriedades.join(" ");

                self.props_por_classe
                    .insert(classe_def.nome.clone(), propriedades.clone());
                // ------------- 2. DEFINE_CLASS vem PRIMEIRO ---------------------------
                let full_class = self.qual(&classe_def.nome);
                self.bytecode_instructions
                    .push(format!("DEFINE_CLASS {} {}", full_class, props_str));

                // ------------- 3. gera cada método como bloco independente ------------
                for metodo in &classe_def.metodos {
                    // a) AST temporário que vive até o fim do loop
                    let sub_programa = ast::Programa {
                        usings: vec![],
                        namespaces: vec![],
                        declaracoes: vec![ast::Declaracao::Comando(ast::Comando::Bloco(
                            metodo.corpo.clone(),
                        ))],
                    };

                    // b) gera bytecode do corpo do método
                                        let mut sub = BytecodeGenerator {
                        programa: &sub_programa,
                        type_checker: self.type_checker,
                        namespace_path: self.namespace_path.clone(),
                        bytecode_instructions: Vec::new(),
                        em_metodo: true,
                        props_por_classe: self.props_por_classe.clone(),
                    };
                    let mut corpo = sub.generate(); // inclui HALT

                    if !matches!(corpo.last(), Some(last) if last == "RETURN") {
                        corpo.push("LOAD_CONST_NULL".to_string());
                        corpo.push("RETURN".to_string());
                    }

                    // c) cabeçalho + corpo
                                        let full_class_name = self.qual(&classe_def.nome);
                    let params: Vec<String> = metodo.parametros.iter().map(|p| p.nome.clone()).collect();
                    let instruction = if metodo.eh_estatica {
                        "DEFINE_STATIC_METHOD"
                    } else {
                        "DEFINE_METHOD"
                    };
                    self.bytecode_instructions.push(format!(
                        "{} {} {} {} {}",
                        instruction,
                        full_class_name,
                        metodo.nome,
                        corpo.len(),
                        params.join(" ")
                    ));
                    self.bytecode_instructions.extend(corpo);
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
                    em_metodo: false,
                    props_por_classe: self.props_por_classe.clone(),
                };
                let mut corpo = sub.generate(); // inclui HALT
                if !matches!(corpo.last(), Some(op) if op == "RETURN") {
                    corpo.push("LOAD_CONST_NULL".to_string());
                    corpo.push("RETURN".to_string());
                }

                // c) cabeçalho DEFINE_FUNCTION
                let params: Vec<String> =
                    func_def.parametros.iter().map(|p| p.nome.clone()).collect();
                let full_fn = self.type_checker.resolver_nome_funcao(&func_def.nome, &self.namespace_path);
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
                em_metodo: false,
                props_por_classe: self.props_por_classe.clone(),
            };
            self.bytecode_instructions.extend(sub.generate());
        }

        std::mem::take(&mut self.bytecode_instructions)
    }

    // Altera a assinatura para `&mut self` e remove o retorno Vec<String>
    fn generate_comando(&mut self, comando: &ast::Comando) {
        match comando {
            ast::Comando::DeclaracaoVar(nome, expr) => {
                self.generate_expressao(expr); // Gera expressão e adiciona à lista interna
                self.bytecode_instructions
                    .push(format!("STORE_VAR {}", nome));
            }
            ast::Comando::DeclaracaoVariavel(_, nome, Some(expr)) => {
                self.generate_expressao(expr);
                self.bytecode_instructions
                    .push(format!("STORE_VAR {}", nome));
            }
            ast::Comando::Atribuicao(nome, expr) => {
                // Adicionado: Atribuição
                self.generate_expressao(expr);
                self.bytecode_instructions
                    .push(format!("STORE_VAR {}", nome));
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
                self.generate_expressao(condicao); // Gera código para a condição

                let jump_if_false_placeholder_ip = self.bytecode_instructions.len();
                self.bytecode_instructions
                    .push("JUMP_IF_FALSE 0".to_string()); // Placeholder para o salto

                self.generate_comando(bloco_if); // Gera código para o bloco 'se'

                if let Some(bloco_else) = bloco_else {
                    let jump_to_end_placeholder_ip = self.bytecode_instructions.len();
                    self.bytecode_instructions.push("JUMP 0".to_string()); // Salta sobre o bloco 'senão'

                    let else_start_ip = self.bytecode_instructions.len();
                    // Patching: Se houver 'senão', o JUMP_IF_FALSE salta para o início do bloco 'senão'
                    self.bytecode_instructions[jump_if_false_placeholder_ip] =
                        format!("JUMP_IF_FALSE {}", else_start_ip);

                    self.generate_comando(bloco_else); // Gera código para o bloco 'senão'

                    let end_if_else_ip = self.bytecode_instructions.len();
                    // Patching: O JUMP sobre o bloco 'senão' salta para o final de tudo
                    self.bytecode_instructions[jump_to_end_placeholder_ip] =
                        format!("JUMP {}", end_if_else_ip);
                } else {
                    let end_if_ip = self.bytecode_instructions.len();
                    // Patching: Se não houver 'senão', o JUMP_IF_FALSE salta para o final do comando 'se'
                    self.bytecode_instructions[jump_if_false_placeholder_ip] =
                        format!("JUMP_IF_FALSE {}", end_if_ip);
                }
            }

            ast::Comando::CriarObjeto(var_nome, classe, argumentos) => {
                // Gerar argumentos
                for arg in argumentos {
                    self.generate_expressao(arg);
                }

                // Criar objeto
                                let nome_completo = self.type_checker.resolver_nome_classe(classe, &self.namespace_path);
                self.bytecode_instructions.push(format!(
                    "NEW_OBJECT {} {}",
                    nome_completo,
                    argumentos.len()
                ));
                self.bytecode_instructions
                    .push(format!("STORE_VAR {}", var_nome));
            }

            ast::Comando::AtribuirPropriedade(objeto_nome, prop_nome, expr) => {
                // 1. Carrega a instância do objeto na pilha.
                self.bytecode_instructions
                    .push(format!("LOAD_VAR {}", objeto_nome));
                // 2. Gera o valor a ser atribuído e o coloca na pilha.
                self.generate_expressao(expr);
                // 3. Emite a nova instrução para definir a propriedade.
                self.bytecode_instructions
                    .push(format!("SET_PROPERTY {}", prop_nome));
            }

            ast::Comando::ChamarMetodo(objeto_nome, metodo, argumentos) => {
                self.bytecode_instructions
                    .push(format!("LOAD_VAR {}", objeto_nome));

                for arg in argumentos {
                    self.generate_expressao(arg);
                }

                self.bytecode_instructions.push(format!(
                    "CALL_METHOD {} {}",
                    metodo,
                    argumentos.len()
                ));
            }

            ast::Comando::Retorne(expr_opt) => {
                // (1) Se houver expressão, gera o bytecode que coloca o valor na pilha
                if let Some(expr) = expr_opt {
                    self.generate_expressao(expr);
                } else {
                    // empilha Nulo para métodos void
                    self.bytecode_instructions
                        .push("LOAD_CONST_NULL".to_string());
                }
                // (2) encerra o frame
                self.bytecode_instructions.push("RETURN".to_string());
            }

            ast::Comando::Expressao(e) => {
                self.generate_expressao(e);
                self.bytecode_instructions.push("POP".into());
            }

            // Para outros comandos não implementados, remova a linha de comentário e implemente se necessário
            _ => { /* Fazer nada ou adicionar tratamento para outros comandos */ }
        }
    }

    fn generate_expressao(&mut self, expr: &ast::Expressao) {
        match expr {
            ast::Expressao::Texto(s) => self
                .bytecode_instructions
                .push(format!("LOAD_CONST_STR \"{}\"", s)),
            ast::Expressao::Inteiro(n) => self
                .bytecode_instructions
                .push(format!("LOAD_CONST_INT {}", n)),
            ast::Expressao::Booleano(b) => self
                .bytecode_instructions
                .push(format!("LOAD_CONST_BOOL {}", b)),
            ast::Expressao::Identificador(nome) => {
                self.bytecode_instructions
                    .push(format!("LOAD_VAR {}", nome));
            }

            ast::Expressao::Este => {
                // empilha o objeto atual do método
                self.bytecode_instructions.push("LOAD_VAR este".to_string());
            }

            ast::Expressao::AcessoMembro(obj_expr, membro) => {
                // 1. gera o objeto (pode ser 'este' ou outro)
                self.generate_expressao(obj_expr);
                // 2. lê a propriedade
                self.bytecode_instructions
                    .push(format!("GET_PROPERTY {}", membro));
            }

            // Expressão para criar um novo objeto
            ast::Expressao::NovoObjeto(classe_nome, argumentos) => {
                // Primeiro, gera o bytecode para cada argumento, colocando-os na pilha
                for arg in argumentos {
                    self.generate_expressao(arg);
                }
                // Em seguida, emite a instrução para criar um novo objeto
                // ✅ NOVO: Resolve o nome completo da classe usando o verificador de tipos
                                let nome_completo = self.type_checker.resolver_nome_classe(classe_nome, &self.namespace_path);
                self.bytecode_instructions.push(format!(
                    "NEW_OBJECT {} {}",
                    nome_completo,
                    argumentos.len()
                ));
            }

            // Modificado: Operadores Aritméticos - Distinguir concatenação de soma numérica
            ast::Expressao::Aritmetica(op, esq, dir) => {
                self.generate_expressao(esq);
                self.generate_expressao(dir);
                match op {
                    ast::OperadorAritmetico::Soma => {
                        // Idealmente, haveria verificação de tipo aqui, ou um operador polimórfico.

                        if Self::is_string_expr(esq) || Self::is_string_expr(dir) {
                            self.bytecode_instructions.push("CONCAT 2".to_string());
                        } else {
                            self.bytecode_instructions.push("ADD".to_string());
                        }
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
                                .push(format!("LOAD_CONST_STR \"{}\"", s));
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
                // ✅ CORRIGIDO: Resolve o nome completo da função usando o type_checker
                let nome_completo = self.type_checker.resolver_nome_funcao(nome_funcao, &self.namespace_path);
                self.bytecode_instructions.push(format!(
                    "CALL_FUNCTION {} {}",
                    nome_completo,
                    argumentos.len()
                ));
            }

            ast::Expressao::ChamadaMetodo(objeto_expr, nome_metodo, argumentos) => {
                if let ast::Expressao::Identificador(class_name) = &**objeto_expr {
                    if self.type_checker.is_class(class_name) {
                        // Static method call
                        for arg in argumentos {
                            self.generate_expressao(arg);
                        }
                        let full_class_name = self.type_checker.resolver_nome_classe(class_name, &self.namespace_path);
                        self.bytecode_instructions.push(format!(
                            "CALL_STATIC_METHOD {} {} {}",
                            full_class_name,
                            nome_metodo,
                            argumentos.len()
                        ));
                        return;
                    }
                }

                // Instance method call
                self.generate_expressao(objeto_expr);
                for arg in argumentos {
                    self.generate_expressao(arg);
                }
                self.bytecode_instructions.push(format!(
                    "CALL_METHOD {} {}",
                    nome_metodo,
                    argumentos.len()
                ));
            }

            // Para outras expressões não implementadas, remova a linha de comentário e implemente se necessário
            _ => { /* Fazer nada ou adicionar tratamento para outras expressões */ }
        }
    }
}
