// src/codegen.rs

use crate::ast;
use std::collections::HashMap;
use std::fs;

#[derive(Debug, Clone)]
pub enum BackendType {
    Llvm,
    CilBytecode,
    Console,
    Bytecode,
}

//_______________________________________________________________________________________________
//
//  LLVM DYNAMIC GENERATOR
//_______________________________________________________________________________________________

struct LlvmGenerator<'a> {
    programa: &'a ast::Programa,
    header: String,
    main_function_body: String,
    string_counter: usize,
    temp_counter: usize,
    variables: HashMap<String, (String, ast::Tipo)>,
}

impl<'a> LlvmGenerator<'a> {
    fn new(programa: &'a ast::Programa) -> Self {
        Self {
            programa,
            header: String::new(),
            main_function_body: String::new(),
            string_counter: 0,
            temp_counter: 0,
            variables: HashMap::new(),
        }
    }

    fn generate(&mut self) -> String {
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
        self.header.push_str("target triple = \"x86_64-pc-linux-gnu\"\n\n");
        self.header.push_str("declare i32 @printf(i8*, ...)\n");
        self.header.push_str("declare i8* @malloc(i64)\n");
        self.header.push_str("declare i32 @sprintf(i8*, i8*, ...)\n");
        self.header.push_str("declare i64 @strlen(i8*)\n\n");
        
        // ✅ CORREÇÃO: Adiciona uma string de formato global para `imprima` que inclui uma quebra de linha.
        self.header.push_str("@.println_fmt = private unnamed_addr constant [4 x i8] c\"%s\\0A\\00\", align 1\n");
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

                // ✅ CORREÇÃO: Usa a string de formato com quebra de linha (@.println_fmt) para imprimir.
                self.main_function_body.push_str(&format!(
                    "  call i32 (i8*, ...) @printf(i8* getelementptr inbounds ([4 x i8], [4 x i8]* @.println_fmt, i32 0, i32 0), i8* {})\n",
                    final_value_reg
                ));
            }
            _ => {
                self.main_function_body.push_str(&format!(
                    "  ; Comando {:?} não implementado\n",
                    comando
                ));
            }
        }
    }
    
    fn declare_and_store_variable(&mut self, name: &str, var_type: ast::Tipo, value_reg: String) {
        let ptr_reg = format!("%var.{}", name);
        match var_type {
            ast::Tipo::Inteiro => {
                self.main_function_body.push_str(&format!("  {} = alloca i32, align 4\n", ptr_reg));
                self.main_function_body.push_str(&format!("  store i32 {}, i32* {}\n", value_reg, ptr_reg));
            }
            ast::Tipo::Texto => {
                self.main_function_body.push_str(&format!("  {} = alloca i8*, align 8\n", ptr_reg));
                self.main_function_body.push_str(&format!("  store i8* {}, i8** {}\n", value_reg, ptr_reg));
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
            ast::Expressao::Aritmetica(ast::OperadorAritmetico::Soma, esq, dir) => {
                let (left_reg, left_type) = self.generate_expressao(esq);
                let (right_reg, right_type) = self.generate_expressao(dir);
                let left_str_reg = self.ensure_string(left_reg, &left_type);
                let right_str_reg = self.ensure_string(right_reg, &right_type);
                let result_reg = self.concatenate_strings(left_str_reg, right_str_reg);
                (result_reg, ast::Tipo::Texto)
            }
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
                self.main_function_body.push_str(&format!("  {} = load i32, i32* {}\n", loaded_reg, &ptr_reg));
            }
            ast::Tipo::Texto => {
                self.main_function_body.push_str(&format!("  {} = load i8*, i8** {}\n", loaded_reg, &ptr_reg));
            }
            _ => {}
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
        self.main_function_body.push_str(&format!("  {} = alloca [21 x i8], align 1\n", buffer));
        let buffer_ptr = self.get_unique_temp_name();
        self.main_function_body.push_str(&format!("  {} = getelementptr inbounds [21 x i8], [21 x i8]* {}, i32 0, i32 0\n", buffer_ptr, buffer));
        self.main_function_body.push_str(&format!(
            "  call i32 (i8*, i8*, ...) @sprintf(i8* {}, i8* {}, i32 {})\n",
            buffer_ptr, format_specifier, int_reg
        ));
        buffer_ptr
    }

    fn concatenate_strings(&mut self, str1_reg: String, str2_reg: String) -> String {
        let format_specifier = self.create_global_string("%s%s");
        let len1_reg = self.get_unique_temp_name();
        self.main_function_body.push_str(&format!("  {} = call i64 @strlen(i8* {})\n", len1_reg, &str1_reg));
        let len2_reg = self.get_unique_temp_name();
        self.main_function_body.push_str(&format!("  {} = call i64 @strlen(i8* {})\n", len2_reg, &str2_reg));
        let total_len_reg = self.get_unique_temp_name();
        self.main_function_body.push_str(&format!("  {} = add i64 {}, {}\n", total_len_reg, len1_reg, len2_reg));
        let alloc_size_reg = self.get_unique_temp_name();
        self.main_function_body.push_str(&format!("  {} = add i64 {}, 1\n", alloc_size_reg, total_len_reg));
        let buffer_reg = self.get_unique_temp_name();
        self.main_function_body.push_str(&format!("  {} = call i8* @malloc(i64 {})\n", buffer_reg, alloc_size_reg));
        self.main_function_body.push_str(&format!(
            "  call i32 (i8*, i8*, ...) @sprintf(i8* {}, i8* {}, i8* {}, i8* {})\n",
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
}

//_______________________________________________________________________________________________
//
//  PUBLIC API (GeradorCodigo)
//_______________________________________________________________________________________________

pub struct GeradorCodigo;

impl GeradorCodigo {
    pub fn new() -> Result<Self, String> { Ok(Self) }
    pub fn new_llvm() -> Result<Self, String> { Self::new() }
    pub fn new_cil_bytecode() -> Result<Self, String> { Self::new() }
    pub fn new_console() -> Result<Self, String> { Self::new() }
    pub fn new_bytecode() -> Result<Self, String> { Self::new() }

    pub fn gerar_llvm_ir_dinamico(&self, programa: &ast::Programa, nome_base: &str) -> Result<(), String> {
        let mut generator = LlvmGenerator::new(programa);
        let llvm_ir_code = generator.generate();
        let output_file = format!("{}.ll", nome_base);
        fs::write(&output_file, llvm_ir_code)
            .map_err(|e| format!("Erro ao escrever LLVM IR dinâmico: {}", e))?;
        Ok(())
    }
    
    pub fn gerar_programa(&self, _programa: &ast::Programa) -> Result<(), String> {
        Ok(())
    }

    pub fn obter_bytecode(&self) -> Vec<String> {
        vec!["HALT".to_string()]
    }
    pub fn obter_bytecode_cil(&self) -> Vec<String> {
        vec!["CIL_RET".to_string()]
    }
    pub fn gerar_cil_do_bytecode_cil(&self, _bytecode: &[String], _nome_base: &str) -> Result<(), String> {
        Ok(())
    }
    pub fn gerar_projeto_console(&self, _programa: &ast::Programa) -> Result<String, String> {
        Ok(String::new())
    }
}