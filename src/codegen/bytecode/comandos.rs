use crate::ast;
use std::collections::{HashMap, HashSet};
use crate::library_loader::LibSimbolo;

use super::BytecodeGenerator;

impl<'a> BytecodeGenerator<'a> {
    pub(crate) fn generate_comando(&mut self, comando: &ast::Comando) {
        match comando {
            ast::Comando::DeclaracaoVar(nome, expr) => {
                self.generate_expressao(expr);
                self.bytecode_instructions
                    .push(format!("STORE_VAR {}", nome));
            }
            ast::Comando::DeclaracaoVariavel(_, nome, expr) => {
                match expr {
                    Some(e) => self.generate_expressao(e),
                    None => self
                        .bytecode_instructions
                        .push("LOAD_CONST_NULL".to_string()),
                }
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

            // Comando 'para' (for): init; condicao; passo
            ast::Comando::Para(init, condicao, passo, corpo) => {
                if let Some(init_cmd) = init {
                    self.generate_comando(init_cmd);
                    if matches!(**init_cmd, ast::Comando::Expressao(_)) {
                        self.bytecode_instructions.push("POP".to_string());
                    }
                }

                let loop_start_ip = self.bytecode_instructions.len();

                let mut jump_if_false_ip: Option<usize> = None;
                if let Some(cond) = condicao {
                    self.generate_expressao(cond);
                    jump_if_false_ip = Some(self.bytecode_instructions.len());
                    self.bytecode_instructions
                        .push("JUMP_IF_FALSE 0".to_string());
                }

                self.generate_comando(corpo);

                if let Some(passo_cmd) = passo {
                    self.generate_comando(passo_cmd);
                    if matches!(**passo_cmd, ast::Comando::Expressao(_)) {
                        self.bytecode_instructions.push("POP".to_string());
                    }
                }

                self.bytecode_instructions
                    .push(format!("JUMP {}", loop_start_ip));

                if let Some(jf_ip) = jump_if_false_ip {
                    let loop_end_ip = self.bytecode_instructions.len();
                    self.bytecode_instructions[jf_ip] =
                        format!("JUMP_IF_FALSE {}", loop_end_ip);
                }
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

            ast::Comando::AtribuirPropriedade(objeto_expr, prop_nome, expr)
            | ast::Comando::AtribuirCampo(objeto_expr, prop_nome, expr) => {
                self.generate_expressao(objeto_expr); // 1. Empilha o objeto
                self.generate_expressao(expr); // 2. Empilha o valor
                self.bytecode_instructions
                    .push(format!("SET_PROPERTY {}", prop_nome)); // 3. Executa a atribuição
                self.bytecode_instructions.push("POP".to_string()); // 4. Remove o objeto da pilha
            }
            ast::Comando::AtribuirIndice(alvo, idx, expr) => {
                if let ast::Expressao::Identificador(nome) = &**alvo {
                    // Arrays são valores imutáveis na VM: é preciso recarregar a
                    // variável com o array modificado retornado por SET_INDEX.
                    self.bytecode_instructions
                        .push(format!("LOAD_VAR {}", nome));
                    self.generate_expressao(idx);
                    self.generate_expressao(expr);
                    self.bytecode_instructions.push("SET_INDEX".to_string());
                    self.bytecode_instructions
                        .push(format!("STORE_VAR {}", nome));
                } else {
                    // pilha: alvo, índice, valor
                    self.generate_expressao(alvo);
                    self.generate_expressao(idx);
                    self.generate_expressao(expr);
                    self.bytecode_instructions.push("SET_INDEX".to_string());
                }
            }

            ast::Comando::ChamarMetodo(objeto_expr, metodo, argumentos) => {
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
                // Resolver o nome completo da classe independentemente de is_static_call
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
                            if let Some(lib_metodo) = lib_classe.metodos.get(metodo) {
                                if let Some(chave_nativa) = lib_metodo
                                    .chave_nativa
                                    .as_deref()
                                    .filter(|c| ast::chave_nativa_valida(c))
                                {
                                    // É uma chamada nativa da biblioteca externa
                                    // O modificador estático é do método, não da classe
                                    let eh_estatica = lib_metodo.eh_estatica;
                                    let sufixo_async = if lib_metodo.eh_assincrona { "_ASYNC" } else { "" };
                                    if eh_estatica {
                                        for arg in argumentos {
                                            self.generate_expressao(arg);
                                        }
                                        self.bytecode_instructions.push(format!(
                                            "CALL_STATIC_NATIVE{} {} {}",
                                            sufixo_async,
                                            chave_nativa,
                                            argumentos.len()
                                        ));
                                    } else {
                                        self.generate_expressao(objeto_expr);
                                        for arg in argumentos {
                                            self.generate_expressao(arg);
                                        }
                                        self.bytecode_instructions.push(format!(
                                            "CALL_NATIVE{} {} {}",
                                            sufixo_async,
                                            chave_nativa,
                                            argumentos.len()
                                        ));
                                    }
                                    self.bytecode_instructions.push("POP".to_string());
                                    return;
                                }
                            }
                        }
                    }
                }

                if let Some(class_fqn) = class_fqn_opt {
                    if let Some(class_info) = self.type_checker.resolved_classes.get(&class_fqn) {
                        if let Some(metodo_info) = class_info.methods.get(metodo) {
                            if let Some(nativo_attr) =
                                metodo_info.attributes.iter().find(|a| a.name == "Nativo")
                            {
                                let chave_nativa = nativo_attr
                                    .arguments
                                    .get(0)
                                    .and_then(|e| match e {
                                        ast::Expressao::Texto(s)
                                            if ast::chave_nativa_valida(s) =>
                                        {
                                            Some(s.as_str())
                                        }
                                        _ => None,
                                    });
                                if let Some(chave_nativa) = chave_nativa {
                                    // É uma chamada nativa
                                    let sufixo_async = if metodo_info.eh_assincrona { "_ASYNC" } else { "" };
                                    if is_static_call || metodo_info.eh_estatica {
                                        for arg in argumentos {
                                            self.generate_expressao(arg);
                                        }
                                        self.bytecode_instructions.push(format!(
                                            "CALL_STATIC_NATIVE{} {} {}",
                                            sufixo_async,
                                            chave_nativa,
                                            argumentos.len()
                                        ));
                                    } else {
                                        self.generate_expressao(objeto_expr);
                                        for arg in argumentos {
                                            self.generate_expressao(arg);
                                        }
                                        self.bytecode_instructions.push(format!(
                                            "CALL_NATIVE{} {} {}",
                                            sufixo_async,
                                            chave_nativa,
                                            argumentos.len()
                                        ));
                                    }
                                    self.bytecode_instructions.push("POP".to_string());
                                    return;
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
                            metodo,
                            argumentos.len()
                        ));
                        self.bytecode_instructions.push("POP".to_string());
                        return;
                    }
                }

                // Chamada de método de instância
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

            ast::Comando::AcessarCampo(obj, campo) => {
                self.bytecode_instructions
                    .push(format!("LOAD_VAR {}", obj));
                self.bytecode_instructions
                    .push(format!("GET_PROPERTY {}", campo));
                self.bytecode_instructions.push("POP".to_string());
            }

            ast::Comando::ParaCada { .. } => {
                // Normalmente o lowering já reescreveu este comando; este
                // braço cobre geradores instanciados diretamente.
                let mut lowered = comando.clone();
                crate::codegen::normalizar_comando_para_cada_unico(&mut lowered);
                self.generate_comando(&lowered);
            }

            ast::Comando::Tente { .. } | ast::Comando::Falhe(_) => {
                panic!("Exceções (tente/capture/finalmente/falhe) ainda não são suportadas pelo backend bytecode")
            }

            // Para outros comandos não implementados, remova a linha de comentário e implemente se necessário
            _ => { /* Fazer nada ou adicionar tratamento para outros comandos */ }
        }
    }

}
