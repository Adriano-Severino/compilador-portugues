use crate::ast;
use std::collections::{HashMap, HashSet};
use crate::library_loader::LibSimbolo;
use crate::codegen::bytecode::expressoes::get_expr_name;

use super::BytecodeGenerator;

impl<'a> BytecodeGenerator<'a> {
    pub(crate) fn gerar_construtor(&mut self, ctor: &ast::ConstrutorClasse, nome_classe: &str) {
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
            "DEFINE_METHOD {} {} {} {} {}",
            nome_classe,
            "construtor",
            "vazio",
            corpo.len(),
            params.join(" ")
        ));
        self.bytecode_instructions.extend(corpo);
    }

    pub(crate) fn generate_declaracao(&mut self, declaracao: &ast::Declaracao) {
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
                            let base = match p {
                                ast::Tipo::Classe(n) => n.as_str(),
                                ast::Tipo::Aplicado { nome, .. } => nome.as_str(),
                                _ => "",
                            };
                            self.type_checker
                                .resolver_nome_classe(base, &self.namespace_path)
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

                // For static classes, we still need to register them but with a special marker
                if classe_def.eh_estatica {
                    self.bytecode_instructions
                        .push(format!("DEFINE_STATIC_CLASS {}", full_class_name));
                } else {
                    self.bytecode_instructions.push(format!(
                        "DEFINE_CLASS {} {} {}",
                        full_class_name, parent_class_name, meta_str
                    ));
                }

                for ctor in &classe_def.construtores {
                    self.gerar_construtor(ctor, &full_class_name);
                }

                for metodo in &classe_def.metodos {
                    if metodo.eh_abstrato {
                        continue; // não gera corpo nem entrada para métodos abstratos
                    }
                    if metodo.eh_estatica {
                        self.gerar_metodo_estatico(metodo, &full_class_name);
                    } else {
                        self.gerar_metodo(metodo, &full_class_name);
                    }
                }

                // Marca o fim da declaração da classe
                if !classe_def.eh_estatica {
                    self.bytecode_instructions.push("END_CLASS".to_string());
                }

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

            // Interfaces não geram bytecode diretamente; usadas apenas pelo verificador de tipos
            ast::Declaracao::DeclaracaoInterface(_iface) => {}

            // Ignora outras declarações por enquanto
            _ => { /* Fazer nada ou adicionar tratamento para outros comandos */ }
        }
    }

    pub(crate) fn gerar_metodo(&mut self, metodo: &ast::MetodoClasse, nome_classe: &str) {
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

        let tipo_retorno_str = metodo
            .tipo_retorno
            .as_ref()
            .map_or("vazio".to_string(), |t| t.to_string());

        let params: Vec<String> = metodo
            .parametros
            .iter()
            .map(|p| format!("{}:{}", p.tipo.to_string(), p.nome))
            .collect();
        self.bytecode_instructions.push(format!(
            "DEFINE_METHOD {} {} {} {} {}",
            nome_classe,
            metodo.nome,
            tipo_retorno_str,
            corpo.len(),
            params.join(" ")
        ));
        self.bytecode_instructions.extend(corpo);
    }

    pub(crate) fn gerar_metodo_estatico(&mut self, metodo: &ast::MetodoClasse, nome_classe: &str) {
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

        let tipo_retorno_str = metodo
            .tipo_retorno
            .as_ref()
            .map_or("vazio".to_string(), |t| t.to_string());

        let params: Vec<String> = metodo
            .parametros
            .iter()
            .map(|p| format!("{}:{}", p.tipo.to_string(), p.nome))
            .collect();

        self.bytecode_instructions.push(format!(
            "DEFINE_STATIC_METHOD {} {} {} {} {}",
            nome_classe,
            metodo.nome,
            tipo_retorno_str,
            corpo.len(),
            params.join(" ")
        ));
        self.bytecode_instructions.extend(corpo);
    }

}
