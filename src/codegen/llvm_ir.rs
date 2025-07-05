use crate::ast;
use std::collections::HashMap;

/// O gerador de código para o alvo LLVM IR.
pub struct LlvmGenerator<'a> {
    programa: &'a ast::Programa,
    header: String,
    main_function_body: String,
    string_counter: usize,
    temp_counter: usize,
    variables: HashMap<String, (String, ast::Tipo)>,
}

impl<'a> LlvmGenerator<'a> {
    pub fn new(programa: &'a ast::Programa) -> Self {
        Self {
            programa,
            header: String::new(),
            main_function_body: String::new(),
            string_counter: 0,
            temp_counter: 0,
            variables: HashMap::new(),
        }
    }

    pub fn generate(&mut self) -> String {
        self.prepare_header();
        self.main_function_body.push_str("define i32 @main() {\n");
        self.main_function_body.push_str("entry:\n");

        for declaracao in &self.programa.declaracoes {
            if let ast::Declaracao::Comando(cmd) = declaracao {
                self.generate_comando(cmd);
            }
        }

        self.main_function_body.push_str("  ret i32 0\n");
        self.main_function_body.push_str("}\n");
        format!("{}\n{}", self.header, self.main_function_body)
    }

    fn prepare_header(&mut self) {
        self.header
            .push_str("target triple = \"x86_64-pc-linux-gnu\"\n\n");
        self.header.push_str("declare i32 @printf(i8*, ...)\n");
        self.header.push_str("declare i8* @malloc(i64)\n");
        self.header
            .push_str("declare i32 @sprintf(i8*, i8*, ...)\n");
        self.header.push_str("declare i64 @strlen(i8*)\n\n");
        self.header.push_str(
            "@.println_fmt = private unnamed_addr constant [4 x i8] c\"%s\\0A\\00\", align 1\n",
        );
    }

