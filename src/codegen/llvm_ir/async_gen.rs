use crate::ast;
use std::collections::{HashMap, HashSet};
use crate::library_loader::LibSimbolo;

use super::LlvmGenerator;

impl<'a> LlvmGenerator<'a> {
    pub(crate) fn native_async_operation(&self, name: &str) -> Option<(&'static str, ast::Tipo)> {
        match name {
            "LerArquivoAssíncrono" => Some(("task_create_read_file", ast::Tipo::Texto)),
            "EscreverArquivoAssíncrono" => Some(("task_create_write_file", ast::Tipo::Booleano)),
            "VerificarArquivoAssíncrono" => Some(("task_create_file_exists", ast::Tipo::Booleano)),
            _ => None,
        }
    }

    pub(crate) fn await_native_operation(
        &mut self,
        operation: &str,
        arguments: &[ast::Expressao],
    ) -> (String, ast::Tipo) {
        let (runtime_function, result_type) =
            self.native_async_operation(operation).unwrap_or_else(|| {
                panic!(
                    "Opera\u{00e7}\u{00e3}o async nativa desconhecida: {}",
                    operation
                )
            });

        let expected_arguments = if runtime_function == "task_create_write_file" {
            2
        } else {
            1
        };
        if arguments.len() != expected_arguments {
            panic!(
                "{} requer {} argumento(s), mas recebeu {}",
                operation,
                expected_arguments,
                arguments.len()
            );
        }

        let mut values = Vec::new();
        for argument in arguments {
            let (value, value_type) = self.generate_expressao(argument);
            values.push(self.ensure_string(&value, &value_type));
        }
        let task = self.get_unique_temp_name();
        let signature = if values.len() == 1 {
            format!("i8* {}", values[0])
        } else {
            format!("i8* {}, i8* {}", values[0], values[1])
        };
        self.body.push_str(&format!(
            "  {0} = call %task* @{1}({2})\n",
            task, runtime_function, signature
        ));
        self.await_task_result(&task, &result_type)
    }

