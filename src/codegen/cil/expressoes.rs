// src/codegen/cil/expressoes.rs

use crate::ast;
use super::tipos::{sanitizar_identificador, tipo_para_cil};
use super::CilGenerator;

impl<'a> CilGenerator<'a> {
    /// Gera instruções CIL para avaliar uma expressão e deixá-la no topo da pilha.
    /// Retorna o `ast::Tipo` resultante da expressão avaliada.
    pub fn generate_expressao(
        &mut self,
        expr: &ast::Expressao,
        code: &mut String,
    ) -> ast::Tipo {
        match expr {
            ast::Expressao::Inteiro(n) => {
                code.push_str(&format!("    ldc.i8 {}\n", n));
                ast::Tipo::Inteiro
            }
            ast::Expressao::Texto(s) => {
                code.push_str(&format!("    ldstr \"{}\"\n", self.escapar_string(s)));
                ast::Tipo::Texto
            }
            ast::Expressao::Booleano(b) => {
                if *b {
                    code.push_str("    ldc.i4.1\n");
                } else {
                    code.push_str("    ldc.i4.0\n");
                }
                ast::Tipo::Booleano
            }
            ast::Expressao::FlutuanteLiteral(f) => {
                let limpo = f.trim_end_matches('f').trim_end_matches('F');
                code.push_str(&format!("    ldc.r4 {}\n", limpo));
                ast::Tipo::Flutuante
            }
            ast::Expressao::DuploLiteral(d) => {
                code.push_str(&format!("    ldc.r8 {}\n", d));
                ast::Tipo::Duplo
            }
            ast::Expressao::Decimal(d) => {
                let limpo = d.trim_end_matches('m').trim_end_matches('M');
                code.push_str(&format!("    ldstr \"{}\"\n", limpo));
                code.push_str("    call valuetype [System.Runtime]System.Decimal [System.Runtime]System.Decimal::Parse(string)\n");
                ast::Tipo::Decimal
            }
            ast::Expressao::Nulo => {
                code.push_str("    ldnull\n");
                ast::Tipo::Objeto
            }
            ast::Expressao::Este => {
                code.push_str("    ldarg.0\n");
                let classe_nome = self.classe_atual.clone().unwrap_or_else(|| "Principal".to_string());
                ast::Tipo::Classe(classe_nome)
            }
            ast::Expressao::Identificador(nome) => {
                // 1. Variável local
                if let Some((idx, tipo)) = self.obter_var_local(nome) {
                    code.push_str(&format!("    ldloc.s {}\n", idx));
                    return tipo;
                }

                // 2. Parâmetro do método
                if let Some((idx, tipo)) = self.obter_parametro(nome) {
                    code.push_str(&format!("    ldarg.s {}\n", idx));
                    return tipo;
                }

                // 3. Enum global ou com namespace
                if let Some(val) = self.obter_valor_enum(nome) {
                    code.push_str(&format!("    ldc.i4 {}\n", val));
                    return ast::Tipo::Inteiro;
                }

                // 4. Membro (campo ou propriedade) da classe atual através de 'este'
                if let Some(ref class_fqn) = self.classe_atual.clone() {
                    if let Some((tipo_campo, dono)) = self.obter_tipo_campo_e_dono(class_fqn, nome) {
                        code.push_str("    ldarg.0\n");
                        let tipo_cil = tipo_para_cil(&tipo_campo);
                        code.push_str(&format!(
                            "    ldfld {} {}::{}\n",
                            tipo_cil,
                            sanitizar_identificador(&dono),
                            sanitizar_identificador(nome)
                        ));
                        return tipo_campo;
                    }

                    if let Some((tipo_prop, dono)) = self.obter_tipo_propriedade_e_dono(class_fqn, nome) {
                        code.push_str("    ldarg.0\n");
                        let tipo_cil = tipo_para_cil(&tipo_prop);
                        code.push_str(&format!(
                            "    callvirt instance {} {}::get_{}()\n",
                            tipo_cil,
                            sanitizar_identificador(&dono),
                            sanitizar_identificador(nome)
                        ));
                        return tipo_prop;
                    }
                }

                // Fallback: assume variável não tipada
                code.push_str(&format!("    // Identificador não resolvido: {}\n", nome));
                code.push_str("    ldnull\n");
                ast::Tipo::Objeto
            }
            ast::Expressao::Aritmetica(op, esq, dir) => {
                let tipo_esq = self.generate_expressao(esq, code);
                let tipo_dir = self.generate_expressao(dir, code);

                // Caso concatenação de strings com '+'
                if *op == ast::OperadorAritmetico::Soma
                    && (tipo_esq == ast::Tipo::Texto || tipo_dir == ast::Tipo::Texto)
                {
                    self.coagir_para_string(&tipo_esq, code, true);
                    self.coagir_para_string(&tipo_dir, code, false);
                    code.push_str("    call string [System.Runtime]System.String::Concat(string, string)\n");
                    return ast::Tipo::Texto;
                }

                let tipo_resultado = match (&tipo_esq, &tipo_dir) {
                    (ast::Tipo::Duplo, _) | (_, ast::Tipo::Duplo) => ast::Tipo::Duplo,
                    (ast::Tipo::Flutuante, _) | (_, ast::Tipo::Flutuante) => ast::Tipo::Flutuante,
                    (ast::Tipo::Decimal, _) | (_, ast::Tipo::Decimal) => ast::Tipo::Decimal,
                    _ => ast::Tipo::Inteiro,
                };

                if tipo_resultado == ast::Tipo::Decimal {
                    match op {
                        ast::OperadorAritmetico::Soma => code.push_str("    call valuetype [System.Runtime]System.Decimal [System.Runtime]System.Decimal::Add(valuetype [System.Runtime]System.Decimal, valuetype [System.Runtime]System.Decimal)\n"),
                        ast::OperadorAritmetico::Subtracao => code.push_str("    call valuetype [System.Runtime]System.Decimal [System.Runtime]System.Decimal::Subtract(valuetype [System.Runtime]System.Decimal, valuetype [System.Runtime]System.Decimal)\n"),
                        ast::OperadorAritmetico::Multiplicacao => code.push_str("    call valuetype [System.Runtime]System.Decimal [System.Runtime]System.Decimal::Multiply(valuetype [System.Runtime]System.Decimal, valuetype [System.Runtime]System.Decimal)\n"),
                        ast::OperadorAritmetico::Divisao => code.push_str("    call valuetype [System.Runtime]System.Decimal [System.Runtime]System.Decimal::Divide(valuetype [System.Runtime]System.Decimal, valuetype [System.Runtime]System.Decimal)\n"),
                        ast::OperadorAritmetico::Modulo => code.push_str("    call valuetype [System.Runtime]System.Decimal [System.Runtime]System.Decimal::Remainder(valuetype [System.Runtime]System.Decimal, valuetype [System.Runtime]System.Decimal)\n"),
                    }
                } else {
                    match op {
                        ast::OperadorAritmetico::Soma => code.push_str("    add\n"),
                        ast::OperadorAritmetico::Subtracao => code.push_str("    sub\n"),
                        ast::OperadorAritmetico::Multiplicacao => code.push_str("    mul\n"),
                        ast::OperadorAritmetico::Divisao => code.push_str("    div\n"),
                        ast::OperadorAritmetico::Modulo => code.push_str("    rem\n"),
                    }
                }

                tipo_resultado
            }
            ast::Expressao::Comparacao(op, esq, dir) => {
                let _ = self.generate_expressao(esq, code);
                let _ = self.generate_expressao(dir, code);

                match op {
                    ast::OperadorComparacao::Igual => {
                        code.push_str("    ceq\n");
                    }
                    ast::OperadorComparacao::Diferente => {
                        code.push_str("    ceq\n");
                        code.push_str("    ldc.i4.0\n");
                        code.push_str("    ceq\n");
                    }
                    ast::OperadorComparacao::Menor => {
                        code.push_str("    clt\n");
                    }
                    ast::OperadorComparacao::MaiorQue => {
                        code.push_str("    cgt\n");
                    }
                    ast::OperadorComparacao::MenorIgual => {
                        code.push_str("    cgt\n");
                        code.push_str("    ldc.i4.0\n");
                        code.push_str("    ceq\n");
                    }
                    ast::OperadorComparacao::MaiorIgual => {
                        code.push_str("    clt\n");
                        code.push_str("    ldc.i4.0\n");
                        code.push_str("    ceq\n");
                    }
                }
                ast::Tipo::Booleano
            }
            ast::Expressao::Logica(op, esq, dir) => {
                let _ = self.generate_expressao(esq, code);
                let _ = self.generate_expressao(dir, code);

                match op {
                    ast::OperadorLogico::E => code.push_str("    and\n"),
                    ast::OperadorLogico::Ou => code.push_str("    or\n"),
                }
                ast::Tipo::Booleano
            }
            ast::Expressao::Unario(op, sub_expr) => {
                let tipo = self.generate_expressao(sub_expr, code);
                match op {
                    ast::OperadorUnario::NegacaoLogica => {
                        code.push_str("    ldc.i4.0\n");
                        code.push_str("    ceq\n");
                        ast::Tipo::Booleano
                    }
                    ast::OperadorUnario::NegacaoNumerica => {
                        code.push_str("    neg\n");
                        tipo
                    }
                }
            }
            ast::Expressao::NovoObjeto(tipo, args) => {
                let class_name = match tipo {
                    ast::Tipo::Classe(n) => self.type_checker.resolver_nome_classe(n, &self.namespace_path),
                    ast::Tipo::Aplicado { nome, .. } => self.type_checker.resolver_nome_classe(nome, &self.namespace_path),
                    _ => "Principal".to_string(),
                };

                // Busca a declaração da classe para verificar construtores com parâmetros padrão
                let class_decl_opt = self.type_checker.classes.get(&class_name)
                    .copied()
                    .or_else(|| {
                        let simple = class_name.split('.').last().unwrap_or(&class_name);
                        self.type_checker.classes.get(simple).copied()
                    })
                    .or_else(|| {
                        self.programa.declaracoes.iter().find_map(|d| match d {
                            ast::Declaracao::DeclaracaoClasse(c) if c.nome == class_name => Some(c),
                            _ => None,
                        })
                    })
                    .or_else(|| {
                        self.programa.namespaces.iter().find_map(|ns| {
                            ns.declaracoes.iter().find_map(|d| match d {
                                ast::Declaracao::DeclaracaoClasse(c)
                                    if c.nome == class_name
                                        || format!("{}.{}", ns.nome, c.nome) == class_name =>
                                {
                                    Some(c)
                                }
                                _ => None,
                            })
                        })
                    });

                let mut ctor_escolhido: Option<&ast::ConstrutorClasse> = None;
                if let Some(class_decl) = class_decl_opt {
                    let mut melhor_total = 0usize;
                    let fornecidos = args.len();
                    for ctor in &class_decl.construtores {
                        let total = ctor.parametros.len();
                        let obrig = ctor.parametros.iter().filter(|p| p.valor_padrao.is_none()).count();
                        if fornecidos >= obrig && fornecidos <= total {
                            if total >= melhor_total {
                                melhor_total = total;
                                ctor_escolhido = Some(ctor);
                            }
                        }
                    }
                }

                let mut tipos_args_cil = Vec::new();
                if let Some(ctor) = ctor_escolhido {
                    let fornecidos = args.len();
                    for (idx, param) in ctor.parametros.iter().enumerate() {
                        if idx < fornecidos {
                            let t = self.generate_expressao(&args[idx], code);
                            tipos_args_cil.push(tipo_para_cil(&t));
                        } else if let Some(ref def_expr) = param.valor_padrao {
                            let t = self.generate_expressao(def_expr, code);
                            tipos_args_cil.push(tipo_para_cil(&t));
                        } else {
                            tipos_args_cil.push(tipo_para_cil(&param.tipo));
                        }
                    }
                } else {
                    for arg in args {
                        let tipo_arg = self.generate_expressao(arg, code);
                        tipos_args_cil.push(tipo_para_cil(&tipo_arg));
                    }
                }

                let class_cil = sanitizar_identificador(&class_name);
                code.push_str(&format!(
                    "    newobj instance void {}::.ctor({})\n",
                    class_cil,
                    tipos_args_cil.join(", ")
                ));

                ast::Tipo::Classe(class_name)
            }
            ast::Expressao::NovoArray(tipo_elem, tam_expr) => {
                let _ = self.generate_expressao(tam_expr, code);
                code.push_str("    conv.i4\n");
                let elem_cil = tipo_para_cil(tipo_elem);
                code.push_str(&format!("    newarr {}\n", elem_cil));
                ast::Tipo::Lista(tipo_elem.clone())
            }
            ast::Expressao::ListaLiteral(itens) => {
                let tipo_elem = if itens.is_empty() {
                    ast::Tipo::Inteiro
                } else {
                    let tipos: Vec<ast::Tipo> = itens
                        .iter()
                        .map(|item| self.inferir_tipo_expressao_simples(item))
                        .collect();

                    let primeiro = tipos[0].clone();
                    let todos_iguais = tipos.iter().all(|t| t == &primeiro);
                    if todos_iguais {
                        primeiro
                    } else {
                        let mut nomes_classes: Vec<String> = Vec::new();
                        let mut todas_classes = true;
                        for t in &tipos {
                            if let ast::Tipo::Classe(ref c) = t {
                                let fqn = self.type_checker.resolver_nome_classe(c, &self.namespace_path);
                                nomes_classes.push(fqn);
                            } else {
                                todas_classes = false;
                                break;
                            }
                        }

                        if todas_classes && !nomes_classes.is_empty() {
                            use std::collections::HashSet;
                            let mut intersec: HashSet<String> = self.type_checker.get_all_interfaces_of_class(&nomes_classes[0]);
                            for c in &nomes_classes[1..] {
                                let si = self.type_checker.get_all_interfaces_of_class(c);
                                intersec = intersec.intersection(&si).cloned().collect();
                                if intersec.is_empty() {
                                    break;
                                }
                            }
                            if let Some(iface) = intersec.into_iter().next() {
                                ast::Tipo::Classe(iface)
                            } else {
                                ast::Tipo::Objeto
                            }
                        } else {
                            ast::Tipo::Objeto
                        }
                    }
                };

                let elem_cil = tipo_para_cil(&tipo_elem);
                code.push_str(&format!("    ldc.i4 {}\n", itens.len()));
                code.push_str(&format!("    newarr {}\n", elem_cil));

                for (i, item) in itens.iter().enumerate() {
                    code.push_str("    dup\n");
                    code.push_str(&format!("    ldc.i4 {}\n", i));
                    let _ = self.generate_expressao(item, code);
                    code.push_str(&format!("    stelem {}\n", elem_cil));
                }

                ast::Tipo::Lista(Box::new(tipo_elem))
            }
            ast::Expressao::AcessoIndice(alvo, idx) => {
                let tipo_alvo = self.generate_expressao(alvo, code);
                let _ = self.generate_expressao(idx, code);
                code.push_str("    conv.i4\n");

                let tipo_elem = match tipo_alvo {
                    ast::Tipo::Lista(t) => *t,
                    _ => ast::Tipo::Objeto,
                };
                let elem_cil = tipo_para_cil(&tipo_elem);
                code.push_str(&format!("    ldelem {}\n", elem_cil));
                tipo_elem
            }
            ast::Expressao::AcessoMembro(alvo, membro) => {
                // Caso especial: enum referenciado como Classe.Membro (ex: Cor.Azul)
                if let ast::Expressao::Identificador(nome_classe) = &**alvo {
                    let fqn_enum = self.resolver_nome_tipo(nome_classe);
                    if let Some(val) = self.obter_valor_enum_membro(&fqn_enum, membro) {
                        code.push_str(&format!("    ldc.i4 {}\n", val));
                        return ast::Tipo::Enum(fqn_enum);
                    }
                }

                let tipo_alvo = self.generate_expressao(alvo, code);

                // Suporte a propriedades intrínsecas: tamanho e comprimento
                if membro == "tamanho" || membro == "comprimento" {
                    match tipo_alvo {
                        ast::Tipo::Lista(_) => {
                            code.push_str("    ldlen\n    conv.i8\n");
                            return ast::Tipo::Inteiro;
                        }
                        ast::Tipo::Texto => {
                            code.push_str("    callvirt instance int32 [System.Runtime]System.String::get_Length()\n    conv.i8\n");
                            return ast::Tipo::Inteiro;
                        }
                        _ => {}
                    }
                }

                let fqn_classe = match tipo_alvo {
                    ast::Tipo::Classe(ref fqn) => self.type_checker.resolver_nome_classe(fqn, &self.namespace_path),
                    _ => "Principal".to_string(),
                };

                if let Some((tipo_prop, dono)) = self.obter_tipo_propriedade_e_dono(&fqn_classe, membro) {
                    let dono_cil = sanitizar_identificador(&dono);
                    let tipo_cil = tipo_para_cil(&tipo_prop);
                    code.push_str(&format!(
                        "    callvirt instance {} {}::get_{}()\n",
                        tipo_cil,
                        dono_cil,
                        sanitizar_identificador(membro)
                    ));
                    return tipo_prop;
                }

                if let Some((tipo_campo, dono)) = self.obter_tipo_campo_e_dono(&fqn_classe, membro) {
                    let dono_cil = sanitizar_identificador(&dono);
                    let tipo_cil = tipo_para_cil(&tipo_campo);
                    code.push_str(&format!(
                        "    ldfld {} {}::{}\n",
                        tipo_cil,
                        dono_cil,
                        sanitizar_identificador(membro)
                    ));
                    return tipo_campo;
                }

                code.push_str("    ldnull\n");
                ast::Tipo::Objeto
            }
            ast::Expressao::ChamadaMetodo(alvo, metodo, args) => {
                if let ast::Expressao::Identificador(nome_id) = &**alvo {
                    if nome_id == "Console" || nome_id == "System.Console" {
                        if metodo == "EscreverLinha" || metodo == "WriteLine" {
                            return self.emitir_escrever_linha(args, code);
                        } else if metodo == "LerLinha" || metodo == "ReadLine" {
                            code.push_str("    call string [System.Console]System.Console::ReadLine()\n");
                            return ast::Tipo::Texto;
                        }
                    }
                }

                // Biblioteca padrão: mapeamento para a BCL do .NET
                // (Matematica→System.Math, Texto→System.String, Guid,
                // Ambiente→System.Environment, Conversao→Parse).
                if let ast::Expressao::Identificador(nome_id) = &**alvo {
                    if let Some(res) = self.tentar_stdlib_bcl(nome_id, metodo, args, code) {
                        return res;
                    }
                }

                // Suporte a intrínsecos: tamanho() e comprimento()
                if (metodo == "tamanho" || metodo == "comprimento") && args.is_empty() {
                    let tipo_alvo = self.generate_expressao(alvo, code);
                    match tipo_alvo {
                        ast::Tipo::Lista(_) => {
                            code.push_str("    ldlen\n    conv.i8\n");
                            return ast::Tipo::Inteiro;
                        }
                        ast::Tipo::Texto => {
                            code.push_str("    callvirt instance int32 [System.Runtime]System.String::get_Length()\n    conv.i8\n");
                            return ast::Tipo::Inteiro;
                        }
                        _ => {}
                    }
                }

                // Verifica se é chamada estática
                let mut is_static = false;
                let mut static_fqn = String::new();

                if let ast::Expressao::Identificador(nome_id) = &**alvo {
                    let fqn = self.type_checker.resolver_nome_classe(nome_id, &self.namespace_path);
                    if self.type_checker.resolved_classes.contains_key(&fqn)
                        || self.type_checker.classes.contains_key(&fqn)
                    {
                        is_static = true;
                        static_fqn = fqn;
                    }
                }

                if is_static {
                    let class_decl_opt = self.type_checker.classes.get(&static_fqn)
                        .copied()
                        .or_else(|| {
                            let simple = static_fqn.split('.').last().unwrap_or(&static_fqn);
                            self.type_checker.classes.get(simple).copied()
                        });
                    let metodo_decl = class_decl_opt.and_then(|cd| cd.metodos.iter().find(|m| m.nome == *metodo));

                    let mut tipos_args_cil = Vec::new();
                    if let Some(m) = metodo_decl {
                        let fornecidos = args.len();
                        for (idx, param) in m.parametros.iter().enumerate() {
                            if idx < fornecidos {
                                let t = self.generate_expressao(&args[idx], code);
                                tipos_args_cil.push(tipo_para_cil(&t));
                            } else if let Some(ref def_expr) = param.valor_padrao {
                                let t = self.generate_expressao(def_expr, code);
                                tipos_args_cil.push(tipo_para_cil(&t));
                            } else {
                                tipos_args_cil.push(tipo_para_cil(&param.tipo));
                            }
                        }
                    } else {
                        for arg in args {
                            let t = self.generate_expressao(arg, code);
                            tipos_args_cil.push(tipo_para_cil(&t));
                        }
                    }

                    let tipo_retorno = self.obter_retorno_metodo(&static_fqn, metodo);
                    let ret_cil = tipo_para_cil(&tipo_retorno);
                    code.push_str(&format!(
                        "    call {} {}::{}({})\n",
                        ret_cil,
                        sanitizar_identificador(&static_fqn),
                        sanitizar_identificador(metodo),
                        tipos_args_cil.join(", ")
                    ));
                    tipo_retorno
                } else {
                    let tipo_alvo = self.generate_expressao(alvo, code);
                    let mut class_fqn = match tipo_alvo {
                        ast::Tipo::Classe(ref fqn) if fqn != "objeto" && fqn != "object" => {
                            self.type_checker.resolver_nome_classe(fqn, &self.namespace_path)
                        }
                        _ => String::new(),
                    };

                    let tem_metodo = !class_fqn.is_empty() && (
                        self.type_checker.resolved_classes.get(&class_fqn).map_or(false, |rc| rc.methods.contains_key(metodo))
                        || self.type_checker.classes.get(&class_fqn).map_or(false, |c| c.metodos.iter().any(|m| m.nome == *metodo))
                        || self.type_checker.interfaces.get(&class_fqn).map_or(false, |i| i.metodos.iter().any(|m| m.nome == *metodo))
                    );

                    if !tem_metodo {
                        if let Some((iface_name, _)) = self.type_checker.interfaces.iter().find(|(_, iface)| {
                            iface.metodos.iter().any(|m| m.nome == *metodo)
                        }) {
                            class_fqn = iface_name.clone();
                            code.push_str(&format!("    castclass {}\n", sanitizar_identificador(&class_fqn)));
                        } else if let Some((c_name, _)) = self.type_checker.resolved_classes.iter().find(|(_, rc)| {
                            rc.methods.contains_key(metodo)
                        }) {
                            class_fqn = c_name.clone();
                            code.push_str(&format!("    castclass {}\n", sanitizar_identificador(&class_fqn)));
                        } else if let Some((c_name, _)) = self.type_checker.classes.iter().find(|(_, c)| {
                            c.metodos.iter().any(|m| m.nome == *metodo)
                        }) {
                            class_fqn = c_name.clone();
                            code.push_str(&format!("    castclass {}\n", sanitizar_identificador(&class_fqn)));
                        } else {
                            class_fqn = self.classe_atual.clone().unwrap_or_else(|| "Principal".to_string());
                        }
                    }

                    let mut parametros_metodo: Option<Vec<ast::Parametro>> = None;
                    if let Some(resolved) = self.type_checker.resolved_classes.get(&class_fqn) {
                        if let Some(m) = resolved.methods.get(metodo) {
                            parametros_metodo = Some(m.parametros.clone());
                        }
                    }
                    if parametros_metodo.is_none() {
                        if let Some(class_decl) = self.type_checker.classes.get(&class_fqn) {
                            if let Some(m) = class_decl.metodos.iter().find(|m| m.nome == *metodo) {
                                parametros_metodo = Some(m.parametros.clone());
                            }
                        }
                    }
                    if parametros_metodo.is_none() {
                        let iface_opt = self.type_checker.interfaces.get(&class_fqn)
                            .or_else(|| {
                                let simple = class_fqn.split('.').last().unwrap_or(&class_fqn);
                                self.type_checker.interfaces.get(simple)
                            });
                        if let Some(iface) = iface_opt {
                            if let Some(m) = iface.metodos.iter().find(|m| m.nome == *metodo) {
                                parametros_metodo = Some(m.parametros.clone());
                            }
                        }
                    }

                    let mut tipos_args_cil = Vec::new();
                    if let Some(params) = parametros_metodo {
                        let fornecidos = args.len();
                        for (idx, param) in params.iter().enumerate() {
                            if idx < fornecidos {
                                let t = self.generate_expressao(&args[idx], code);
                                tipos_args_cil.push(tipo_para_cil(&t));
                            } else if let Some(ref def_expr) = param.valor_padrao {
                                let t = self.generate_expressao(def_expr, code);
                                tipos_args_cil.push(tipo_para_cil(&t));
                            } else {
                                tipos_args_cil.push(tipo_para_cil(&param.tipo));
                            }
                        }
                    } else {
                        for arg in args {
                            let t = self.generate_expressao(arg, code);
                            tipos_args_cil.push(tipo_para_cil(&t));
                        }
                    }

                    let tipo_retorno = self.obter_retorno_metodo(&class_fqn, metodo);
                    let ret_cil = tipo_para_cil(&tipo_retorno);
                    code.push_str(&format!(
                        "    callvirt instance {} {}::{}({})\n",
                        ret_cil,
                        sanitizar_identificador(&class_fqn),
                        sanitizar_identificador(metodo),
                        tipos_args_cil.join(", ")
                    ));
                    tipo_retorno
                }
            }
            ast::Expressao::Chamada(nome_funcao, args) => {
                let short_func_name = nome_funcao.rsplit('.').next().unwrap_or(nome_funcao);
                if short_func_name == "EscreverLinha" {
                    return self.emitir_escrever_linha(args, code);
                }
                if short_func_name == "LerLinha" {
                    code.push_str("    call string [System.Console]System.Console::ReadLine()\n");
                    return ast::Tipo::Texto;
                }

                let fqn_func_name = self.type_checker.resolver_nome_funcao(nome_funcao, &self.namespace_path);
                let func_opt = self.programa.declaracoes.iter().find_map(|d| match d {
                    ast::Declaracao::DeclaracaoFuncao(f)
                        if f.nome == *nome_funcao || self.type_checker.resolver_nome_funcao(&f.nome, "") == fqn_func_name =>
                    {
                        Some(f)
                    }
                    _ => None,
                }).or_else(|| {
                    self.programa.namespaces.iter().find_map(|ns| {
                        ns.declaracoes.iter().find_map(|d| match d {
                            ast::Declaracao::DeclaracaoFuncao(f)
                                if f.nome == *nome_funcao
                                    || self.type_checker.resolver_nome_funcao(&f.nome, &ns.nome) == fqn_func_name =>
                            {
                                Some(f)
                            }
                            _ => None,
                        })
                    })
                });

                let mut tipos_args_cil = Vec::new();
                if let Some(func) = func_opt {
                    let fornecidos = args.len();
                    for (idx, param) in func.parametros.iter().enumerate() {
                        if idx < fornecidos {
                            let t = self.generate_expressao(&args[idx], code);
                            tipos_args_cil.push(tipo_para_cil(&t));
                        } else if let Some(ref def_expr) = param.valor_padrao {
                            let t = self.generate_expressao(def_expr, code);
                            tipos_args_cil.push(tipo_para_cil(&t));
                        } else {
                            tipos_args_cil.push(tipo_para_cil(&param.tipo));
                        }
                    }
                    let tipo_retorno = func.tipo_retorno.clone().unwrap_or(ast::Tipo::Vazio);
                    let ret_cil = tipo_para_cil(&tipo_retorno);
                    code.push_str(&format!(
                        "    call {} Principal::{}({})\n",
                        ret_cil,
                        sanitizar_identificador(&func.nome),
                        tipos_args_cil.join(", ")
                    ));
                    tipo_retorno
                } else {
                    for arg in args {
                        let t = self.generate_expressao(arg, code);
                        tipos_args_cil.push(tipo_para_cil(&t));
                    }
                    let tipo_retorno = self.obter_retorno_metodo("Principal", nome_funcao);
                    let ret_cil = tipo_para_cil(&tipo_retorno);
                    let simple_name = nome_funcao.split('.').last().unwrap_or(nome_funcao);
                    code.push_str(&format!(
                        "    call {} Principal::{}({})\n",
                        ret_cil,
                        sanitizar_identificador(simple_name),
                        tipos_args_cil.join(", ")
                    ));
                    tipo_retorno
                }
            }
            ast::Expressao::StringInterpolada(partes) => {
                if partes.is_empty() {
                    code.push_str("    ldstr \"\"\n");
                    return ast::Tipo::Texto;
                }

                // Carrega cada parte e garante string
                for parte in partes {
                    match parte {
                        ast::PartStringInterpolada::Texto(s) => {
                            code.push_str(&format!("    ldstr \"{}\"\n", self.escapar_string(s)));
                        }
                        ast::PartStringInterpolada::Expressao(e) => {
                            let tipo_e = self.generate_expressao(e, code);
                            self.box_ou_to_string(&tipo_e, code);
                        }
                    }
                }

                let num_partes = partes.len();
                if num_partes == 1 {
                    // Já é string no topo da pilha
                } else if num_partes == 2 {
                    code.push_str("    call string [System.Runtime]System.String::Concat(string, string)\n");
                } else if num_partes == 3 {
                    code.push_str("    call string [System.Runtime]System.String::Concat(string, string, string)\n");
                } else if num_partes == 4 {
                    code.push_str("    call string [System.Runtime]System.String::Concat(string, string, string, string)\n");
                } else {
                    // Para mais de 4 itens, empacota num array de strings
                    // Como as partes já estão na pilha, empilhamos num array invertido
                    // Ou simplificadamente fazemos concatenacões sucessivas
                    for _ in 1..num_partes {
                        code.push_str("    call string [System.Runtime]System.String::Concat(string, string)\n");
                    }
                }

                ast::Tipo::Texto
            }
            ast::Expressao::Aguarde(sub_expr) => {
                match &**sub_expr {
                    ast::Expressao::Chamada(nome, args) => {
                        let short_name = nome.rsplit('.').next().unwrap_or(nome);
                        match short_name {
                            "LerArquivoAssíncrono" => {
                                if let Some(arg) = args.first() {
                                    let _ = self.generate_expressao(arg, code);
                                } else {
                                    code.push_str("    ldstr \"\"\n");
                                }
                                code.push_str("    call string [System.IO.FileSystem]System.IO.File::ReadAllText(string)\n");
                                ast::Tipo::Texto
                            }
                            "EscreverArquivoAssíncrono" => {
                                if args.len() >= 2 {
                                    let _ = self.generate_expressao(&args[0], code);
                                    let _ = self.generate_expressao(&args[1], code);
                                }
                                code.push_str("    call void [System.IO.FileSystem]System.IO.File::WriteAllText(string, string)\n");
                                code.push_str("    ldc.i4.1\n");
                                ast::Tipo::Booleano
                            }
                            "VerificarArquivoAssíncrono" => {
                                if let Some(arg) = args.first() {
                                    let _ = self.generate_expressao(arg, code);
                                } else {
                                    code.push_str("    ldstr \"\"\n");
                                }
                                code.push_str("    call bool [System.IO.FileSystem]System.IO.File::Exists(string)\n");
                                ast::Tipo::Booleano
                            }
                            _ => {
                                self.generate_expressao(sub_expr, code)
                            }
                        }
                    }
                    ast::Expressao::ChamadaMetodo(alvo, metodo, args) => {
                        if let ast::Expressao::Identificador(nome) = &**alvo {
                            if nome == "Arquivo" {
                                match metodo.as_str() {
                                    "LerTextoAssíncrono" => {
                                        if let Some(arg) = args.first() {
                                            let _ = self.generate_expressao(arg, code);
                                        } else {
                                            code.push_str("    ldstr \"\"\n");
                                        }
                                        code.push_str("    call string [System.IO.FileSystem]System.IO.File::ReadAllText(string)\n");
                                        return ast::Tipo::Texto;
                                    }
                                    "EscreverTextoAssíncrono" => {
                                        if args.len() >= 2 {
                                            let _ = self.generate_expressao(&args[0], code);
                                            let _ = self.generate_expressao(&args[1], code);
                                        }
                                        code.push_str("    call void [System.IO.FileSystem]System.IO.File::WriteAllText(string, string)\n");
                                        code.push_str("    ldc.i4.1\n");
                                        return ast::Tipo::Booleano;
                                    }
                                    "ExisteAssíncrono" => {
                                        if let Some(arg) = args.first() {
                                            let _ = self.generate_expressao(arg, code);
                                        } else {
                                            code.push_str("    ldstr \"\"\n");
                                        }
                                        code.push_str("    call bool [System.IO.FileSystem]System.IO.File::Exists(string)\n");
                                        return ast::Tipo::Booleano;
                                    }
                                    _ => {}
                                }
                            }
                        }
                        self.generate_expressao(sub_expr, code)
                    }
                    _ => {
                        self.generate_expressao(sub_expr, code)
                    }
                }
            }
        }
    }