    fn generate_comando(&mut self, comando: &ast::Comando) {
        match comando {
            ast::Comando::DeclaracaoVar(nome, expr) => {
                let (value_reg, value_type) = self.generate_expressao(expr);
                self.declare_and_store_variable(nome, value_type, value_reg);
            }
            ast::Comando::DeclaracaoVariavel(tipo, nome, Some(expr)) => {
                let (value_reg, _) = self.generate_expressao(expr);
                self.declare_and_store_variable(nome, tipo.clone(), value_reg);
            }
            ast::Comando::Imprima(expr) => {
                let (value_reg, value_type) = self.generate_expressao(expr);
                let final_value_reg = self.ensure_string(value_reg, &value_type);
                self.main_function_body.push_str(&format!(
                    "  call i32 (i8*, ...) @printf(i8* getelementptr inbounds ([4 x i8], [4 x i8]* @.println_fmt, i32 0, i32 0), i8* {}\n",
                    final_value_reg
                ));
            }
            ast::Comando::Bloco(comandos) => {
                for cmd in comandos {
                    self.generate_comando(cmd);
                }
            }
            ast::Comando::AtribuirPropriedade(objeto, propriedade, valor) => {
                let (obj_reg, obj_type) = self.generate_expressao(objeto);
                let (value_reg, value_type) = self.generate_expressao(valor);
                // TODO: Implementar a atribuição de propriedade real
                // Por enquanto, apenas um placeholder para evitar o panic
                self.main_function_body.push_str(&format!("  ; Atribuição de propriedade simulada: {}.{} = {}\n", obj_reg, propriedade, value_reg));
            }
            ast::Comando::ChamarMetodo(objeto_nome, metodo_nome, argumentos) => {
                let (obj_reg, obj_type) = self.load_variable(objeto_nome);
                // TODO: Implementar a chamada de método real, incluindo vtables para polimorfismo
                // Por enquanto, apenas um placeholder para evitar o panic
                self.main_function_body.push_str(&format!("  ; Chamada de método simulada: {}.{}\n", objeto_nome, metodo_nome));
            }
            ast::Comando::Atribuicao(nome, expr) => {
                let (value_reg, value_type) = self.generate_expressao(expr);
                let (ptr_reg, _) = self.variables.get(nome).expect("Variável não declarada");
                match value_type {
                    ast::Tipo::Inteiro => {
                        self.main_function_body.push_str(&format!("  store i32 {}, i32* {}\n", value_reg, ptr_reg));
                    }
                    ast::Tipo::Texto => {
                        self.main_function_body.push_str(&format!("  store i8* {}, i8** {}\n", value_reg, ptr_reg));
                    }
                    ast::Tipo::Classe(_) => {
                        self.main_function_body.push_str(&format!("  store i8* {}, i8** {}\n", value_reg, ptr_reg));
                    }
                    _ => panic!("Tipo de variável não suportado para atribuição: {:?}", value_type),
                }
            }
            ast::Comando::Expressao(expr) => {
                self.generate_expressao(expr);
            }
            ast::Comando::Enquanto(cond, body) => {
                let loop_cond_label = self.get_unique_label("loop.cond");
                let loop_body_label = self.get_unique_label("loop.body");
                let loop_end_label = self.get_unique_label("loop.end");

                self.main_function_body.push_str(&format!("  br label %{}\n", loop_cond_label));
                self.main_function_body.push_str(&format!("{}:\n", loop_cond_label));

                let (cond_reg, _) = self.generate_expressao(cond);
                self.main_function_body.push_str(&format!(
                    "  br i1 {}, label %{}, label %{}\n",
                    cond_reg, loop_body_label, loop_end_label
                ));

                self.main_function_body.push_str(&format!("{}:\n", loop_body_label));
                self.generate_comando(body);
                self.main_function_body.push_str(&format!("  br label %{}\n", loop_cond_label));

                self.main_function_body.push_str(&format!("{}:\n", loop_end_label));
            }
            ast::Comando::Se(cond, then_block, else_block) => {
                let (cond_reg, _) = self.generate_expressao(cond);
                let then_label = self.get_unique_label("then");
                let else_label = self.get_unique_label("else");
                let end_label = self.get_unique_label("end");

                self.main_function_body.push_str(&format!(
                    "  br i1 {}, label %{}, label %{}\n",
                    cond_reg, then_label, else_label
                ));

                self.main_function_body.push_str(&format!("{}:\n", then_label));
                self.generate_comando(then_block);
                self.main_function_body.push_str(&format!("  br label %{}\n", end_label));

                self.main_function_body.push_str(&format!("{}:\n", else_label));
                if let Some(else_cmd) = else_block {
                    self.generate_comando(else_cmd);
                }
                self.main_function_body.push_str(&format!("  br label %{}\n", end_label));

                self.main_function_body.push_str(&format!("{}:\n", end_label));
            }
            _ => panic!("Comando não suportado para geração de LLVM IR: {:?}", comando),
        }
    }

    fn declare_and_store_variable(&mut self, name: &str, var_type: ast::Tipo, value_reg: String) {
        let ptr_reg = format!("%var.{}", name);
        match var_type {
            ast::Tipo::Inteiro => {
                self.main_function_body
                    .push_str(&format!("  {} = alloca i32, align 4\n", ptr_reg));
                self.main_function_body
                    .push_str(&format!("  store i32 {}, i32* {}\n", value_reg, ptr_reg));
            }
            ast::Tipo::Texto => {
                self.main_function_body
                    .push_str(&format!("  {} = alloca i8*, align 8\n", ptr_reg));
                self.main_function_body
                    .push_str(&format!("  store i8* {}, i8** {}\n", value_reg, ptr_reg));
            }
            ast::Tipo::Classe(_) => {
                self.main_function_body
                    .push_str(&format!("  {} = alloca i8*, align 8\n", ptr_reg));
                self.main_function_body
                    .push_str(&format!("  store i8* {}, i8** {}\n", value_reg, ptr_reg));
            }
            _ => panic!("Tipo de variável não suportado: {:?}", var_type),
        }
        self.variables.insert(name.to_string(), (ptr_reg, var_type));
    }

