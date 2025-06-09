use inkwell::{
    context::Context,
    module::Linkage,
    AddressSpace,
    // REMOVIDO: values::BasicValueEnum,
    // REMOVIDO: IntPredicate,
};
use std::collections::HashMap;
use std::cell::RefCell;

pub struct GeradorCodigo<'ctx> {
    pub context: &'ctx Context,
    pub module: inkwell::module::Module<'ctx>,
    pub builder: inkwell::builder::Builder<'ctx>,
    pub variaveis: RefCell<HashMap<String, i64>>,
}

impl<'ctx> GeradorCodigo<'ctx> {
    pub fn new(context: &'ctx Context) -> Self {
        let module = context.create_module("main");
        let builder = context.create_builder();
        Self { 
            context, 
            module, 
            builder,
            variaveis: RefCell::new(HashMap::new()),
        }
    }

    pub fn compilar_programa(&self, programa: &super::ast::Programa) -> Result<(), String> {
        for declaracao in &programa.declaracoes {
            match declaracao {
                super::ast::Declaracao::Comando(comando) => {
                    self.compilar_comando(comando)?;
                },
                super::ast::Declaracao::DeclaracaoFuncao(funcao) => {
                    // Por enquanto, processa apenas os comandos das funções
                    for comando in &funcao.corpo {
                        self.compilar_comando(comando)?;
                    }
                },
                _ => {
                    // Outros tipos de declaração não implementados ainda
                }
            }
        }
        Ok(())
    }

    pub fn compilar_comando(&self, comando: &super::ast::Comando) -> Result<(), String> {
        match comando {
            super::ast::Comando::Se(cond, cmd_then, cmd_else) => {
                self.gerar_se(cond, cmd_then, cmd_else.as_ref().map(|v| &**v))
            },
            super::ast::Comando::Enquanto(cond, cmd) => {
                self.gerar_enquanto(cond, cmd)
            },
            super::ast::Comando::Imprima(expr) => self.gerar_imprima(expr),
            super::ast::Comando::Bloco(comandos) => self.gerar_bloco(comandos),
            super::ast::Comando::DeclaracaoVariavel(tipo, nome, valor) => {
                self.gerar_declaracao_variavel(tipo, nome, valor.as_ref())
            },
            super::ast::Comando::Atribuicao(nome, expr) => {
                self.gerar_atribuicao(nome, expr)
            },
            super::ast::Comando::Retorne(expr) => {
                self.gerar_retorne(expr.as_ref())
            },
            super::ast::Comando::Expressao(expr) => {
                self.avaliar_expressao(expr)?;
                Ok(())
            },
            _ => Ok(()),
        }
    }

    fn gerar_bloco(&self, comandos: &[super::ast::Comando]) -> Result<(), String> {
        for comando in comandos {
            self.compilar_comando(comando)?;
        }
        Ok(())
    }

    fn gerar_imprima(&self, expr: &super::ast::Expressao) -> Result<(), String> {
        let valor = self.avaliar_expressao(expr)?;
        
        let i8_ptr_type = self.context.ptr_type(AddressSpace::default());
        let i32_type = self.context.i32_type();

        match valor {
            ValorAvaliado::Inteiro(int_val) => {
                let llvm_val = self.context.i64_type().const_int(int_val as u64, true);
                
                let format_str = "%lld\n\0";
                let global_string_ptr = self.builder
                    .build_global_string_ptr(format_str, "int_format")
                    .map_err(|e| format!("Erro ao criar format string: {:?}", e))?
                    .as_pointer_value();

                let printf_fn_type = i32_type.fn_type(&[i8_ptr_type.into()], true);
                let printf_func = self.module.get_function("printf").unwrap_or_else(|| {
                    self.module.add_function("printf", printf_fn_type, Some(Linkage::External))
                });

                self.builder.build_call(printf_func, &[global_string_ptr.into(), llvm_val.into()], "printf_call")
                    .map_err(|e| format!("Erro ao gerar call printf: {:?}", e))?;
            },
            ValorAvaliado::Texto(text_val) => {
                let c_string = format!("{}\0", text_val);
                let global_string_ptr = self.builder
                    .build_global_string_ptr(&c_string, "string_literal")
                    .map_err(|e| format!("Erro ao criar string literal: {:?}", e))?
                    .as_pointer_value();

                let puts_fn_type = i32_type.fn_type(&[i8_ptr_type.into()], false);
                let puts_func = self.module.get_function("puts").unwrap_or_else(|| {
                    self.module.add_function("puts", puts_fn_type, Some(Linkage::External))
                });

                self.builder.build_call(puts_func, &[global_string_ptr.into()], "puts_call")
                    .map_err(|e| format!("Erro ao gerar call puts: {:?}", e))?;
            },
            ValorAvaliado::Booleano(bool_val) => {
                let text = if bool_val { "verdadeiro" } else { "falso" };
                let c_string = format!("{}\0", text);
                let global_string_ptr = self.builder
                    .build_global_string_ptr(&c_string, "bool_literal")
                    .map_err(|e| format!("Erro ao criar bool literal: {:?}", e))?
                    .as_pointer_value();

                let puts_fn_type = i32_type.fn_type(&[i8_ptr_type.into()], false);
                let puts_func = self.module.get_function("puts").unwrap_or_else(|| {
                    self.module.add_function("puts", puts_fn_type, Some(Linkage::External))
                });

                self.builder.build_call(puts_func, &[global_string_ptr.into()], "puts_call")
                    .map_err(|e| format!("Erro ao gerar call puts: {:?}", e))?;
            },
        }

        Ok(())
    }

