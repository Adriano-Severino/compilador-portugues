use crate::ast;
use std::collections::{HashMap, HashSet};
use crate::library_loader::LibSimbolo;

use super::LlvmGenerator;

impl<'a> LlvmGenerator<'a> {
    pub(crate) fn generate_comando(&mut self, comando: &ast::Comando) {
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
                let class_name = match &obj_type {
                    ast::Tipo::Classe(name) => name.clone(),
                    ast::Tipo::Aplicado { ref nome, ref args } => {
                        if args.is_empty() {
                            // Tipo já resolvido e mangled por `resolve_type`, `nome` é o FQN mangled.
                            nome.clone()
                        } else {
                            // Tipo não resolvido (ex: de um acesso a membro), precisa ser processado.
                            let fqn_base = self
                                .type_checker
                                .resolver_nome_classe(nome, &self.namespace_path);
                            let norm_args: Vec<ast::Tipo> = args
                                .iter()
                                .map(|a| self.resolve_type(a, &self.namespace_path))
                                .collect();
                            self.mangle_aplicado_name(&fqn_base, &norm_args)
                        }
                    }
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

    pub(crate) fn declare_and_store_variable(
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

}