    /// Converte o `void*` do runtime no valor esperado pela expressão
    /// aguardada. As operações que produzem bool usam 0/1 como ponteiro
    /// sentinela, portanto a conversão passa por `ptrtoint`.
    pub(crate) fn await_task_result(&mut self, task: &str, result_type: &ast::Tipo) -> (String, ast::Tipo) {
        let raw_result = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {0} = call i8* @task_await(%task* {1})\n",
            raw_result, task
        ));

        match result_type {
            ast::Tipo::Vazio => ("".to_string(), ast::Tipo::Vazio),
            ast::Tipo::Texto | ast::Tipo::Decimal => (raw_result, result_type.clone()),
            ast::Tipo::Booleano => {
                let as_integer = self.get_unique_temp_name();
                let value = self.get_unique_temp_name();
                self.body.push_str(&format!(
                    "  {0} = ptrtoint i8* {1} to i64\n  {2} = trunc i64 {0} to i1\n",
                    as_integer, raw_result, value
                ));
                (value, result_type.clone())
            }
            ast::Tipo::Classe(_) | ast::Tipo::Aplicado { .. } | ast::Tipo::Lista(_) => {
                let typed = self.get_unique_temp_name();
                self.body.push_str(&format!(
                    "  {0} = bitcast i8* {1} to {2}\n",
                    typed,
                    raw_result,
                    self.map_type_to_llvm_arg(result_type)
                ));
                (typed, result_type.clone())
            }
            _ => self.unbox_async_scalar_result(&raw_result, result_type),
        }
    }

    pub(crate) fn unbox_async_scalar_result(
        &mut self,
        raw_result: &str,
        result_type: &ast::Tipo,
    ) -> (String, ast::Tipo) {
        let llvm_type = self.map_type_to_llvm_arg(result_type);
        let pointer = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {0} = bitcast i8* {1} to {2}*\n",
            pointer, raw_result, llvm_type
        ));
        let value = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {0} = load {1}, {1}* {2}\n",
            value, llvm_type, pointer
        ));
        (value, result_type.clone())
    }

    pub(crate) fn find_async_function(&self, fqn: &str) -> Option<ast::DeclaracaoFuncao> {
        for declaration in &self.programa.declaracoes {
            if let ast::Declaracao::DeclaracaoFuncao(function) = declaration {
                if self.type_checker.resolver_nome_funcao(&function.nome, "") == fqn
                    && function.eh_assincrona
                {
                    return Some(function.clone());
                }
            }
        }
        for namespace in &self.programa.namespaces {
            for declaration in &namespace.declaracoes {
                if let ast::Declaracao::DeclaracaoFuncao(function) = declaration {
                    if self
                        .type_checker
                        .resolver_nome_funcao(&function.nome, &namespace.nome)
                        == fqn
                        && function.eh_assincrona
                    {
                        return Some(function.clone());
                    }
                }
            }
        }
        None
    }

    pub(crate) fn ensure_async_wrapper(
        &mut self,
        fqn: &str,
        function: &ast::DeclaracaoFuncao,
        namespace: &str,
    ) -> String {
        if let Some(symbol) = self.async_wrapper_symbols.get(fqn) {
            return symbol.clone();
        }

        let safe_name = fqn.replace('.', "_");
        let args_type = format!("%async.args.{}", safe_name);
        let wrapper_symbol = format!(".async.wrapper.{}", safe_name);
        let param_types: Vec<ast::Tipo> = function
            .parametros
            .iter()
            .map(|parameter| self.resolve_type(&parameter.tipo, namespace))
            .collect();
        let return_type = self.resolve_type(
            &function.tipo_retorno.clone().unwrap_or(ast::Tipo::Vazio),
            namespace,
        );
        let return_llvm = self.map_type_to_llvm_arg(&return_type);

        if !param_types.is_empty() {
            let fields = param_types
                .iter()
                .map(|ty| self.map_type_to_llvm_storage(ty))
                .collect::<Vec<_>>()
                .join(", ");
            self.header
                .push_str(&format!("{} = type {{ {} }}\n", args_type, fields));
        }

        let mut wrapper = String::new();
        wrapper.push_str(&format!(
            "define i8* @\"{}\"(i8* %raw_args) {{\nentry:\n",
            wrapper_symbol
        ));
        let mut call_arguments = Vec::new();
        if !param_types.is_empty() {
            wrapper.push_str(&format!(
                "  %args = bitcast i8* %raw_args to {}*\n",
                args_type
            ));
            for (index, ty) in param_types.iter().enumerate() {
                let llvm_type = self.map_type_to_llvm_arg(ty);
                wrapper.push_str(&format!(
                    "  %arg_ptr.{} = getelementptr inbounds {}, {}* %args, i32 0, i32 {}\n",
                    index, args_type, args_type, index
                ));
                wrapper.push_str(&format!(
                    "  %arg.{} = load {}, {}* %arg_ptr.{}\n",
                    index, llvm_type, llvm_type, index
                ));
                call_arguments.push(format!("{} %arg.{}", llvm_type, index));
            }
        }
        let target_symbol = fqn.replace('.', "_");
        if return_type == ast::Tipo::Vazio {
            wrapper.push_str(&format!(
                "  call void @\"{}\"({})\n  ret i8* null\n}}\n",
                target_symbol,
                call_arguments.join(", ")
            ));
        } else if return_llvm == "i8*" {
            wrapper.push_str(&format!(
                "  %result = call {} @\"{}\"({})\n  ret i8* %result\n}}\n",
                return_llvm,
                target_symbol,
                call_arguments.join(", ")
            ));
        } else if return_llvm.ends_with('*') {
            wrapper.push_str(&format!(
                "  %result = call {} @\"{}\"({})\n  %as_i8 = bitcast {} %result to i8*\n  ret i8* %as_i8\n}}\n",
                return_llvm,
                target_symbol,
                call_arguments.join(", "),
                return_llvm
            ));
        } else {
            wrapper.push_str(&format!(
                "  %result = call {} @\"{}\"({})\n  %size_ptr = getelementptr {}, {}* null, i32 1\n  %size = ptrtoint {}* %size_ptr to i64\n  %memory = call i8* @malloc(i64 %size)\n  %boxed = bitcast i8* %memory to {}*\n  store {} %result, {}* %boxed\n  ret i8* %memory\n}}\n",
                return_llvm,
                target_symbol,
                call_arguments.join(", "),
                return_llvm,
                return_llvm,
                return_llvm,
                return_llvm,
                return_llvm,
                return_llvm
            ));
        }
        self.header.push_str(&wrapper);
        self.async_wrapper_symbols
            .insert(fqn.to_string(), wrapper_symbol.clone());
        wrapper_symbol
    }

    pub(crate) fn await_async_function(
        &mut self,
        fqn: &str,
        arguments: &[ast::Expressao],
    ) -> (String, ast::Tipo) {
        let function = self.find_async_function(fqn).unwrap_or_else(|| {
            panic!(
                "'{}' n\u{00e3}o \u{00e9} uma fun\u{00e7}\u{00e3}o ass\u{00ed}ncrona declarada",
                fqn
            )
        });
        if function.parametros.len() != arguments.len() {
            panic!(
                "Fun\u{00e7}\u{00e3}o ass\u{00ed}ncrona '{}' requer {} argumento(s), mas recebeu {}",
                fqn,
                function.parametros.len(),
                arguments.len()
            );
        }
        let namespace = self.get_namespace_from_fqn(fqn);
        let wrapper_symbol = self.ensure_async_wrapper(fqn, &function, &namespace);
        let parameter_types: Vec<ast::Tipo> = function
            .parametros
            .iter()
            .map(|parameter| self.resolve_type(&parameter.tipo, &namespace))
            .collect();
        let return_type = self.resolve_type(
            &function.tipo_retorno.unwrap_or(ast::Tipo::Vazio),
            &namespace,
        );
        let safe_name = fqn.replace('.', "_");
        let args_type = format!("%async.args.{}", safe_name);

        let raw_args = if parameter_types.is_empty() {
            "null".to_string()
        } else {
            let size_ptr = self.get_unique_temp_name();
            let size = self.get_unique_temp_name();
            let memory = self.get_unique_temp_name();
            let typed_args = self.get_unique_temp_name();
            self.body.push_str(&format!(
                "  {0} = getelementptr {1}, {1}* null, i32 1\n  {2} = ptrtoint {1}* {0} to i64\n  {3} = call i8* @malloc(i64 {2})\n  {4} = bitcast i8* {3} to {1}*\n",
                size_ptr, args_type, size, memory, typed_args
            ));
            for (index, (argument, expected_type)) in
                arguments.iter().zip(parameter_types.iter()).enumerate()
            {
                let (value, actual_type) = self.generate_expressao(argument);
                let coerced = self.ensure_value_type(&value, &actual_type, expected_type);
                let field = self.get_unique_temp_name();
                let llvm_type = self.map_type_to_llvm_storage(expected_type);
                self.body.push_str(&format!(
                    "  {0} = getelementptr inbounds {1}, {1}* {2}, i32 0, i32 {3}\n  store {4} {5}, {4}* {0}\n",
                    field, args_type, typed_args, index, llvm_type, coerced
                ));
            }
            let raw = self.get_unique_temp_name();
            self.body.push_str(&format!(
                "  {0} = bitcast {1}* {2} to i8*\n",
                raw, args_type, typed_args
            ));
            raw
        };
        let id = self.get_unique_temp_name();
        let task = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {0} = call i32 @next_task_id()\n  {1} = call %task* @task_create(i32 {0})\n  call void @task_submit_to_pool(%task* {1}, i8* (i8*)* @\"{2}\", i8* {3})\n",
            id, task, wrapper_symbol, raw_args
        ));
        self.await_task_result(&task, &return_type)
    }

}
