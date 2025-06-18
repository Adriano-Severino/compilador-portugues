// src/codegen.rs

use crate::ast;
use std::collections::HashMap;
use std::fs;

//_______________________________________________________________________________________________
//
//  ARQUITETURA DE BACKENDS
//_______________________________________________________________________________________________
//
//  Cada gerador de backend é uma struct separada.
//  Isso garante que a lógica de cada alvo seja independente.
//

/// O gerador de código para o alvo LLVM IR.
struct LlvmGenerator<'a> {
    programa: &'a ast::Programa,
    header: String,
    main_function_body: String,
    string_counter: usize,
    temp_counter: usize,
    variables: HashMap<String, (String, ast::Tipo)>,
}

/// O gerador de código para o alvo CIL (Common Intermediate Language) do .NET.
struct CilGenerator<'a> {
    programa: &'a ast::Programa,
    assembly_name: String,
}

/// O gerador de código para o alvo Console Application em C#.
struct ConsoleGenerator<'a> {
    programa: &'a ast::Programa,
}

/// O gerador de código para o seu formato de bytecode customizado.
struct BytecodeGenerator<'a> {
    programa: &'a ast::Programa,
}


// --- IMPLEMENTAÇÃO DO GERADOR LLVM (Existente e Funcional) ---
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
                self.main_function_body.push_str(&format!(
                    "  call i32 (i8*, ...) @printf(i8* getelementptr inbounds ([4 x i8], [4 x i8]* @.println_fmt, i32 0, i32 0), i8* {})\n",
                    final_value_reg
                ));
            }
            _ => {}
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

// --- IMPLEMENTAÇÃO DO GERADOR CIL ---
impl<'a> CilGenerator<'a> {
    fn new(programa: &'a ast::Programa, assembly_name: String) -> Self {
        Self { programa, assembly_name }
    }

    fn generate(&self) -> String {
        let mut code = String::new();
        code.push_str(&format!(".assembly extern mscorlib {{}}\n"));
        code.push_str(&format!(".assembly {} {{}}\n\n", self.assembly_name));
        code.push_str(".class private auto ansi beforefieldinit Principal extends [mscorlib]System.Object\n{\n");
        code.push_str("  .method public hidebysig static void Main() cil managed\n  {\n");
        code.push_str("    .entrypoint\n");
        code.push_str("    .maxstack  8\n");

        for declaracao in &self.programa.declaracoes {
            if let ast::Declaracao::Comando(cmd) = declaracao {
                code.push_str(&self.generate_comando(cmd));
            }
        }

        code.push_str("    ret\n");
        code.push_str("  }\n");
        code.push_str("  .method public hidebysig specialname rtspecialname instance void .ctor() cil managed { ret }\n");
        code.push_str("}\n");
        code
    }

    fn generate_comando(&self, comando: &ast::Comando) -> String {
        match comando {
            ast::Comando::Imprima(expr) => self.generate_expressao(expr),
            _ => format!("    // Comando {:?} não implementado para CIL\n", comando),
        }
    }
    
    fn generate_expressao(&self, expr: &ast::Expressao) -> String {
        let mut code = String::new();
        match expr {
            ast::Expressao::Texto(s) => {
                code.push_str(&format!("    ldstr \"{}\\n\"\n", s.replace('\n', "")));
                code.push_str("    call void [mscorlib]System.Console::WriteLine(string)\n");
            }
            ast::Expressao::Inteiro(n) => {
                code.push_str(&format!("    ldc.i4 {}\n", n));
                code.push_str("    call void [mscorlib]System.Console::WriteLine(int32)\n");
            }
            ast::Expressao::Aritmetica(ast::OperadorAritmetico::Soma, _, _) => {
                let parts = self.flatten_soma(expr);
                for part in parts {
                    match part {
                        ast::Expressao::Texto(s) => code.push_str(&format!("    ldstr \"{}\"\n    call void [mscorlib]System.Console::Write(string)\n", s)),
                        ast::Expressao::Inteiro(n) => code.push_str(&format!("    ldc.i4 {}\n    call void [mscorlib]System.Console::Write(int32)\n", n)),
                        _ => {}
                    }
                }
                code.push_str("    call void [mscorlib]System.Console::WriteLine()\n");
            }
            _ => code.push_str(&format!("    // Expressão {:?} não implementada para CIL\n", expr)),
        }
        code
    }
    
    fn flatten_soma(&self, expr: &'a ast::Expressao) -> Vec<&'a ast::Expressao> {
        let mut parts = Vec::new();
        let mut stack = vec![expr];
        while let Some(e) = stack.pop() {
            if let ast::Expressao::Aritmetica(ast::OperadorAritmetico::Soma, esq, dir) = e {
                stack.push(dir);
                stack.push(esq);
            } else {
                parts.push(e);
            }
        }
        parts
    }
}

// --- IMPLEMENTAÇÃO DO GERADOR DE CONSOLE C# ---
impl<'a> ConsoleGenerator<'a> {
    fn new(programa: &'a ast::Programa) -> Self {
        Self { programa }
    }

    fn generate(&self) -> String {
        let mut code = String::new();
        for declaracao in &self.programa.declaracoes {
            if let ast::Declaracao::Comando(cmd) = declaracao {
                code.push_str(&self.generate_comando(cmd, 4));
            }
        }
        code
    }

