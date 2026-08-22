use crate::ast;
use std::fmt;
use super::BytecodeGenerator;
use crate::library_loader::LibSimbolo;

impl fmt::Display for ast::Expressao {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ast::Expressao::Identificador(s) => write!(f, "{}", s),
            ast::Expressao::Este => write!(f, "este"),
            _ => write!(f, "<expressao>"),
        }
    }
}

pub fn get_expr_name(expr: &ast::Expressao) -> Option<String> {
    match expr {
        ast::Expressao::Identificador(s) => Some(s.clone()),
        ast::Expressao::Este => Some("este".to_string()),
        _ => None,
    }
}

impl<'a> BytecodeGenerator<'a> {
    pub(crate) fn is_string_expr(expr: &ast::Expressao) -> bool {
        use ast::{Expressao as E, OperadorAritmetico as OA};
        match expr {
            E::Texto(_) | E::StringInterpolada(_) => true,
            E::Aritmetica(OA::Soma, l, r) => Self::is_string_expr(l) || Self::is_string_expr(r),
            _ => false,
        }
    }

    pub(crate) fn generate_expressao(&mut self, expr: &ast::Expressao) {
        match expr {
            ast::Expressao::Texto(s) => {
                // Emite com aspas para preservar espaços
                let escaped = s.replace('"', "\\\"");
                self.bytecode_instructions
                    .push(format!("LOAD_CONST_STR \"{}\"", escaped));
            }
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
                                class_decl.classe_pai.as_ref().and_then(|parent_tipo| {
                                    let base = match parent_tipo {
                                        ast::Tipo::Classe(n) => n.as_str(),
                                        ast::Tipo::Aplicado { nome, .. } => nome.as_str(),
                                        _ => return None,
                                    };
                                    let fqn = self
                                        .type_checker
                                        .resolver_nome_classe(base, &self.namespace_path);
                                    self.type_checker.classes.get(&fqn).copied()
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
            ast::Expressao::NovoObjeto(tipo, argumentos) => {
                let classe_nome = match tipo {
                    ast::Tipo::Classe(n) => n.clone(),
                    ast::Tipo::Aplicado { nome, .. } => nome.clone(),
                    _ => panic!("Instanciação de tipo não suportado em bytecode: {:?}", tipo),
                };
                let nome_completo = self
                    .type_checker
                    .resolver_nome_classe(&classe_nome, &self.namespace_path);

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
            ast::Expressao::NovoArray(tipo, tamanho) => {
                self.generate_expressao(tamanho);
                self.bytecode_instructions
                    .push(format!("NEW_ARRAY_OF_TYPE {:?}", tipo));
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
                            let escaped = s.replace('"', "\\\"");
                            self.bytecode_instructions
                                .push(format!("LOAD_CONST_STR \"{}\"", escaped));
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

                if nome_completo == "LerArquivoAssíncrono"
                    || nome_completo == "EscreverArquivoAssíncrono"
                {
                    self.bytecode_instructions.push(format!(
                        "CALL_STATIC_NATIVE_ASYNC {} {}",
                        nome_completo,
                        argumentos.len()
                    ));
                } else {
                    self.bytecode_instructions.push(format!(
                        "CALL_FUNCTION {} {}",
                        nome_completo,
                        argumentos.len()
                    ));
                }
            }

            ast::Expressao::Aguarde(expr) => {
                self.generate_expressao(expr);
                self.bytecode_instructions.push("AWAIT".to_string());
            }

            ast::Expressao::ChamadaMetodo(objeto_expr, nome_metodo, argumentos) => {
                let mut is_static_call = false;
                let mut class_fqn_opt = None;

                if let ast::Expressao::Identificador(nome_classe) = &**objeto_expr {
                    let full_class_name = self
                        .type_checker
                        .resolver_nome_classe(nome_classe, &self.namespace_path);
                    if let Some(classe_info) =
                        self.type_checker.resolved_classes.get(&full_class_name)
                    {
                        if classe_info.eh_estatica {
                            is_static_call = true;
                            class_fqn_opt = Some(full_class_name);
                        }
                    } else {
                        // NOVO: Verificar se a classe está na biblioteca externa
                        if let Some(bib) = &self.type_checker.biblioteca_externa {
                            if let Some(LibSimbolo::Classe(lib_classe)) =
                                bib.simbolos.get(&full_class_name)
                            {
                                if lib_classe.eh_estatica {
                                    is_static_call = true;
                                    class_fqn_opt = Some(full_class_name);
                                }
                            }
                        }
                    }
                }

                if !is_static_call {
                    // Inferir tipo sem modificar o type_checker usando apenas as informações resolvidas
                    if let ast::Expressao::Identificador(nome_var) = &**objeto_expr {
                        // Tentar resolver como nome de classe
                        let full_class_name = self
                            .type_checker
                            .resolver_nome_classe(nome_var, &self.namespace_path);
                        if self
                            .type_checker
                            .resolved_classes
                            .contains_key(&full_class_name)
                        {
                            class_fqn_opt = Some(full_class_name);
                        } else {
                            // NOVO: Verificar se a classe está na biblioteca externa
                            if let Some(bib) = &self.type_checker.biblioteca_externa {
                                if bib.simbolos.contains_key(&full_class_name) {
                                    class_fqn_opt = Some(full_class_name);
                                }
                            }
                        }
                    }
                }

                // NOVO: Consultar biblioteca externa para métodos nativos
                let class_fqn = if let Some(fqn) = &class_fqn_opt {
                    fqn.clone()
                } else if let ast::Expressao::Identificador(nome_classe) = &**objeto_expr {
                    self.type_checker
                        .resolver_nome_classe(nome_classe, &self.namespace_path)
                } else {
                    String::new()
                };

                if !class_fqn.is_empty() {
                    if let Some(bib) = &self.type_checker.biblioteca_externa {
                        if let Some(LibSimbolo::Classe(lib_classe)) = bib.simbolos.get(&class_fqn) {
                            if let Some(lib_metodo) = lib_classe.metodos.get(nome_metodo) {
                                if let Some(chave_nativa) = &lib_metodo.chave_nativa {
                                    // É uma chamada nativa da biblioteca externa
                                    let eh_estatica = lib_classe.eh_estatica;
                                    if eh_estatica {
                                        for arg in argumentos {
                                            self.generate_expressao(arg);
                                        }
                                        self.bytecode_instructions.push(format!(
                                            "CALL_STATIC_NATIVE {} {}",
                                            chave_nativa,
                                            argumentos.len()
                                        ));
                                    } else {
                                        self.generate_expressao(objeto_expr);
                                        for arg in argumentos {
                                            self.generate_expressao(arg);
                                        }
                                        self.bytecode_instructions.push(format!(
                                            "CALL_NATIVE {} {}",
                                            chave_nativa,
                                            argumentos.len()
                                        ));
                                    }
                                    return;
                                }
                            }
                        }
                    }
                }

                if let Some(class_fqn) = class_fqn_opt {
                    if let Some(class_info) = self.type_checker.resolved_classes.get(&class_fqn) {
                        if let Some(metodo_info) = class_info.methods.get(nome_metodo) {
                            if let Some(nativo_attr) =
                                metodo_info.attributes.iter().find(|a| a.name == "Nativo")
                            {
                                if let Some(ast::Expressao::Texto(chave_nativa)) =
                                    nativo_attr.arguments.get(0)
                                {
                                    // É uma chamada nativa
                                    if is_static_call {
                                        for arg in argumentos {
                                            self.generate_expressao(arg);
                                        }
                                        self.bytecode_instructions.push(format!(
                                            "CALL_STATIC_NATIVE {} {}",
                                            chave_nativa,
                                            argumentos.len()
                                        ));
                                    } else {
                                        self.generate_expressao(objeto_expr); // Empilha 'este'
                                        for arg in argumentos {
                                            self.generate_expressao(arg);
                                        }
                                        self.bytecode_instructions.push(format!(
                                            "CALL_NATIVE {} {}",
                                            chave_nativa,
                                            argumentos.len()
                                        ));
                                    }
                                    return; // Fim do tratamento para chamada nativa
                                }
                            }
                        }
                    }
                }

                // Lógica original para chamadas não-nativas
                if is_static_call {
                    if let ast::Expressao::Identificador(nome_classe) = &**objeto_expr {
                        let full_class_name = self
                            .type_checker
                            .resolver_nome_classe(nome_classe, &self.namespace_path);
                        for arg in argumentos {
                            self.generate_expressao(arg);
                        }
                        self.bytecode_instructions.push(format!(
                            "CALL_STATIC_METHOD {} {} {}",
                            full_class_name,
                            nome_metodo,
                            argumentos.len()
                        ));
                        return;
                    }
                }

                // Chamada de método de instância
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