    /// Auxiliar para escapar literais de string em CIL.
    pub fn escapar_string(&self, s: &str) -> String {
        s.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t")
    }

    /// Infere tipo de expressões simples sem gerar código.
    fn inferir_tipo_expressao_simples(&self, expr: &ast::Expressao) -> ast::Tipo {
        match expr {
            ast::Expressao::Inteiro(_) => ast::Tipo::Inteiro,
            ast::Expressao::Texto(_) => ast::Tipo::Texto,
            ast::Expressao::Booleano(_) => ast::Tipo::Booleano,
            ast::Expressao::FlutuanteLiteral(_) => ast::Tipo::Flutuante,
            ast::Expressao::DuploLiteral(_) => ast::Tipo::Duplo,
            ast::Expressao::Decimal(_) => ast::Tipo::Decimal,
            ast::Expressao::Identificador(nome) => {
                if let Some((_, tipo)) = self.obter_var_local(nome) {
                    tipo
                } else if let Some((_, tipo)) = self.obter_parametro(nome) {
                    tipo
                } else {
                    let fqn_cls = self.type_checker.resolver_nome_classe(nome, &self.namespace_path);
                    if self.type_checker.classes.contains_key(&fqn_cls) || self.type_checker.resolved_classes.contains_key(&fqn_cls) {
                        ast::Tipo::Classe(fqn_cls)
                    } else {
                        let fqn_en = self.type_checker.resolver_nome_enum(nome, &self.namespace_path);
                        if self.type_checker.enums.contains_key(&fqn_en) {
                            ast::Tipo::Enum(fqn_en)
                        } else {
                            ast::Tipo::Objeto
                        }
                    }
                }
            }
            ast::Expressao::NovoObjeto(tipo, _) => self.resolver_tipo(tipo),
            ast::Expressao::NovoArray(tipo, _) => ast::Tipo::Lista(Box::new(self.resolver_tipo(tipo))),
            ast::Expressao::AcessoIndice(alvo, _) => {
                match self.inferir_tipo_expressao_simples(alvo) {
                    ast::Tipo::Lista(elem) => *elem,
                    _ => ast::Tipo::Objeto,
                }
            }
            ast::Expressao::ChamadaMetodo(alvo, metodo, _) => {
                let tipo_alvo = self.inferir_tipo_expressao_simples(alvo);
                if let ast::Tipo::Classe(c) = tipo_alvo {
                    let fqn = self.type_checker.resolver_nome_classe(&c, &self.namespace_path);
                    self.obter_retorno_metodo(&fqn, metodo)
                } else {
                    ast::Tipo::Objeto
                }
            }
            ast::Expressao::Chamada(nome_funcao, _) => {
                let short = nome_funcao.rsplit('.').next().unwrap_or(nome_funcao);
                if short == "LerLinha" {
                    return ast::Tipo::Texto;
                }
                if short == "EscreverLinha" {
                    return ast::Tipo::Vazio;
                }
                let fqn = self.type_checker.resolver_nome_funcao(nome_funcao, &self.namespace_path);
                self.obter_retorno_metodo("Principal", &fqn)
            }
            ast::Expressao::Aguarde(inner) => {
                match &**inner {
                    ast::Expressao::Chamada(nome, _) => {
                        let short = nome.rsplit('.').next().unwrap_or(nome);
                        match short {
                            "LerArquivoAssíncrono" => ast::Tipo::Texto,
                            "EscreverArquivoAssíncrono" | "VerificarArquivoAssíncrono" => ast::Tipo::Booleano,
                            _ => {
                                let fqn = self.type_checker.resolver_nome_funcao(nome, &self.namespace_path);
                                self.obter_retorno_metodo("Principal", &fqn)
                            }
                        }
                    }
                    ast::Expressao::ChamadaMetodo(alvo, metodo, _) => {
                        if let ast::Expressao::Identificador(nome) = &**alvo {
                            if nome == "Arquivo" {
                                match metodo.as_str() {
                                    "LerTextoAssíncrono" => return ast::Tipo::Texto,
                                    "EscreverTextoAssíncrono" | "ExisteAssíncrono" => return ast::Tipo::Booleano,
                                    _ => {}
                                }
                            }
                        }
                        self.inferir_tipo_expressao_simples(inner)
                    }
                    _ => self.inferir_tipo_expressao_simples(inner),
                }
            }
            _ => ast::Tipo::Objeto,
        }
    }

