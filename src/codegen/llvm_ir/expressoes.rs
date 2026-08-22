use crate::ast;
use std::collections::{HashMap, HashSet};
use crate::library_loader::LibSimbolo;

use super::LlvmGenerator;

impl<'a> LlvmGenerator<'a> {
    pub(crate) fn const_llvm_init_for_expr(
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

    pub(crate) fn get_member_ptr(
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
    pub(crate) fn get_declaring_class_of_method(&self, metodo_ref: &'a ast::MetodoClasse) -> Option<String> {
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

    pub(crate) fn store_variable(&mut self, name: &str, value_type: &ast::Tipo, value_reg: &str) {
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

    /// Retorna o helper do runtime e o tipo produzido por cada operação de
    /// arquivo assíncrona que o alvo LLVM disponibiliza nativamente.
    pub(crate) fn generate_expressao(&mut self, expr: &ast::Expressao) -> (String, ast::Tipo) {
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
            ast::Expressao::NovoObjeto(tipo, argumentos) => {
                let (nome_classe, tipo_resultado, base_fqn) = match tipo {
                    ast::Tipo::Classe(n) => {
                        let fqn = self
                            .type_checker
                            .resolver_nome_classe(n, &self.namespace_path);
                        (fqn.clone(), ast::Tipo::Classe(fqn.clone()), fqn)
                    }
                    ast::Tipo::Aplicado { ref nome, ref args } => {
                        let fqn_base = self
                            .type_checker
                            .resolver_nome_classe(nome, &self.namespace_path);
                        let norm_args: Vec<ast::Tipo> = args
                            .iter()
                            .map(|a| self.resolve_type(a, &self.namespace_path))
                            .collect();
                        let mangled_name = self.mangle_aplicado_name(&fqn_base, &norm_args);
                        // Register this instantiation for struct generation
                        self.applied_class_insts
                            .entry(fqn_base.clone())
                            .or_default()
                            .push(norm_args.clone());
                        // Generate the struct definition immediately in the header
                        self.define_applied_struct(&fqn_base, &norm_args);
                        (
                            mangled_name.clone(),
                            ast::Tipo::Aplicado {
                                nome: mangled_name,
                                args: norm_args.clone(),
                            },
                            fqn_base,
                        )
                    }
                    _ => panic!("Instanciação de tipo não suportado em LLVM IR: {:?}", tipo),
                };
                let fqn = self
                    .type_checker
                    .resolver_nome_classe(&base_fqn, &self.namespace_path);
                // Bloquear instanciação de classe abstrata
                if let Some(class_decl) = self.type_checker.classes.get(&fqn) {
                    if class_decl.eh_abstrata {
                        panic!("Não é possível instanciar classe abstrata: {}", fqn);
                    }
                }
                let sanitized_fqn = nome_classe.replace('.', "_");
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
                // Use the base FQN for vtable (generic classes reuse base class vtable)
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
                if let Some(class_decl) = self.type_checker.classes.get(&base_fqn) {
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
                                    panic!("Argumento obrigatório ausente para parâmetro '{}' do construtor de '{}'", param.nome, base_fqn);
                                }
                            }
                        }

                        // Chamada ao construtor LLVM
                        let func_name =
                            format!("{0}::construtor${1}", nome_classe, ctor.parametros.len())
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

                (obj_ptr_reg, tipo_resultado)
            }
            ast::Expressao::NovoArray(tipo, _tamanho) => {
                // Para biblioteca padrão, criamos um array vazio como placeholder
                // Em uma implementação completa, precisaria alocar dinamicamente
                let array_reg = self.get_unique_temp_name();
                self.body
                    .push_str(&format!("  {} = call i8* @malloc(i64 16)\n", array_reg));
                // Inicializa array vazio: tamanho = 0, ponteiro = null
                let gep_len = self.get_unique_temp_name();
                self.body.push_str(&format!(
                    "  {} = getelementptr inbounds [2 x i32], [2 x i32]* {}, i32 0, i32 0\n",
                    gep_len, array_reg
                ));
                self.body
                    .push_str(&format!("  store i32 0, i32* {}\n", gep_len));
                let gep_data = self.get_unique_temp_name();
                self.body.push_str(&format!(
                    "  {} = getelementptr inbounds [2 x i32], [2 x i32]* {}, i32 0, i32 1\n",
                    gep_data, array_reg
                ));
                self.body
                    .push_str(&format!("  store i8* null, i8** {}\n", gep_data));
                (array_reg, ast::Tipo::Lista((*tipo).clone()))
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

                let fqn_class_name = match &obj_type {
                    ast::Tipo::Classe(ref name) => {
                        self.type_checker.resolver_nome_classe(name, &self.namespace_path)
                    }
                    ast::Tipo::Aplicado { ref nome, ref args } => {
                        if args.is_empty() {
                            // Tipo já resolvido e mangled por `resolve_type`, `nome` é o FQN mangled.
                            nome.clone()
                        } else {
                           // Tipo não resolvido (ex: de um acesso a membro), precisa ser processado.
                           let fqn_base = self.type_checker.resolver_nome_classe(nome, &self.namespace_path);
                           let norm_args: Vec<ast::Tipo> = args.iter().map(|a| self.resolve_type(a, &self.namespace_path)).collect();
                           self.mangle_aplicado_name(&fqn_base, &norm_args)
                        }
                    },
                    _ => panic!(
                        "Chamada de método em algo que não é um objeto. metodo='{}' obj_type={:?} obj_expr={:?}",
                        metodo_nome,
                        &obj_type,
                        obj_expr
                    ),
                };
                // Determina se é virtual (tem índice de vtable)
                let vtable_idx_opt = self
                    .vtable_index
                    .get(&fqn_class_name)
                    .and_then(|m| m.get(metodo_nome).cloned());

                // Resolve tipo de retorno pela classe estática
                // Para classes genéricas mangled, tenta base name se não encontrar no mangled
                let resolved_method = self
                    .resolved_classes
                    .get(&fqn_class_name)
                    .and_then(|c| c.methods.get(metodo_nome))
                    .cloned()
                    .or_else(|| {
                        // Se não encontrou no nome mangled, tenta extrair o nome base
                        if fqn_class_name.contains('$') {
                            let base_name =
                                fqn_class_name.split('$').next().unwrap_or(&fqn_class_name);
                            self.resolved_classes
                                .get(base_name)
                                .and_then(|c| c.methods.get(metodo_nome))
                                .cloned()
                        } else {
                            None
                        }
                    })
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
                    // args_values já inclui o objeto (self) como primeiro argumento;
                    // não duplique o self aqui.
                    let args_vals: Vec<String> = args_values
                        .into_iter()
                        .map(|(reg, ty)| format!("{0} {1}", self.map_type_to_llvm_arg(&ty), reg))
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
                    (ast::Tipo::Classe(_), ast::Tipo::Classe(_)) => {
                        // Para comparação de classes/objetos, tratamos como comparação de ponteiros
                        let pred = match op {
                            ast::OperadorComparacao::Igual => "eq",
                            ast::OperadorComparacao::Diferente => "ne",
                            _ => "eq", // Para comparações de objeto, usamos igualdade por padrão
                        };
                        self.body.push_str(&format!(
                            "  {0} = icmp {1} i8* {2}, {3}\n",
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
            ast::Expressao::Logica(op, esq, dir) => {
                let (left_reg, left_type) = self.generate_expressao(esq);
                let (right_reg, right_type) = self.generate_expressao(dir);
                let result_reg = self.get_unique_temp_name();

                match op {
                    ast::OperadorLogico::E => {
                        // Para E (AND): primeiro extende ambos para i1, depois faz and
                        let left_bool = self.ensure_bool(&left_reg, &left_type);
                        let right_bool = self.ensure_bool(&right_reg, &right_type);
                        self.body.push_str(&format!(
                            "  {0} = and i1 {1}, {2}\n",
                            result_reg, left_bool, right_bool
                        ));
                    }
                    ast::OperadorLogico::Ou => {
                        // Para OU (OR): primeiro extende ambos para i1, depois faz or
                        let left_bool = self.ensure_bool(&left_reg, &left_type);
                        let right_bool = self.ensure_bool(&right_reg, &right_type);
                        self.body.push_str(&format!(
                            "  {0} = or i1 {1}, {2}\n",
                            result_reg, left_bool, right_bool
                        ));
                    }
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
            ast::Expressao::Aguarde(inner) => match inner.as_ref() {
                ast::Expressao::Chamada(name, arguments)
                    if self.native_async_operation(name).is_some() =>
                {
                    self.await_native_operation(name, arguments)
                }
                ast::Expressao::Chamada(name, arguments) => {
                    let fqn = self
                        .type_checker
                        .resolver_nome_funcao(name, &self.namespace_path);
                    self.await_async_function(&fqn, arguments)
                }
                ast::Expressao::ChamadaMetodo(object, method, arguments) => {
                    // A biblioteca padrão expõe as mesmas operações como
                    // Arquivo.LerTextoAssíncrono/EscreverTextoAssíncrono/ExisteAssíncrono.
                    if matches!(object.as_ref(), ast::Expressao::Identificador(name) if name == "Arquivo")
                    {
                        let operation = match method.as_str() {
                            "LerTextoAssíncrono" => "LerArquivoAssíncrono",
                            "EscreverTextoAssíncrono" => "EscreverArquivoAssíncrono",
                            "ExisteAssíncrono" => "VerificarArquivoAssíncrono",
                            _ => panic!(
                                "Método async '{}' não é suportado pelo runtime LLVM",
                                method
                            ),
                        };
                        self.await_native_operation(operation, arguments)
                    } else {
                        panic!("aguarde requer uma chamada async nativa ou uma função marcada como assíncrona")
                    }
                }
                _ => panic!("aguarde requer uma chamada async"),
            },
            ast::Expressao::Este => self.load_variable("self"),
            ast::Expressao::Nulo => {
                // Para nulo, retornamos um null pointer para o tipo genérico
                let null_reg = self.get_unique_temp_name();
                self.body.push_str(&format!(
                    "  {} = getelementptr i8, i8* null, i32 0\n",
                    null_reg
                ));
                (null_reg, ast::Tipo::Classe("objeto".to_string()))
            }
            ast::Expressao::Unario(op, expr) => {
                let (reg, tipo) = self.generate_expressao(expr);
                let result_reg = self.get_unique_temp_name();
                match op {
                    ast::OperadorUnario::NegacaoNumerica => match tipo {
                        ast::Tipo::Inteiro => {
                            self.body
                                .push_str(&format!("  {} = sub i32 0, {}\n", result_reg, reg));
                        }
                        ast::Tipo::Flutuante => {
                            let ensured = self.ensure_float(&reg, &tipo);
                            self.body.push_str(&format!(
                                "  {} = fsub float -0.0, {}\n",
                                result_reg, ensured
                            ));
                        }
                        ast::Tipo::Duplo => {
                            let ensured = self.ensure_double(&reg, &tipo);
                            self.body.push_str(&format!(
                                "  {} = fsub double -0.0, {}\n",
                                result_reg, ensured
                            ));
                        }
                        _ => panic!("Negação numérica não suportada para tipo: {:?}", tipo),
                    },
                    ast::OperadorUnario::NegacaoLogica => {
                        let bool_reg = self.ensure_bool(&reg, &tipo);
                        self.body
                            .push_str(&format!("  {} = xor i1 {}, true\n", result_reg, bool_reg));
                    }
                }
                (result_reg, tipo)
            }
        }
    }

    pub(crate) fn load_variable(&mut self, name: &str) -> (String, ast::Tipo) {
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

}
