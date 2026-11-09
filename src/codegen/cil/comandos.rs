// src/codegen/cil/comandos.rs

use crate::ast;
use super::tipos::{sanitizar_identificador, tipo_para_cil};
use super::CilGenerator;

impl<'a> CilGenerator<'a> {
    /// Gera instruções CIL para um comando individual.
    pub fn generate_comando(&mut self, comando: &ast::Comando, code: &mut String) {
        match comando {
            ast::Comando::DeclaracaoVar(nome, expr) => {
                let tipo = self.generate_expressao(expr, code);
                let idx = self.registrar_var_local(nome, tipo);
                code.push_str(&format!("    stloc.s {}\n", idx));
            }
            ast::Comando::DeclaracaoVariavel(tipo, nome, init) => {
                let tipo_resolvido = self.resolver_tipo(tipo);
                let idx = self.registrar_var_local(nome, tipo_resolvido);
                if let Some(expr) = init {
                    let _ = self.generate_expressao(expr, code);
                    code.push_str(&format!("    stloc.s {}\n", idx));
                }
            }
            ast::Comando::Atribuicao(nome, expr) => {
                // 1. Variável local
                if let Some((idx, _)) = self.obter_var_local(nome) {
                    let _ = self.generate_expressao(expr, code);
                    code.push_str(&format!("    stloc.s {}\n", idx));
                    return;
                }

                // 2. Parâmetro
                if let Some((idx, _)) = self.obter_parametro(nome) {
                    let _ = self.generate_expressao(expr, code);
                    code.push_str(&format!("    starg.s {}\n", idx));
                    return;
                }

                // 3. Propriedade ou campo da classe atual
                if let Some(ref class_fqn) = self.classe_atual.clone() {
                    if let Some(tipo_prop) = self.obter_tipo_propriedade(class_fqn, nome) {
                        code.push_str("    ldarg.0\n");
                        let _ = self.generate_expressao(expr, code);
                        let tipo_cil = tipo_para_cil(&tipo_prop);
                        code.push_str(&format!(
                            "    callvirt instance void {}::set_{}({})\n",
                            sanitizar_identificador(class_fqn),
                            sanitizar_identificador(nome),
                            tipo_cil
                        ));
                        return;
                    }

                    if let Some(tipo_campo) = self.obter_tipo_campo(class_fqn, nome) {
                        code.push_str("    ldarg.0\n");
                        let _ = self.generate_expressao(expr, code);
                        let tipo_cil = tipo_para_cil(&tipo_campo);
                        code.push_str(&format!(
                            "    stfld {} {}::{}\n",
                            tipo_cil,
                            sanitizar_identificador(class_fqn),
                            sanitizar_identificador(nome)
                        ));
                        return;
                    }
                }

                // Fallback: registra como variável local caso não exista
                let tipo = self.generate_expressao(expr, code);
                let idx = self.registrar_var_local(nome, tipo);
                code.push_str(&format!("    stloc.s {}\n", idx));
            }
            ast::Comando::AtribuirPropriedade(alvo, prop, expr) => {
                let tipo_alvo = self.generate_expressao(alvo, code);
                let tipo_expr = self.generate_expressao(expr, code);
                let class_name = match tipo_alvo {
                    ast::Tipo::Classe(ref fqn) => self.type_checker.resolver_nome_classe(fqn, &self.namespace_path),
                    _ => self.classe_atual.clone().unwrap_or_else(|| "Principal".to_string()),
                };
                if let Some((tipo_prop, dono)) = self.obter_tipo_propriedade_e_dono(&class_name, prop) {
                    let dono_cil = sanitizar_identificador(&dono);
                    let tipo_cil = tipo_para_cil(&tipo_prop);
                    code.push_str(&format!(
                        "    callvirt instance void {}::set_{}({})\n",
                        dono_cil,
                        sanitizar_identificador(prop),
                        tipo_cil
                    ));
                } else if let Some((tipo_campo, dono)) = self.obter_tipo_campo_e_dono(&class_name, prop) {
                    let dono_cil = sanitizar_identificador(&dono);
                    let tipo_cil = tipo_para_cil(&tipo_campo);
                    code.push_str(&format!(
                        "    stfld {} {}::{}\n",
                        tipo_cil,
                        dono_cil,
                        sanitizar_identificador(prop)
                    ));
                } else {
                    let class_cil = sanitizar_identificador(&class_name);
                    let tipo_cil = tipo_para_cil(&tipo_expr);
                    code.push_str(&format!(
                        "    callvirt instance void {}::set_{}({})\n",
                        class_cil,
                        sanitizar_identificador(prop),
                        tipo_cil
                    ));
                }
            }
            ast::Comando::AtribuirCampo(alvo, campo, expr) => {
                let tipo_alvo = self.generate_expressao(alvo, code);
                let tipo_expr = self.generate_expressao(expr, code);
                let class_name = match tipo_alvo {
                    ast::Tipo::Classe(ref fqn) => self.type_checker.resolver_nome_classe(fqn, &self.namespace_path),
                    _ => self.classe_atual.clone().unwrap_or_else(|| "Principal".to_string()),
                };
                if let Some((tipo_campo, dono)) = self.obter_tipo_campo_e_dono(&class_name, campo) {
                    let dono_cil = sanitizar_identificador(&dono);
                    let tipo_cil = tipo_para_cil(&tipo_campo);
                    code.push_str(&format!(
                        "    stfld {} {}::{}\n",
                        tipo_cil,
                        dono_cil,
                        sanitizar_identificador(campo)
                    ));
                } else if let Some((tipo_prop, dono)) = self.obter_tipo_propriedade_e_dono(&class_name, campo) {
                    let dono_cil = sanitizar_identificador(&dono);
                    let tipo_cil = tipo_para_cil(&tipo_prop);
                    code.push_str(&format!(
                        "    callvirt instance void {}::set_{}({})\n",
                        dono_cil,
                        sanitizar_identificador(campo),
                        tipo_cil
                    ));
                } else {
                    let class_cil = sanitizar_identificador(&class_name);
                    let tipo_cil = tipo_para_cil(&tipo_expr);
                    code.push_str(&format!(
                        "    stfld {} {}::{}\n",
                        tipo_cil,
                        class_cil,
                        sanitizar_identificador(campo)
                    ));
                }
            }
            ast::Comando::AtribuirIndice(alvo, idx, expr) => {
                let tipo_alvo = self.generate_expressao(alvo, code);
                let _ = self.generate_expressao(idx, code);
                code.push_str("    conv.i4\n");
                let tipo_expr = self.generate_expressao(expr, code);
                let tipo_elem = match tipo_alvo {
                    ast::Tipo::Lista(t) => *t,
                    _ => tipo_expr,
                };
                let elem_cil = tipo_para_cil(&tipo_elem);
                code.push_str(&format!("    stelem {}\n", elem_cil));
            }
            ast::Comando::Imprima(expr) => {
                let tipo = self.generate_expressao(expr, code);
                match tipo {
                    ast::Tipo::Inteiro => {
                        code.push_str("    call void [System.Console]System.Console::WriteLine(int64)\n");
                    }
                    ast::Tipo::Texto => {
                        code.push_str("    call void [System.Console]System.Console::WriteLine(string)\n");
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
                    ast::Tipo::Decimal => {
                        code.push_str("    call void [System.Console]System.Console::WriteLine(valuetype [System.Runtime]System.Decimal)\n");
                    }
                    _ => {
                        let tipo_cil = tipo_para_cil(&tipo);
                        code.push_str(&format!("    box {}\n", tipo_cil));
                        code.push_str("    call void [System.Console]System.Console::WriteLine(object)\n");
                    }
                }
            }
            ast::Comando::Se(cond, bloco_se, bloco_senao) => {
                let _ = self.generate_expressao(cond, code);
                let lbl_else = self.novo_label("IL_ELSE");
                let lbl_end = self.novo_label("IL_END");

                if let Some(ref senao) = bloco_senao {
                    code.push_str(&format!("    brfalse {}\n", lbl_else));
                    self.generate_comando(bloco_se, code);
                    code.push_str(&format!("    br {}\n", lbl_end));
                    code.push_str(&format!("{}:\n", lbl_else));
                    self.generate_comando(senao, code);
                    code.push_str(&format!("{}:\n", lbl_end));
                } else {
                    code.push_str(&format!("    brfalse {}\n", lbl_end));
                    self.generate_comando(bloco_se, code);
                    code.push_str(&format!("{}:\n", lbl_end));
                }
            }
            ast::Comando::Enquanto(cond, corpo) => {
                let lbl_start = self.novo_label("IL_WHILE_START");
                let lbl_end = self.novo_label("IL_WHILE_END");

                code.push_str(&format!("{}:\n", lbl_start));
                let _ = self.generate_expressao(cond, code);
                code.push_str(&format!("    brfalse {}\n", lbl_end));
                self.generate_comando(corpo, code);
                code.push_str(&format!("    br {}\n", lbl_start));
                code.push_str(&format!("{}:\n", lbl_end));
            }
            ast::Comando::Para(init, cond, passo, corpo) => {
                if let Some(i) = init {
                    self.generate_comando(i, code);
                }
                let lbl_start = self.novo_label("IL_FOR_START");
                let lbl_end = self.novo_label("IL_FOR_END");
                let lbl_step = self.novo_label("IL_FOR_STEP");

                code.push_str(&format!("{}:\n", lbl_start));
                if let Some(c) = cond {
                    let _ = self.generate_expressao(c, code);
                    code.push_str(&format!("    brfalse {}\n", lbl_end));
                }
                self.generate_comando(corpo, code);
                code.push_str(&format!("{}:\n", lbl_step));
                if let Some(p) = passo {
                    self.generate_comando(p, code);
                }
                code.push_str(&format!("    br {}\n", lbl_start));
                code.push_str(&format!("{}:\n", lbl_end));
            }
            ast::Comando::Bloco(comandos) => {
                for cmd in comandos {
                    self.generate_comando(cmd, code);
                }
            }
            ast::Comando::Retorne(expr_opt) => {
                if let Some(expr) = expr_opt {
                    let _ = self.generate_expressao(expr, code);
                }
                code.push_str("    ret\n");
            }
            ast::Comando::Expressao(expr) => {
                let tipo = self.generate_expressao(expr, code);
                if tipo != ast::Tipo::Vazio {
                    code.push_str("    pop\n");
                }
            }
            ast::Comando::CriarObjeto(var_nome, classe, args) => {
                let expr_novo = ast::Expressao::NovoObjeto(ast::Tipo::Classe(classe.clone()), args.clone());
                let tipo = self.generate_expressao(&expr_novo, code);
                let idx = self.registrar_var_local(var_nome, tipo);
                code.push_str(&format!("    stloc.s {}\n", idx));
            }
            ast::Comando::ChamarMetodo(alvo, metodo, args) => {
                let expr_chama = ast::Expressao::ChamadaMetodo(alvo.clone(), metodo.clone(), args.clone());
                let tipo = self.generate_expressao(&expr_chama, code);
                if tipo != ast::Tipo::Vazio {
                    code.push_str("    pop\n");
                }
            }
            ast::Comando::AcessarCampo(_, _) => {}
        }
    }
}