    /// Emite chamada para EscreverLinha com suporte a zero, um ou múltiplos argumentos concatenados.
    fn emitir_escrever_linha(&mut self, args: &[ast::Expressao], code: &mut String) -> ast::Tipo {
        if args.is_empty() {
            code.push_str("    call void [System.Console]System.Console::WriteLine()\n");
        } else if args.len() == 1 {
            let t = self.generate_expressao(&args[0], code);
            match t {
                ast::Tipo::Texto => {
                    code.push_str("    call void [System.Console]System.Console::WriteLine(string)\n");
                }
                ast::Tipo::Inteiro => {
                    code.push_str("    call void [System.Console]System.Console::WriteLine(int64)\n");
                }
                ast::Tipo::Booleano => {
                    code.push_str("    call void [System.Console]System.Console::WriteLine(bool)\n");
                }
                ast::Tipo::Flutuante => {
                    code.push_str("    call void [System.Console]System.Console::WriteLine(float32)\n");
                }
                ast::Tipo::Duplo => {
                    code.push_str("    call void [System.Console]System.Console::WriteLine(float64)\n");
                }
                _ => {
                    self.box_ou_to_string(&t, code);
                    code.push_str("    call void [System.Console]System.Console::WriteLine(object)\n");
                }
            }
        } else {
            for (idx, arg) in args.iter().enumerate() {
                let t = self.generate_expressao(arg, code);
                self.coagir_para_string(&t, code, idx == 0);
                if idx > 0 {
                    code.push_str("    call string [System.Runtime]System.String::Concat(string, string)\n");
                }
            }
            code.push_str("    call void [System.Console]System.Console::WriteLine(string)\n");
        }
        ast::Tipo::Vazio
    }

