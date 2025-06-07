use inkwell::{
    context::Context,
    module::Linkage,
    AddressSpace,
    values::BasicValueEnum,
    types::BasicTypeEnum,
    IntPredicate,
};
use std::collections::HashMap;
use std::cell::RefCell;

pub struct GeradorCodigo<'ctx> {
    pub context: &'ctx Context,
    pub module: inkwell::module::Module<'ctx>,
    pub builder: inkwell::builder::Builder<'ctx>,
    pub variaveis: RefCell<HashMap<String, BasicValueEnum<'ctx>>>,
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
        for comando in &programa.comandos {
            self.compilar_comando(comando)?;
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
                self.compilar_expressao(expr)?;
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
        let valor = self.compilar_expressao(expr)?;
        
        let i8_ptr_type = self.context.ptr_type(AddressSpace::default());
        let i32_type = self.context.i32_type();

        match valor {
            BasicValueEnum::IntValue(int_val) => {
                // Para números inteiros, usar printf com %lld para i64
                let format_str = "%lld\n\0";
                let global_string_ptr = self.builder
                    .build_global_string_ptr(format_str, "int_format")
                    .map_err(|e| format!("Erro ao criar format string: {:?}", e))?
                    .as_pointer_value();

                let printf_fn_type = i32_type.fn_type(&[i8_ptr_type.into()], true);
                let printf_func = self.module.get_function("printf").unwrap_or_else(|| {
                    self.module.add_function("printf", printf_fn_type, Some(Linkage::External))
                });

                self.builder.build_call(printf_func, &[global_string_ptr.into(), int_val.into()], "printf_call")
                    .map_err(|e| format!("Erro ao gerar call printf: {:?}", e))?;
            },
            BasicValueEnum::PointerValue(ptr_val) => {
                // Para strings, usar puts
                let puts_fn_type = i32_type.fn_type(&[i8_ptr_type.into()], false);
                let puts_func = self.module.get_function("puts").unwrap_or_else(|| {
                    self.module.add_function("puts", puts_fn_type, Some(Linkage::External))
                });

                self.builder.build_call(puts_func, &[ptr_val.into()], "puts_call")
                    .map_err(|e| format!("Erro ao gerar call puts: {:?}", e))?;
            },
            _ => return Err("Tipo não suportado para impressão".to_string()),
        }

        Ok(())
    }

    fn compilar_expressao(&self, expr: &super::ast::Expressao) -> Result<BasicValueEnum<'ctx>, String> {
        match expr {
            super::ast::Expressao::Inteiro(val) => {
                Ok(self.context.i64_type().const_int(*val as u64, true).into())
            },
            super::ast::Expressao::Texto(val) => {
                let c_string = format!("{}\0", val);
                Ok(self.builder
                    .build_global_string_ptr(&c_string, "string_literal")
                    .map_err(|e| format!("Erro ao criar string literal: {:?}", e))?
                    .as_pointer_value().into())
            },
            super::ast::Expressao::Booleano(val) => {
                Ok(self.context.bool_type().const_int(*val as u64, false).into())
            },
            super::ast::Expressao::Identificador(nome) => {
                // CORREÇÃO: Recuperar valor real da variável
                let variaveis = self.variaveis.borrow();
                match variaveis.get(nome) {
                    Some(valor) => Ok(*valor),
                    None => Err(format!("Variável '{}' não foi declarada", nome)),
                }
            },
            super::ast::Expressao::Aritmetica(op, esq, dir) => {
                let val_esq = self.compilar_expressao(esq)?;
                let val_dir = self.compilar_expressao(dir)?;
                
                if let (BasicValueEnum::IntValue(left), BasicValueEnum::IntValue(right)) = (val_esq, val_dir) {
                    let resultado = match op {
                        super::ast::OperadorAritmetico::Soma => {
                            self.builder.build_int_add(left, right, "add")
                                .map_err(|e| format!("Erro na soma: {:?}", e))?
                        },
                        super::ast::OperadorAritmetico::Subtracao => {
                            self.builder.build_int_sub(left, right, "sub")
                                .map_err(|e| format!("Erro na subtração: {:?}", e))?
                        },
                        super::ast::OperadorAritmetico::Multiplicacao => {
                            self.builder.build_int_mul(left, right, "mul")
                                .map_err(|e| format!("Erro na multiplicação: {:?}", e))?
                        },
                        super::ast::OperadorAritmetico::Divisao => {
                            self.builder.build_int_signed_div(left, right, "div")
                                .map_err(|e| format!("Erro na divisão: {:?}", e))?
                        },
                    };
                    Ok(resultado.into())
                } else {
                    Err("Operação aritmética requer valores inteiros".to_string())
                }
            },
            super::ast::Expressao::Comparacao(op, esq, dir) => {
                let val_esq = self.compilar_expressao(esq)?;
                let val_dir = self.compilar_expressao(dir)?;
                
                if let (BasicValueEnum::IntValue(left), BasicValueEnum::IntValue(right)) = (val_esq, val_dir) {
                    let pred = match op {
                        super::ast::OperadorComparacao::Igual => IntPredicate::EQ,
                        super::ast::OperadorComparacao::Diferente => IntPredicate::NE,
                        super::ast::OperadorComparacao::MaiorQue => IntPredicate::SGT,
                        super::ast::OperadorComparacao::MaiorIgual => IntPredicate::SGE,
                        super::ast::OperadorComparacao::Menor => IntPredicate::SLT,
                        super::ast::OperadorComparacao::MenorIgual => IntPredicate::SLE,
                    };
                    
                    let resultado = self.builder.build_int_compare(pred, left, right, "cmp")
                        .map_err(|e| format!("Erro na comparação: {:?}", e))?;
                    Ok(resultado.into())
                } else {
                    Err("Comparação requer valores do mesmo tipo".to_string())
                }
            },
            _ => Err("Expressão não implementada".to_string()),
        }
    }

    fn gerar_se(&self, cond: &super::ast::Expressao, cmd_then: &super::ast::Comando, cmd_else: Option<&super::ast::Comando>) -> Result<(), String> {
        // Avaliar condição e executar comando adequado
        let resultado_cond = self.compilar_expressao(cond)?;
        
        // Para simplificar, vamos avaliar estaticamente quando possível
        let executa_then = match resultado_cond {
            BasicValueEnum::IntValue(int_val) => {
                // Se é resultado de comparação (0 ou 1), verificar se é verdadeiro
                int_val.get_sign_extended_constant().unwrap_or(0) != 0
            },
            _ => true, // Default para outros tipos
        };

        if executa_then {
            self.compilar_comando(cmd_then)?;
        } else if let Some(else_cmd) = cmd_else {
            self.compilar_comando(else_cmd)?;
        }

        Ok(())
    }

    fn gerar_enquanto(&self, _cond: &super::ast::Expressao, _cmd: &super::ast::Comando) -> Result<(), String> {
        // Implementação simplificada por enquanto
        Ok(())
    }

    // CORREÇÃO: Implementar declaração de variável corretamente
    fn gerar_declaracao_variavel(&self, _tipo: &super::ast::Tipo, nome: &str, valor: Option<&super::ast::Expressao>) -> Result<(), String> {
        let val = if let Some(expr) = valor {
            self.compilar_expressao(expr)?
        } else {
            // Valor padrão baseado no tipo
            self.context.i64_type().const_int(0, true).into()
        };

        // Armazenar variável no mapa
        self.variaveis.borrow_mut().insert(nome.to_string(), val);
        Ok(())
    }

    // CORREÇÃO: Implementar atribuição corretamente
    fn gerar_atribuicao(&self, nome: &str, expr: &super::ast::Expressao) -> Result<(), String> {
        let valor = self.compilar_expressao(expr)?;
        
        // Verificar se variável existe
        if !self.variaveis.borrow().contains_key(nome) {
            return Err(format!("Variável '{}' não foi declarada", nome));
        }

        // Atualizar valor da variável
        self.variaveis.borrow_mut().insert(nome.to_string(), valor);
        Ok(())
    }

    fn gerar_retorne(&self, _expr: Option<&super::ast::Expressao>) -> Result<(), String> {
        Ok(())
    }
}