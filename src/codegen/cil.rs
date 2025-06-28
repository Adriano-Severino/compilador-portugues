use crate::ast;

/// O gerador de código para o alvo CIL (Common Intermediate Language) do .NET.
pub struct CilGenerator<'a> {
    programa: &'a ast::Programa,
    assembly_name: String,
}

impl<'a> CilGenerator<'a> {
    pub fn new(programa: &'a ast::Programa, assembly_name: String) -> Self {
        Self {
            programa,
            assembly_name,
        }
    }

    pub fn generate(&self) -> String {
        let mut code = String::new();
        code.push_str(&format!(".assembly extern mscorlib\n"));
        code.push_str(&format!(".assembly {}\n\n", self.assembly_name));
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
                code.push_str(&format!("    ldstr \"{}\n\"\n", s.replace('\n', "")));
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