    /// Coage um valor para string se necessário.
    fn coagir_para_string(&self, tipo: &ast::Tipo, code: &mut String, _is_first: bool) {
        if *tipo == ast::Tipo::Texto {
            return;
        }
        let tipo_cil = tipo_para_cil(tipo);
        code.push_str(&format!("    box {}\n", tipo_cil));
        code.push_str("    callvirt instance string [System.Runtime]System.Object::ToString()\n");
    }

    /// Converte um valor primitivo na pilha para string chamando ToString()
    fn box_ou_to_string(&self, tipo: &ast::Tipo, code: &mut String) {
        if *tipo == ast::Tipo::Texto {
            return;
        }
        let tipo_cil = tipo_para_cil(tipo);
        code.push_str(&format!("    box {}\n", tipo_cil));
        code.push_str("    callvirt instance string [System.Runtime]System.Object::ToString()\n");
    }

    /* ------------------------------------------------------------------ */
    /* Mapeamento biblioteca padrão → BCL .NET                              */
    /* ------------------------------------------------------------------ */

    /// Resolve o identificador para o nome simples de uma classe da
    /// biblioteca padrão carregada (.pbl). Retorna None se não for stdlib.
    fn stdlib_classe_simples(&self, nome: &str) -> Option<String> {
        let bib = self.type_checker.biblioteca_externa.as_ref()?;
        let fqn = self
            .type_checker
            .resolver_nome_classe(nome, &self.namespace_path);
        // 1) Correspondência exata pelo FQN resolvido.
        for s in bib.simbolos.values() {
            if let crate::library_loader::LibSimbolo::Classe(c) = s {
                if c.fqn == fqn {
                    return c.fqn.rsplit('.').next().map(|s| s.to_string());
                }
            }
        }
        // 2) Fallback: nome simples único na biblioteca.
        let mut achado: Option<String> = None;
        for s in bib.simbolos.values() {
            if let crate::library_loader::LibSimbolo::Classe(c) = s {
                if c.fqn.rsplit('.').next() == Some(nome) {
                    if achado.is_some() {
                        return None; // ambíguo
                    }
                    achado = c.fqn.rsplit('.').next().map(|s| s.to_string());
                }
            }
        }
        achado
    }

