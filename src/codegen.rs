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

// --- IMPLEMENTAÇÃO DO GERADOR LLVM (Existente e Funcional) ---
/// O gerador de código para o alvo LLVM IR.
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
                self.main_function_body
                    .push_str(&format!("  {} = load i32, i32* {}\n", loaded_reg, &ptr_reg));
            }
            ast::Tipo::Texto => {
                self.main_function_body
                    .push_str(&format!("  {} = load i8*, i8** {}\n", loaded_reg, &ptr_reg));
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
        self.main_function_body
            .push_str(&format!("  {} = alloca [21 x i8], align 1\n", buffer));
        let buffer_ptr = self.get_unique_temp_name();
        self.main_function_body.push_str(&format!(
            "  {} = getelementptr inbounds [21 x i8], [21 x i8]* {}, i32 0, i32 0\n",
            buffer_ptr, buffer
        ));
        self.main_function_body.push_str(&format!(
            "  call i32 (i8*, i8*, ...) @sprintf(i8* {}, i8* {}, i32 {})\n",
            buffer_ptr, format_specifier, int_reg
        ));
        buffer_ptr
    }

    fn concatenate_strings(&mut self, str1_reg: String, str2_reg: String) -> String {
        let format_specifier = self.create_global_string("%s%s");
        let len1_reg = self.get_unique_temp_name();
        self.main_function_body.push_str(&format!(
            "  {} = call i64 @strlen(i8* {})\n",
            len1_reg, &str1_reg
        ));
        let len2_reg = self.get_unique_temp_name();
        self.main_function_body.push_str(&format!(
            "  {} = call i64 @strlen(i8* {})\n",
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
            "  {} = call i8* @malloc(i64 {})\n",
            buffer_reg, alloc_size_reg
        ));
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
/// O gerador de código para o alvo CIL (Common Intermediate Language) do .NET.
struct CilGenerator<'a> {
    programa: &'a ast::Programa,
    assembly_name: String,
}

