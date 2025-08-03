use crate::ast;

/// O gerador de código para o alvo Console Application em C#.
pub struct ConsoleGenerator<'a> {
    programa: &'a ast::Programa,
}

impl<'a> ConsoleGenerator<'a> {
    pub fn new(programa: &'a ast::Programa) -> Self {
        Self { programa }
    }

    pub fn generate(&self) -> String {
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
            ast::Expressao::Texto(s) => format!("{}", s),
            ast::Expressao::Inteiro(n) => n.to_string(),
            ast::Expressao::Decimal(d) => format!("{}m", d),
            ast::Expressao::Identificador(name) => name.clone(),
            ast::Expressao::Aritmetica(ast::OperadorAritmetico::Soma, esq, dir) => {
                format!(
                    "{} + {}",
                    self.generate_expressao(esq),
                    self.generate_expressao(dir)
                )
            }
            _ => format!("ERRO: Expressao {:?} nao suportada", expr),
        }
    }

    fn map_type(&self, tipo: &ast::Tipo) -> &str {
        match tipo {
            ast::Tipo::Inteiro => "int",
            ast::Tipo::Texto => "string",
            ast::Tipo::Booleano => "bool",
            ast::Tipo::Decimal => "decimal",
            _ => "object",
        }
    }
}