    /// Tenta emitir uma chamada estática da stdlib como chamada BCL.
    /// Retorna o tipo do resultado ou None se o método não tiver
    /// mapeamento (o chamador segue o fluxo normal).
    fn tentar_stdlib_bcl(
        &mut self,
        nome_classe: &str,
        metodo: &str,
        args: &[ast::Expressao],
        code: &mut String,
    ) -> Option<ast::Tipo> {
        let classe = self.stdlib_classe_simples(nome_classe)?;
        let aridade = args.len();
        macro_rules! arg {
            ($i:expr) => {
                self.generate_expressao(&args[$i], code)
            };
        }
        match (classe.as_str(), metodo) {
            /* ---- Sistema.Matematica → System.Math (float64) ---- */
            ("Matematica", m) => {
                const SR: &str = "[System.Runtime]System.Math";
                if (m == "Pi" || m == "E") && aridade == 0 {
                    // PI/E são const literais — ldsfld falha em runtime;
                    // emitir o valor direto (igual ao compilador C#).
                    let valor = if m == "Pi" {
                        "3.14159265358979323846264338327950288"
                    } else {
                        "2.71828182845904523536028747135266250"
                    };
                    code.push_str(&format!("    ldc.r8 {}\n", valor));
                    return Some(ast::Tipo::Duplo);
                }
                let (instr, n_args): (&str, usize) = match m {
                    "Abs" => ("Abs", 1),
                    "Max" => ("Max", 2),
                    "Min" => ("Min", 2),
                    "Potencia" => ("Pow", 2),
                    "RaizQuadrada" => ("Sqrt", 1),
                    "Arredondar" => ("Round", 1),
                    "Truncar" => ("Truncate", 1),
                    "Piso" => ("Floor", 1),
                    "Teto" => ("Ceiling", 1),
                    "Seno" => ("Sin", 1),
                    "Cosseno" => ("Cos", 1),
                    "Tangente" => ("Tan", 1),
                    "LogaritmoNatural" => ("Log", 1),
                    "LogaritmoDez" => ("Log10", 1),
                    "Expoente" => ("Exp", 1),
                    _ => return None,
                };
                if aridade != n_args {
                    return None;
                }
                for i in 0..n_args {
                    let t = arg!(i);
                    // Math.* espera float64; converte inteiros/flutuantes.
                    if t != ast::Tipo::Duplo {
                        code.push_str("    conv.r8\n");
                    }
                }
                if n_args == 1 {
                    code.push_str(&format!(
                        "    call float64 {}::{}(float64)\n",
                        SR, instr
                    ));
                } else {
                    code.push_str(&format!(
                        "    call float64 {}::{}(float64, float64)\n",
                        SR, instr
                    ));
                }
                Some(ast::Tipo::Duplo)
            }

            /* ---- Sistema.Texto.Texto → System.String ---- */
            ("Texto", m) => {
                const S: &str = "[System.Runtime]System.String";
                match m {
                    "ParaMaiusculas" | "ParaMinusculas" | "RemoverEspacos"
                        if aridade == 1 =>
                    {
                        arg!(0);
                        let met = match m {
                            "ParaMaiusculas" => "ToUpper",
                            "ParaMinusculas" => "ToLower",
                            _ => "Trim",
                        };
                        code.push_str(&format!(
                            "    callvirt instance string {}::{}()\n",
                            S, met
                        ));
                        Some(ast::Tipo::Texto)
                    }
                    "VazioOuNulo" if aridade == 1 => {
                        arg!(0);
                        code.push_str(&format!(
                            "    call bool {}::IsNullOrEmpty(string)\n",
                            S
                        ));
                        Some(ast::Tipo::Booleano)
                    }
                    "Tamanho" if aridade == 1 => {
                        arg!(0);
                        code.push_str(&format!(
                            "    callvirt instance int32 {}::get_Length()\n",
                            S
                        ));
                        code.push_str("    conv.i8\n");
                        Some(ast::Tipo::Inteiro)
                    }
                    "EstaVazia" if aridade == 1 => {
                        arg!(0);
                        code.push_str(&format!(
                            "    callvirt instance int32 {}::get_Length()\n",
                            S
                        ));
                        code.push_str("    ldc.i4.0\n    ceq\n");
                        Some(ast::Tipo::Booleano)
                    }
                    "IndiceDe" | "UltimoIndiceDe" if aridade == 2 => {
                        arg!(0);
                        arg!(1);
                        let met = if m == "IndiceDe" { "IndexOf" } else { "LastIndexOf" };
                        code.push_str(&format!(
                            "    callvirt instance int32 {}::{}(string)\n",
                            S, met
                        ));
                        code.push_str("    conv.i8\n");
                        Some(ast::Tipo::Inteiro)
                    }
                    "Subtexto" if aridade == 3 => {
                        arg!(0);
                        arg!(1);
                        code.push_str("    conv.i4\n");
                        arg!(2);
                        code.push_str("    conv.i4\n");
                        code.push_str(&format!(
                            "    callvirt instance string {}::Substring(int32, int32)\n",
                            S
                        ));
                        Some(ast::Tipo::Texto)
                    }
                    "PadEsquerda" | "PadDireita" if aridade == 3 => {
                        arg!(0);
                        arg!(1);
                        code.push_str("    conv.i4\n");
                        arg!(2);
                        code.push_str("    ldc.i4.0\n");
                        code.push_str(&format!(
                            "    callvirt instance char {}::get_Chars(int32)\n",
                            S
                        ));
                        let met = if m == "PadEsquerda" { "PadLeft" } else { "PadRight" };
                        code.push_str(&format!(
                            "    callvirt instance string {}::{}(int32, char)\n",
                            S, met
                        ));
                        Some(ast::Tipo::Texto)
                    }
                    "ComecaCom" | "TerminaCom" | "Contem" if aridade == 2 => {
                        arg!(0);
                        arg!(1);
                        let met = match m {
                            "ComecaCom" => "StartsWith",
                            "TerminaCom" => "EndsWith",
                            _ => "Contains",
                        };
                        code.push_str(&format!(
                            "    callvirt instance bool {}::{}(string)\n",
                            S, met
                        ));
                        Some(ast::Tipo::Booleano)
                    }
                    "Substituir" if aridade == 3 => {
                        arg!(0);
                        arg!(1);
                        arg!(2);
                        code.push_str(&format!(
                            "    callvirt instance string {}::Replace(string, string)\n",
                            S
                        ));
                        Some(ast::Tipo::Texto)
                    }
                    "Comparar" if aridade == 2 => {
                        arg!(0);
                        arg!(1);
                        code.push_str(&format!(
                            "    call int32 {}::CompareOrdinal(string, string)\n",
                            S
                        ));
                        code.push_str("    conv.i8\n");
                        Some(ast::Tipo::Inteiro)
                    }
                    "IgualIgnoreCase" if aridade == 2 => {
                        arg!(0);
                        arg!(1);
                        code.push_str("    ldc.i4.5\n"); // StringComparison.OrdinalIgnoreCase
                        code.push_str(&format!(
                            "    call bool {}::Equals(string, string, valuetype [System.Runtime]System.StringComparison)\n",
                            S
                        ));
                        Some(ast::Tipo::Booleano)
                    }
                    "Remover" if aridade == 3 => {
                        arg!(0);
                        arg!(1);
                        code.push_str("    conv.i4\n");
                        arg!(2);
                        code.push_str("    conv.i4\n");
                        code.push_str(&format!(
                            "    callvirt instance string {}::Remove(int32, int32)\n",
                            S
                        ));
                        Some(ast::Tipo::Texto)
                    }
                    "Inserir" if aridade == 3 => {
                        arg!(0);
                        arg!(1);
                        code.push_str("    conv.i4\n");
                        arg!(2);
                        code.push_str(&format!(
                            "    callvirt instance string {}::Insert(int32, string)\n",
                            S
                        ));
                        Some(ast::Tipo::Texto)
                    }
                    _ => None,
                }
            }

            /* ---- Sistema.Guid.Guid → System.Guid ---- */
            ("Guid", m) => {
                const G: &str = "[System.Runtime]System.Guid";
                match m {
                    "Novo" | "Vazio" if aridade == 0 => {
                        if m == "Novo" {
                            code.push_str(&format!(
                                "    call valuetype {} {}::NewGuid()\n",
                                G, G
                            ));
                        } else {
                            // Guid.Empty é campo estático (não propriedade).
                            code.push_str(&format!(
                                "    ldsfld valuetype {} {}::Empty\n",
                                G, G
                            ));
                        }
                        // Guid é valuetype: box antes de chamar ToString
                        // virtual (callvirt direto em struct não é válido).
                        code.push_str(&format!("    box {}\n", G));
                        code.push_str(
                            "    callvirt instance string [System.Runtime]System.Object::ToString()\n",
                        );
                        Some(ast::Tipo::Texto)
                    }
                    _ => None,
                }
            }

            /* ---- Sistema.Ambiente.Ambiente → System.Environment ---- */
            ("Ambiente", m) => {
                const E: &str = "[System.Runtime]System.Environment";
                match m {
                    "NomeComputador" if aridade == 0 => {
                        code.push_str(&format!(
                            "    call string {}::get_MachineName()\n",
                            E
                        ));
                        Some(ast::Tipo::Texto)
                    }
                    "VariavelAmbiente" if aridade == 1 => {
                        arg!(0);
                        code.push_str(&format!(
                            "    call string {}::GetEnvironmentVariable(string)\n",
                            E
                        ));
                        Some(ast::Tipo::Texto)
                    }
                    "DefinirVariavelAmbiente" if aridade == 2 => {
                        arg!(0);
                        arg!(1);
                        code.push_str(&format!(
                            "    call void {}::SetEnvironmentVariable(string, string)\n",
                            E
                        ));
                        Some(ast::Tipo::Vazio)
                    }
                    _ => None,
                }
            }

            /* ---- Sistema.Conversao.Conversao → Parse/ToBoolean ---- */
            ("Conversao", m) => {
                const S: &str = "[System.Runtime]System.String";
                const CI: &str = "[System.Runtime]System.Globalization.CultureInfo";
                // Parse com cultura invariante: "3.5" deve ser 3.5 em
                // qualquer localidade (o interpretador usa strtod/C locale).
                // Emite: arg; InvariantCulture; call Parse(string, IFormatProvider)
                macro_rules! parse_invariante {
                    ($ret:expr, $dono:expr) => {{
                        arg!(0);
                        code.push_str(&format!(
                            "    call valuetype {} {}::get_InvariantCulture()\n",
                            CI, CI
                        ));
                        code.push_str(&format!(
                            "    call {} {}::Parse(string, class [System.Runtime]System.IFormatProvider)\n",
                            $ret, $dono
                        ));
                    }};
                }
                match m {
                    "ParaInteiro" if aridade == 1 => {
                        parse_invariante!("int64", "[System.Runtime]System.Int64");
                        Some(ast::Tipo::Inteiro)
                    }
                    "ParaFlutuante" if aridade == 1 => {
                        parse_invariante!("float32", "[System.Runtime]System.Single");
                        Some(ast::Tipo::Flutuante)
                    }
                    "ParaDuplo" if aridade == 1 => {
                        parse_invariante!("float64", "[System.Runtime]System.Double");
                        Some(ast::Tipo::Duplo)
                    }
                    "ParaDecimal" if aridade == 1 => {
                        arg!(0);
                        code.push_str(&format!(
                            "    call valuetype {} {}::get_InvariantCulture()\n",
                            CI, CI
                        ));
                        code.push_str(
                            "    call valuetype [System.Runtime]System.Decimal [System.Runtime]System.Decimal::Parse(string, class [System.Runtime]System.IFormatProvider)\n",
                        );
                        Some(ast::Tipo::Decimal)
                    }
                    "ParaBooleano" if aridade == 1 => {
                        arg!(0);
                        code.push_str(&format!(
                            "    callvirt instance string {}::ToLower()\n",
                            S
                        ));
                        // Normaliza literais Por do Sol para true/false.
                        for (de, para) in [
                            ("verdadeiro", "true"),
                            ("falso", "false"),
                            ("sim", "true"),
                            ("nao", "false"),
                        ] {
                            code.push_str(&format!("    ldstr \"{}\"\n", de));
                            code.push_str(&format!("    ldstr \"{}\"\n", para));
                            code.push_str(&format!(
                                "    callvirt instance string {}::Replace(string, string)\n",
                                S
                            ));
                        }
                        code.push_str(
                            "    call bool [System.Runtime]System.Boolean::Parse(string)\n",
                        );
                        Some(ast::Tipo::Booleano)
                    }
                    _ => None,
                }
            }

            _ => None,
        }
    }
}