    fn generate_expressao(&mut self, expr: &ast::Expressao) -> (String, ast::Tipo) {
        match expr {
            ast::Expressao::Inteiro(n) => (n.to_string(), ast::Tipo::Inteiro),
            ast::Expressao::Texto(s) => (self.create_global_string(s), ast::Tipo::Texto),
            ast::Expressao::Identificador(name) => self.load_variable(name),
            ast::Expressao::Aritmetica(op, esq, dir) => {
                let (left_reg, left_type) = self.generate_expressao(esq);
                let (right_reg, right_type) = self.generate_expressao(dir);

                match op {
                    ast::OperadorAritmetico::Soma => {
                        if left_type == ast::Tipo::Texto || right_type == ast::Tipo::Texto {
                            let left_str_reg = self.ensure_string(left_reg, &left_type);
                            let right_str_reg = self.ensure_string(right_reg, &right_type);
                            let result_reg = self.concatenate_strings(left_str_reg, right_str_reg);
                            (result_reg, ast::Tipo::Texto)
                        } else if left_type == ast::Tipo::Inteiro && right_type == ast::Tipo::Inteiro {
                            let result_reg = self.get_unique_temp_name();
                            self.main_function_body.push_str(&format!("  {} = add i32 {}, {}\n", result_reg, left_reg, right_reg));
                            (result_reg, ast::Tipo::Inteiro)
                        } else {
                            panic!("Operação de soma não suportada para tipos: {:?} e {:?}", left_type, right_type);
                        }
                    },
                    ast::OperadorAritmetico::Subtracao => {
                        if left_type == ast::Tipo::Inteiro && right_type == ast::Tipo::Inteiro {
                            let result_reg = self.get_unique_temp_name();
                            self.main_function_body.push_str(&format!("  {} = sub i32 {}, {}\n", result_reg, left_reg, right_reg));
                            (result_reg, ast::Tipo::Inteiro)
                        } else {
                            panic!("Operação de subtração não suportada para tipos: {:?} e {:?}", left_type, right_type);
                        }
                    },
                    ast::OperadorAritmetico::Multiplicacao => {
                        if left_type == ast::Tipo::Inteiro && right_type == ast::Tipo::Inteiro {
                            let result_reg = self.get_unique_temp_name();
                            self.main_function_body.push_str(&format!("  {} = mul i32 {}, {}\n", result_reg, left_reg, right_reg));
                            (result_reg, ast::Tipo::Inteiro)
                        }
                        else {
                            panic!("Operação de multiplicação não suportada para tipos: {:?} e {:?}", left_type, right_type);
                        }
                    },
                    ast::OperadorAritmetico::Divisao => {
                        if left_type == ast::Tipo::Inteiro && right_type == ast::Tipo::Inteiro {
                            let result_reg = self.get_unique_temp_name();
                            self.main_function_body.push_str(&format!("  {} = sdiv i32 {}, {}\n", result_reg, left_reg, right_reg));
                            (result_reg, ast::Tipo::Inteiro)
                        } else {
                            panic!("Operação de divisão não suportada para tipos: {:?} e {:?}", left_type, right_type);
                        }
                    },
                    ast::OperadorAritmetico::Modulo => {
                        if left_type == ast::Tipo::Inteiro && right_type == ast::Tipo::Inteiro {
                            let result_reg = self.get_unique_temp_name();
                            self.main_function_body.push_str(&format!("  {} = srem i32 {}, {}\n", result_reg, left_reg, right_reg));
                            (result_reg, ast::Tipo::Inteiro)
                        }
                        else {
                            panic!("Operação de módulo não suportada para tipos: {:?} e {:?}", left_type, right_type);
                        }
                    },
                }
            },
            ast::Expressao::NovoObjeto(nome_classe, _argumentos) => {
                // Implementação básica: alocar memória para o objeto
                // TODO: Chamar construtor e inicializar campos
                let obj_ptr_reg = self.get_unique_temp_name();
                // Para simplificar, vamos alocar um ponteiro nulo por enquanto.
                // Em uma implementação real, você alocaria memória com `malloc` e chamaria o construtor.
                self.main_function_body.push_str(&format!("  {} = inttoptr i64 0 to i8*\n", obj_ptr_reg));
                (obj_ptr_reg, ast::Tipo::Classe(nome_classe.clone()))
            },
            ast::Expressao::Chamada(nome_funcao, argumentos) => {
                let mut arg_regs = Vec::new();
                let mut arg_types = Vec::new();
                for arg in argumentos {
                    let (arg_reg, arg_type) = self.generate_expressao(arg);
                    arg_regs.push(arg_reg);
                    arg_types.push(self.map_type_to_llvm(&arg_type));
                }
                let args_str = arg_regs.join(", ");
                let arg_types_str = arg_types.join(", ");
                let result_reg = self.get_unique_temp_name();
                self.main_function_body.push_str(&format!("  {} = call i32 @{}({})
", result_reg, nome_funcao, args_str));
                (result_reg, ast::Tipo::Inteiro) // Assuming i32 return for now
            },
            ast::Expressao::Comparacao(op, esq, dir) => {
                let (left_reg, left_type) = self.generate_expressao(esq);
                let (right_reg, right_type) = self.generate_expressao(dir);

                if left_type != ast::Tipo::Inteiro || right_type != ast::Tipo::Inteiro {
                    panic!("A comparação só é suportada para inteiros por enquanto.");
                }

                let op_str = match op {
                    ast::OperadorComparacao::Igual => "eq",
                    ast::OperadorComparacao::Diferente => "ne",
                    ast::OperadorComparacao::Menor => "slt",
                    ast::OperadorComparacao::MaiorQue => "sgt",
                    ast::OperadorComparacao::MenorIgual => "sle",
                    ast::OperadorComparacao::MaiorIgual => "sge",
                };

                let result_reg = self.get_unique_temp_name();
                self.main_function_body.push_str(&format!(
                    "  {} = icmp {} i32 {}, {}\n",
                    result_reg, op_str, left_reg, right_reg
                ));
                (result_reg, ast::Tipo::Booleano)
            },
            _ => panic!("Expressão não suportada: {:?}", expr),
        }
    }

    fn load_variable(&mut self, name: &str) -> (String, ast::Tipo) {
        let (ptr_reg, var_type) = if let Some(data) = self.variables.get(name) {
            data.clone()
        } else {
            panic!("Variável não declarada: {}", name);
        };

        let loaded_reg = self.get_unique_temp_name();

        match var_type {
            ast::Tipo::Inteiro => {
                self.main_function_body
                    .push_str(&format!("  {} = load i32, i32* {}\n", loaded_reg, &ptr_reg));
            }
            ast::Tipo::Texto => {
                self.main_function_body
                    .push_str(&format!("  {} = load i8*, i8** {}\n", loaded_reg, &ptr_reg));
            }
            ast::Tipo::Classe(_) => {
                self.main_function_body
                    .push_str(&format!("  {} = load i8*, i8** {}\n", loaded_reg, &ptr_reg));
            }
            ast::Tipo::Classe(_) => {
                self.main_function_body
                    .push_str(&format!("  {} = load i8*, i8** {}\n", loaded_reg, &ptr_reg));
            }
            ast::Tipo::Classe(_) => {
                self.main_function_body
                    .push_str(&format!("  {} = load i8*, i8** {}\n", loaded_reg, &ptr_reg));
            }
            _ => panic!("Tipo de variável não suportado para carregamento: {:?}", var_type),
        }
        (loaded_reg, var_type)
    }

    fn ensure_string(&mut self, reg: String, tipo: &ast::Tipo) -> String {
        match tipo {
            ast::Tipo::Texto => reg,
            ast::Tipo::Inteiro => self.convert_int_to_string(reg),
            _ => panic!("Não é possível converter {:?} para string", tipo),
        }
    }

    fn convert_int_to_string(&mut self, int_reg: String) -> String {
        let format_specifier = self.create_global_string("%d");
        let buffer = self.get_unique_temp_name();
        self.main_function_body
            .push_str(&format!("  {} = alloca [21 x i8], align 1\n", buffer));
        let buffer_ptr = self.get_unique_temp_name();
        self.main_function_body.push_str(&format!(
            "  {} = getelementptr inbounds [21 x i8], [21 x i8]* {}, i32 0, i32 0\n",
            buffer_ptr, buffer
        ));
        self.main_function_body.push_str(&format!(
            "  call i32 (i8*, i8*, ...) @sprintf(i8* {}, i8* {}, i32 {}\n",
            buffer_ptr, format_specifier, int_reg
        ));
        buffer_ptr
    }

    fn concatenate_strings(&mut self, str1_reg: String, str2_reg: String) -> String {
        let format_specifier = self.create_global_string("%s%s");
        let len1_reg = self.get_unique_temp_name();
        self.main_function_body.push_str(&format!(
            "  {} = call i64 @strlen(i8* {}\n",
            len1_reg, &str1_reg
        ));
        let len2_reg = self.get_unique_temp_name();
        self.main_function_body.push_str(&format!(
            "  {} = call i64 @strlen(i8* {}\n",
            len2_reg, &str2_reg
        ));
        let total_len_reg = self.get_unique_temp_name();
        self.main_function_body.push_str(&format!(
            "  {} = add i64 {}, {}\n",
            total_len_reg, len1_reg, len2_reg
        ));
        let alloc_size_reg = self.get_unique_temp_name();
        self.main_function_body.push_str(&format!(
            "  {} = add i64 {}, 1\n",
            alloc_size_reg, total_len_reg
        ));
        let buffer_reg = self.get_unique_temp_name();
        self.main_function_body.push_str(&format!(
            "  {} = call i8* @malloc(i64 {}\n",
            buffer_reg, alloc_size_reg
        ));
        self.main_function_body.push_str(&format!(
            "  call i32 (i8*, i8*, ...) @sprintf(i8* {}, i8* {}, i8* {}, i8* {}\n",
            buffer_reg, format_specifier, &str1_reg, &str2_reg
        ));
        buffer_reg
    }

    fn create_global_string(&mut self, text: &str) -> String {
        let str_len = text.len() + 1;
        let str_name = format!("@.str.{}", self.string_counter);
        self.string_counter += 1;
        let sanitized_text = text.replace('\n', "\\0A");
        self.header.push_str(&format!(
            "{} = private unnamed_addr constant [{} x i8] c\"{}\\00\", align 1\n",
            str_name, str_len, sanitized_text
        ));
        let ptr_register = self.get_unique_temp_name();
        self.main_function_body.push_str(&format!(
            "  {} = getelementptr inbounds [{} x i8], [{} x i8]* {}, i32 0, i32 0\n",
            ptr_register, str_len, str_len, str_name
        ));
        ptr_register
    }

    fn get_unique_temp_name(&mut self) -> String {
        let name = format!("%tmp.{}", self.temp_counter);
        self.temp_counter += 1;
        name
    }

    fn get_unique_label(&mut self, prefix: &str) -> String {
        let label = format!("{}.{}", prefix, self.temp_counter);
        self.temp_counter += 1;
        label
    }

    fn map_type_to_llvm(&self, tipo: &ast::Tipo) -> String {
        match tipo {
            ast::Tipo::Inteiro => "i32".to_string(),
            ast::Tipo::Texto => "i8*".to_string(),
            ast::Tipo::Booleano => "i1".to_string(),
            ast::Tipo::Vazio => "void".to_string(),
            ast::Tipo::Classe(_) => "i8*".to_string(), // Generic pointer for classes
            _ => panic!("Tipo LLVM não mapeado: {:?}", tipo),
        }
    }
}