impl<'a> CilGenerator<'a> {
    fn new(programa: &'a ast::Programa, assembly_name: String) -> Self {
        Self {
            programa,
            assembly_name,
        }
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
            _ => code.push_str(&format!(
                "    // Expressão {:?} não implementada para CIL\n",
                expr
            )),
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
/// O gerador de código para o alvo Console Application em C#.
struct ConsoleGenerator<'a> {
    programa: &'a ast::Programa,
}

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
                format!(
                    "{}{} {} = {};\n",
                    prefix,
                    self.map_type(tipo),
                    nome,
                    self.generate_expressao(expr)
                )
            }
            ast::Comando::DeclaracaoVar(nome, expr) => {
                format!(
                    "{}var {} = {};\n",
                    prefix,
                    nome,
                    self.generate_expressao(expr)
                )
            }
            ast::Comando::Imprima(expr) => {
                format!(
                    "{}Console.WriteLine({});\n",
                    prefix,
                    self.generate_expressao(expr)
                )
            }
            _ => format!(
                "{}// Comando {:?} não implementado para Console\n",
                prefix, comando
            ),
        }
    }

    fn generate_expressao(&self, expr: &ast::Expressao) -> String {
        match expr {
            ast::Expressao::Texto(s) => format!("\"{}\"", s),
            ast::Expressao::Inteiro(n) => n.to_string(),
            ast::Expressao::Identificador(name) => name.clone(),
            ast::Expressao::Aritmetica(ast::OperadorAritmetico::Soma, esq, dir) => {
                format!(
                    "{} + {}",
                    self.generate_expressao(esq),
                    self.generate_expressao(dir)
                )
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
struct BytecodeGenerator<'a> {
    programa: &'a ast::Programa,
    bytecode_instructions: Vec<String>, //Armazena as instruções geradas
    em_metodo: bool,
}

impl<'a> BytecodeGenerator<'a> {
    fn new(programa: &'a ast::Programa, em_metodo: bool) -> Self {
        Self {
            programa,
            bytecode_instructions: Vec::new(),
            em_metodo,
        }
    }

    fn is_string_expr(expr: &ast::Expressao) -> bool {
        use ast::{Expressao as E, OperadorAritmetico as OA};
        match expr {
            E::Texto(_) | E::StringInterpolada(_) => true,
            E::Aritmetica(OA::Soma, l, r) => Self::is_string_expr(l) || Self::is_string_expr(r),
            _ => false,
        }
    }
    // ADICIONE ESTA NOVA FUNÇÃO:
    fn generate_declaracao(&mut self, declaracao: &ast::Declaracao) {
        match declaracao {
            // ✅ Reconhece e processa a declaração de classe
            ast::Declaracao::DeclaracaoClasse(classe_def) => {
                // ------------- 1. coleta as propriedades (campos + props) -------------
                let propriedades: Vec<String> = classe_def
                    .propriedades
                    .iter()
                    .map(|p| p.nome.clone())
                    .chain(classe_def.campos.iter().map(|c| c.nome.clone()))
                    .collect();
                let props_str = propriedades.join(" ");

                // ------------- 2. DEFINE_CLASS vem PRIMEIRO ---------------------------
                self.bytecode_instructions
                    .push(format!("DEFINE_CLASS {} {}", classe_def.nome, props_str));

                // ------------- 3. gera cada método como bloco independente ------------
                for metodo in &classe_def.metodos {
                    // a) AST temporário que vive até o fim do loop
                    let sub_programa = ast::Programa {
                        namespaces: vec![],
                        declaracoes: vec![ast::Declaracao::Comando(ast::Comando::Bloco(
                            metodo.corpo.clone(),
                        ))],
                    };

                    // b) gera bytecode do corpo do método
                    let mut sub = BytecodeGenerator::new(&sub_programa, true);
                    let mut corpo = sub.generate(); // inclui HALT
                    corpo.pop(); // remove HALT final

                    // c) cabeçalho + corpo
                    self.bytecode_instructions.push(format!(
                        "DEFINE_METHOD {} {} {}",
                        classe_def.nome,
                        metodo.nome,
                        corpo.len()
                    ));
                    self.bytecode_instructions.extend(corpo);
                }
            }

            // Mantém o comportamento para comandos
            ast::Declaracao::Comando(cmd) => {
                self.generate_comando(cmd);
            }

            // Ignora outras declarações por enquanto
            _ => { /* Não faz nada para funções, módulos, etc. ainda */ }
        }
    }

    fn generate(&mut self) -> Vec<String> {
        // Itera sobre as declarações no nível raiz do programa
        for declaracao in &self.programa.declaracoes {
            self.generate_declaracao(declaracao);
        }

        // ✅ CORREÇÃO: Adicione este loop para iterar sobre os namespaces
        for namespace in &self.programa.namespaces {
            for declaracao in &namespace.declaracoes {
                self.generate_declaracao(declaracao);
            }
        }

        self.bytecode_instructions.push("HALT".to_string());
        std::mem::take(&mut self.bytecode_instructions)
    }
    // Altera a assinatura para `&mut self` e remove o retorno Vec<String>
    fn generate_comando(&mut self, comando: &ast::Comando) {
        match comando {
            ast::Comando::DeclaracaoVar(nome, expr) => {
                self.generate_expressao(expr); // Gera expressão e adiciona à lista interna
                self.bytecode_instructions
                    .push(format!("STORE_VAR {}", nome));
            }
            ast::Comando::DeclaracaoVariavel(_, nome, Some(expr)) => {
                self.generate_expressao(expr);
                self.bytecode_instructions
                    .push(format!("STORE_VAR {}", nome));
            }
            ast::Comando::Atribuicao(nome, expr) => {
                // Adicionado: Atribuição
                self.generate_expressao(expr);
                self.bytecode_instructions
                    .push(format!("STORE_VAR {}", nome));
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

            // Adicionado: Comando 'se'
            ast::Comando::Se(condicao, bloco_if, bloco_else) => {
                self.generate_expressao(condicao); // Gera código para a condição

                let jump_if_false_placeholder_ip = self.bytecode_instructions.len();
                self.bytecode_instructions
                    .push("JUMP_IF_FALSE 0".to_string()); // Placeholder para o salto

                self.generate_comando(bloco_if); // Gera código para o bloco 'se'

                if let Some(bloco_else) = bloco_else {
                    let jump_to_end_placeholder_ip = self.bytecode_instructions.len();
                    self.bytecode_instructions.push("JUMP 0".to_string()); // Salta sobre o bloco 'senão'

                    let else_start_ip = self.bytecode_instructions.len();
                    // Patching: Se houver 'senão', o JUMP_IF_FALSE salta para o início do bloco 'senão'
                    self.bytecode_instructions[jump_if_false_placeholder_ip] =
                        format!("JUMP_IF_FALSE {}", else_start_ip);

                    self.generate_comando(bloco_else); // Gera código para o bloco 'senão'

                    let end_if_else_ip = self.bytecode_instructions.len();
                    // Patching: O JUMP sobre o bloco 'senão' salta para o final de tudo
                    self.bytecode_instructions[jump_to_end_placeholder_ip] =
                        format!("JUMP {}", end_if_else_ip);
                } else {
                    let end_if_ip = self.bytecode_instructions.len();
                    // Patching: Se não houver 'senão', o JUMP_IF_FALSE salta para o final do comando 'se'
                    self.bytecode_instructions[jump_if_false_placeholder_ip] =
                        format!("JUMP_IF_FALSE {}", end_if_ip);
                }
            }

            ast::Comando::CriarObjeto(var_nome, classe, argumentos) => {
                // Gerar argumentos
                for arg in argumentos {
                    self.generate_expressao(arg);
                }

                // Criar objeto
                self.bytecode_instructions.push(format!(
                    "NEW_OBJECT {} {}",
                    classe,
                    argumentos.len()
                ));
                self.bytecode_instructions
                    .push(format!("STORE_VAR {}", var_nome));
            }

            ast::Comando::AtribuirPropriedade(objeto_nome, prop_nome, expr) => {
                // 1. Gera o valor a ser atribuído e o coloca na pilha.
                self.generate_expressao(expr);
                // 2. Carrega a instância do objeto na pilha.
                self.bytecode_instructions
                    .push(format!("LOAD_VAR {}", objeto_nome));
                // 3. Emite a nova instrução para definir a propriedade.
                self.bytecode_instructions
                    .push(format!("SET_PROPERTY {}", prop_nome));

                self.bytecode_instructions
                    .push(format!("STORE_VAR {}", objeto_nome));
            }

            ast::Comando::ChamarMetodo(objeto_nome, metodo, argumentos) => {
                self.bytecode_instructions
                    .push(format!("LOAD_VAR {}", objeto_nome));

                for arg in argumentos {
                    self.generate_expressao(arg);
                }

                self.bytecode_instructions.push(format!(
                    "CALL_METHOD {} {}",
                    metodo,
                    argumentos.len()
                ));
                self.bytecode_instructions.push("POP".to_string()); // Descartar resultado se não usado
            }

            // dentro de generate_comando
            ast::Comando::AtribuirPropriedade(objeto_nome, prop_nome, expr) => {
                // 1. empilha o valor
                self.generate_expressao(expr);
                // 2. empilha o objeto
                self.bytecode_instructions
                    .push(format!("LOAD_VAR {}", objeto_nome));
                // 3. altera a propriedade
                self.bytecode_instructions
                    .push(format!("SET_PROPERTY {}", prop_nome));
                // 4. grava o objeto atualizado na variável
                self.bytecode_instructions
                    .push(format!("STORE_VAR {}", objeto_nome)); //  <<< ADICIONE ESTA LINHA
            }

            // Para outros comandos não implementados, remova a linha de comentário e implemente se necessário
            _ => { /* Fazer nada ou adicionar tratamento para outros comandos */ }
        }
    }

    // Altera a assinatura para `&mut self` e remove o retorno Vec<String>
    fn generate_expressao(&mut self, expr: &ast::Expressao) {
        match expr {
            ast::Expressao::Texto(s) => self
                .bytecode_instructions
                .push(format!("LOAD_CONST_STR \"{}\"", s)),
            ast::Expressao::Inteiro(n) => self
                .bytecode_instructions
                .push(format!("LOAD_CONST_INT {}", n)),
            ast::Expressao::Booleano(b) => self
                .bytecode_instructions
                .push(format!("LOAD_CONST_BOOL {}", b)),
            ast::Expressao::Identificador(nome) => {
                if self.em_metodo {
                    // dentro de método ⇒ equivalente a “este.<nome>”
                    self.bytecode_instructions.push("LOAD_VAR este".to_string());
                    self.bytecode_instructions
                        .push(format!("GET_PROPERTY {}", nome));
                } else {
                    self.bytecode_instructions
                        .push(format!("LOAD_VAR {}", nome));
                }
            }

            // Expressão para criar um novo objeto
            ast::Expressao::NovoObjeto(classe_nome, argumentos) => {
                // Primeiro, gera o bytecode para cada argumento, colocando-os na pilha
                for arg in argumentos {
                    self.generate_expressao(arg);
                }
                // Em seguida, emite a instrução para criar um novo objeto
                self.bytecode_instructions.push(format!(
                    "NEW_OBJECT {} {}",
                    classe_nome,
                    argumentos.len()
                ));
            }

            // Modificado: Operadores Aritméticos - Distinguir concatenação de soma numérica
            ast::Expressao::Aritmetica(op, esq, dir) => {
                self.generate_expressao(esq);
                self.generate_expressao(dir);
                match op {
                    ast::OperadorAritmetico::Soma => {
                        // Idealmente, haveria verificação de tipo aqui, ou um operador polimórfico.

                        if Self::is_string_expr(esq) || Self::is_string_expr(dir) {
                            self.bytecode_instructions.push("CONCAT 2".to_string());
                        } else {
                            self.bytecode_instructions.push("ADD".to_string());
                        }
                    }
                    ast::OperadorAritmetico::Subtracao => {
                        self.bytecode_instructions.push("SUB".to_string())
                    }
                    ast::OperadorAritmetico::Multiplicacao => {
                        self.bytecode_instructions.push("MUL".to_string())
                    }
                    ast::OperadorAritmetico::Divisao => {
                        self.bytecode_instructions.push("DIV".to_string())
                    }
                    ast::OperadorAritmetico::Modulo => {
                        self.bytecode_instructions.push("MOD".to_string())
                    }
                }
            }

            // Adicionado: Operadores de Comparação
            ast::Expressao::Comparacao(op, esq, dir) => {
                self.generate_expressao(esq);
                self.generate_expressao(dir);
                match op {
                    ast::OperadorComparacao::Igual => {
                        self.bytecode_instructions.push("COMPARE_EQ".to_string())
                    }
                    ast::OperadorComparacao::Diferente => {
                        self.bytecode_instructions.push("COMPARE_NE".to_string())
                    }
                    ast::OperadorComparacao::Menor => {
                        self.bytecode_instructions.push("COMPARE_LT".to_string())
                    }
                    ast::OperadorComparacao::MaiorQue => {
                        self.bytecode_instructions.push("COMPARE_GT".to_string())
                    }
                    ast::OperadorComparacao::MenorIgual => {
                        self.bytecode_instructions.push("COMPARE_LE".to_string())
                    }
                    ast::OperadorComparacao::MaiorIgual => {
                        self.bytecode_instructions.push("COMPARE_GE".to_string())
                    }
                }
            }

            // Adicionado: Operadores Unários
            ast::Expressao::Unario(op, expr) => {
                self.generate_expressao(expr);
                match op {
                    ast::OperadorUnario::NegacaoLogica => {
                        self.bytecode_instructions.push("NEGATE_BOOL".to_string())
                    }
                    ast::OperadorUnario::NegacaoNumerica => {
                        self.bytecode_instructions.push("NEGATE_INT".to_string())
                    }
                }
            }

            ast::Expressao::StringInterpolada(partes) => {
                // Empilha cada pedaço (texto ou expressão)
                for parte in partes {
                    match parte {
                        ast::PartStringInterpolada::Texto(s) => {
                            self.bytecode_instructions
                                .push(format!("LOAD_CONST_STR \"{}\"", s));
                        }
                        ast::PartStringInterpolada::Expressao(e) => {
                            self.generate_expressao(e);
                        }
                    }
                }
                // Concatena tudo; resultado fica no topo da pilha
                self.bytecode_instructions
                    .push(format!("CONCAT {}", partes.len()));
            }

            // Para outras expressões não implementadas, remova a linha de comentário e implemente se necessário
            _ => { /* Fazer nada ou adicionar tratamento para outras expressões */ }
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
        fs::write(format!("{}/{}.csproj", dir_projeto, nome_base), csproj)
            .map_err(|e| e.to_string())?;

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
}}"#,
            nome_base, main_body
        );
        fs::write(format!("{}/Program.cs", dir_projeto), program_cs).map_err(|e| e.to_string())
    }

    pub fn gerar_bytecode(&self, programa: &ast::Programa, nome_base: &str) -> Result<(), String> {
        let mut generator = BytecodeGenerator::new(programa, false);
        let bytecode = generator.generate();
        fs::write(format!("{}.pbc", nome_base), bytecode.join("\n")).map_err(|e| e.to_string())
    }
}