    fn generate_comando(&self, comando: &ast::Comando, indent: usize) -> String {
        let prefix = " ".repeat(indent);
        match comando {
            ast::Comando::DeclaracaoVariavel(tipo, nome, Some(expr)) => {
                format!("{}{} {} = {};\n", prefix, self.map_type(tipo), nome, self.generate_expressao(expr))
            }
            ast::Comando::DeclaracaoVar(nome, expr) => {
                format!("{}var {} = {};\n", prefix, nome, self.generate_expressao(expr))
            }
            ast::Comando::Imprima(expr) => {
                format!("{}Console.WriteLine({});\n", prefix, self.generate_expressao(expr))
            }
            _ => format!("{}// Comando {:?} não implementado para Console\n", prefix, comando),
        }
    }

    fn generate_expressao(&self, expr: &ast::Expressao) -> String {
        match expr {
            ast::Expressao::Texto(s) => format!("\"{}\"", s),
            ast::Expressao::Inteiro(n) => n.to_string(),
            ast::Expressao::Identificador(name) => name.clone(),
            ast::Expressao::Aritmetica(ast::OperadorAritmetico::Soma, esq, dir) => {
                format!("{} + {}", self.generate_expressao(esq), self.generate_expressao(dir))
            }
            _ => format!("\"ERRO: Expressao {:?} nao suportada\"", expr),
        }
    }
    
    fn map_type(&self, tipo: &ast::Tipo) -> &str {
        match tipo {
            ast::Tipo::Inteiro => "int",
            ast::Tipo::Texto => "string",
            ast::Tipo::Booleano => "bool",
            _ => "object",
        }
    }
}

// --- IMPLEMENTAÇÃO DO GERADOR DE BYTECODE ---
impl<'a> BytecodeGenerator<'a> {
    fn new(programa: &'a ast::Programa) -> Self {
        Self { programa }
    }

    fn generate(&self) -> Vec<String> {
        let mut bytecode = Vec::new();
        for declaracao in &self.programa.declaracoes {
            if let ast::Declaracao::Comando(cmd) = declaracao {
                bytecode.extend(self.generate_comando(cmd));
            }
        }
        bytecode.push("HALT".to_string());
        bytecode
    }

    fn generate_comando(&self, comando: &ast::Comando) -> Vec<String> {
        match comando {
            ast::Comando::DeclaracaoVar(nome, expr) => {
                let mut instructions = self.generate_expressao(expr);
                instructions.push(format!("STORE_VAR {}", nome));
                instructions
            }
            ast::Comando::DeclaracaoVariavel(_, nome, Some(expr)) => {
                let mut instructions = self.generate_expressao(expr);
                instructions.push(format!("STORE_VAR {}", nome));
                instructions
            }
            ast::Comando::Imprima(expr) => {
                let mut instructions = self.generate_expressao(expr);
                instructions.push("PRINT".to_string());
                instructions
            }
            _ => vec![format!("; Comando {:?} não implementado para bytecode", comando)],
        }
    }
    
    fn generate_expressao(&self, expr: &ast::Expressao) -> Vec<String> {
        match expr {
            ast::Expressao::Texto(s) => vec![format!("LOAD_CONST_STR \"{}\"", s)],
            ast::Expressao::Inteiro(n) => vec![format!("LOAD_CONST_INT {}", n)],
            ast::Expressao::Identificador(nome) => vec![format!("LOAD_VAR {}", nome)],
            ast::Expressao::Aritmetica(ast::OperadorAritmetico::Soma, esq, dir) => {
                let mut instructions = self.generate_expressao(esq);
                instructions.extend(self.generate_expressao(dir));
                instructions.push("CONCAT 2".to_string());
                instructions
            }
            _ => vec![format!("; Expressão {:?} não implementada para bytecode", expr)],
        }
    }
}


//_______________________________________________________________________________________________
//
//  API PÚBLICA (GeradorCodigo)
//_______________________________________________________________________________________________
pub struct GeradorCodigo;

impl GeradorCodigo {
    pub fn new() -> Result<Self, String> {
        Ok(Self)
    }

    pub fn gerar_llvm_ir(&self, programa: &ast::Programa, nome_base: &str) -> Result<(), String> {
        let mut generator = LlvmGenerator::new(programa);
        let code = generator.generate();
        fs::write(format!("{}.ll", nome_base), code).map_err(|e| e.to_string())
    }

    pub fn gerar_cil(&self, programa: &ast::Programa, nome_base: &str) -> Result<(), String> {
        let generator = CilGenerator::new(programa, nome_base.to_string());
        let code = generator.generate();
        fs::write(format!("{}.il", nome_base), code).map_err(|e| e.to_string())
    }

    pub fn gerar_console(&self, programa: &ast::Programa, nome_base: &str) -> Result<(), String> {
        let generator = ConsoleGenerator::new(programa);
        let main_body = generator.generate();

        let dir_projeto = format!("./{}", nome_base);
        fs::create_dir_all(&dir_projeto).map_err(|e| e.to_string())?;

        let csproj = format!(
            r#"<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <TargetFramework>net8.0</TargetFramework>
    <ImplicitUsings>enable</ImplicitUsings>
    <Nullable>enable</Nullable>
  </PropertyGroup>
</Project>"#
        );
        fs::write(format!("{}/{}.csproj", dir_projeto, nome_base), csproj).map_err(|e| e.to_string())?;

        let program_cs = format!(
            r#"namespace {}
{{
    class Program
    {{
        static void Main(string[] args)
        {{
{}
        }}
    }}
}}"#, nome_base, main_body);
        fs::write(format!("{}/Program.cs", dir_projeto), program_cs).map_err(|e| e.to_string())
    }

    pub fn gerar_bytecode(&self, programa: &ast::Programa, nome_base: &str) -> Result<(), String> {
        let generator = BytecodeGenerator::new(programa);
        let bytecode = generator.generate();
        fs::write(format!("{}.pbc", nome_base), bytecode.join("\n")).map_err(|e| e.to_string())
    }
}