    fn avaliar_expressao(&self, expr: &super::ast::Expressao) -> Result<ValorAvaliado, String> {
        match expr {
            super::ast::Expressao::Inteiro(val) => Ok(ValorAvaliado::Inteiro(*val)),
            super::ast::Expressao::Texto(val) => Ok(ValorAvaliado::Texto(val.clone())),
            super::ast::Expressao::Booleano(val) => Ok(ValorAvaliado::Booleano(*val)),
            super::ast::Expressao::Identificador(nome) => {
                let variaveis = self.variaveis.borrow();
                match variaveis.get(nome) {
                    Some(valor) => Ok(ValorAvaliado::Inteiro(*valor)),
                    None => Err(format!("Variável '{}' não foi declarada", nome)),
                }
            },
            super::ast::Expressao::Aritmetica(op, esq, dir) => {
                let val_esq = self.avaliar_expressao(esq)?;
                let val_dir = self.avaliar_expressao(dir)?;
                
                if let (ValorAvaliado::Inteiro(left), ValorAvaliado::Inteiro(right)) = (val_esq, val_dir) {
                    let resultado = match op {
                        super::ast::OperadorAritmetico::Soma => left + right,
                        super::ast::OperadorAritmetico::Subtracao => left - right,
                        super::ast::OperadorAritmetico::Multiplicacao => left * right,
                        super::ast::OperadorAritmetico::Divisao => {
                            if right == 0 {
                                return Err("Divisão por zero".to_string());
                            }
                            left / right
                        },
                        super::ast::OperadorAritmetico::Modulo => {
                            if right == 0 {
                                return Err("Módulo por zero".to_string());
                            }
                            left % right
                        },
                    };
                    Ok(ValorAvaliado::Inteiro(resultado))
                } else {
                    Err("Operação aritmética requer valores inteiros".to_string())
                }
            },
            super::ast::Expressao::Comparacao(op, esq, dir) => {
                let val_esq = self.avaliar_expressao(esq)?;
                let val_dir = self.avaliar_expressao(dir)?;
                
                if let (ValorAvaliado::Inteiro(left), ValorAvaliado::Inteiro(right)) = (val_esq, val_dir) {
                    let resultado = match op {
                        super::ast::OperadorComparacao::Igual => left == right,
                        super::ast::OperadorComparacao::Diferente => left != right,
                        super::ast::OperadorComparacao::MaiorQue => left > right,
                        super::ast::OperadorComparacao::MaiorIgual => left >= right,
                        super::ast::OperadorComparacao::Menor => left < right,
                        super::ast::OperadorComparacao::MenorIgual => left <= right,
                    };
                    Ok(ValorAvaliado::Booleano(resultado))
                } else {
                    Err("Comparação requer valores do mesmo tipo".to_string())
                }
            },
            _ => Err("Expressão não implementada".to_string()),
        }
    }

    fn gerar_se(&self, cond: &super::ast::Expressao, cmd_then: &super::ast::Comando, cmd_else: Option<&super::ast::Comando>) -> Result<(), String> {
        let resultado_cond = self.avaliar_expressao(cond)?;
        
        let executa_then = match resultado_cond {
            ValorAvaliado::Booleano(val) => val,
            ValorAvaliado::Inteiro(val) => val != 0,
            _ => true,
        };

        if executa_then {
            self.compilar_comando(cmd_then)?;
        } else if let Some(else_cmd) = cmd_else {
            self.compilar_comando(else_cmd)?;
        }

        Ok(())
    }

    fn gerar_enquanto(&self, cond: &super::ast::Expressao, cmd: &super::ast::Comando) -> Result<(), String> {
        const MAX_ITERACOES: i32 = 10000;
        let mut iteracoes = 0;

        loop {
            if iteracoes >= MAX_ITERACOES {
                return Err("Loop 'enquanto' excedeu o limite máximo de iterações".to_string());
            }

            let resultado_cond = self.avaliar_expressao(cond)?;
            let continua = match resultado_cond {
                ValorAvaliado::Booleano(val) => val,
                ValorAvaliado::Inteiro(val) => val != 0,
                _ => false,
            };

            if !continua {
                break;
            }

            self.compilar_comando(cmd)?;
            iteracoes += 1;
        }

        Ok(())
    }

    fn gerar_declaracao_variavel(&self, _tipo: &super::ast::Tipo, nome: &str, valor: Option<&super::ast::Expressao>) -> Result<(), String> {
        let val = if let Some(expr) = valor {
            match self.avaliar_expressao(expr)? {
                ValorAvaliado::Inteiro(v) => v,
                _ => 0,
            }
        } else {
            0
        };

        self.variaveis.borrow_mut().insert(nome.to_string(), val);
        Ok(())
    }

    fn gerar_atribuicao(&self, nome: &str, expr: &super::ast::Expressao) -> Result<(), String> {
        let valor = match self.avaliar_expressao(expr)? {
            ValorAvaliado::Inteiro(v) => v,
            _ => return Err("Atribuição suporta apenas valores inteiros por enquanto".to_string()),
        };
        
        if !self.variaveis.borrow().contains_key(nome) {
            return Err(format!("Variável '{}' não foi declarada", nome));
        }

        self.variaveis.borrow_mut().insert(nome.to_string(), valor);
        Ok(())
    }

    fn gerar_retorne(&self, _expr: Option<&super::ast::Expressao>) -> Result<(), String> {
        Ok(())
    }
}

#[derive(Debug, Clone)]
enum ValorAvaliado {
    Inteiro(i64),
    Texto(String),
    Booleano(bool),
}