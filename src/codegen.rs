use inkwell::{
    context::Context,
    module::Linkage,
    AddressSpace,
};
use std::collections::HashMap;
use std::cell::RefCell;

pub struct GeradorCodigo<'ctx> {
    pub context: &'ctx Context,
    pub module: inkwell::module::Module<'ctx>,
    pub builder: inkwell::builder::Builder<'ctx>,
    pub variaveis: RefCell<HashMap<String, ValorAvaliado>>,
    pub escopos: RefCell<Vec<HashMap<String, ValorAvaliado>>>, // NOVO: Sistema de escopos
    pub funcoes: RefCell<HashMap<String, super::ast::DeclaracaoFuncao>>, // NOVO: Registro de funções
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
            escopos: RefCell::new(vec![HashMap::new()]), // Escopo global
            funcoes: RefCell::new(HashMap::new()),
        }
    }

    // NOVO: Métodos para gerenciar escopos
    fn entrar_escopo(&self) {
        self.escopos.borrow_mut().push(HashMap::new());
    }

    fn sair_escopo(&self) {
        self.escopos.borrow_mut().pop();
    }

    fn buscar_variavel(&self, nome: &str) -> Option<ValorAvaliado> {
        // Buscar primeiro no sistema de escopos (do mais interno ao externo)
        let escopos = self.escopos.borrow();
        for escopo in escopos.iter().rev() {
            if let Some(valor) = escopo.get(nome) {
                return Some(valor.clone());
            }
        }
        
        // Depois buscar no sistema antigo para compatibilidade
        let variaveis = self.variaveis.borrow();
        variaveis.get(nome).cloned()
    }

    fn definir_variavel(&self, nome: String, valor: ValorAvaliado) {
        let mut escopos = self.escopos.borrow_mut();
        if let Some(escopo_atual) = escopos.last_mut() {
            escopo_atual.insert(nome, valor);
        }
    }

    pub fn compilar_programa(&self, programa: &super::ast::Programa) -> Result<(), String> {
        // Primeira passada: registrar todas as funções
        for declaracao in &programa.declaracoes {
            if let super::ast::Declaracao::DeclaracaoFuncao(funcao) = declaracao {
                self.funcoes.borrow_mut().insert(funcao.nome.clone(), funcao.clone());
            }
        }

        // Segunda passada: processar declarações
        for declaracao in &programa.declaracoes {
            match declaracao {
                super::ast::Declaracao::Comando(comando) => {
                    self.compilar_comando(comando)?;
                },
                super::ast::Declaracao::DeclaracaoFuncao(funcao) => {
                    self.compilar_funcao(funcao)?; // MODIFICADO: usar método específico
                },
                _ => {
                    // Outros tipos de declaração não implementados ainda
                }
            }
        }
        Ok(())
    }

    // NOVO: Método específico para compilar funções
    fn compilar_funcao(&self, funcao: &super::ast::DeclaracaoFuncao) -> Result<(), String> {
        // Entrar em novo escopo para a função
        self.entrar_escopo();

        // Registrar parâmetros da função no escopo atual
        for parametro in &funcao.parametros {
            // Para parâmetros, inicializamos com valores padrão baseados no tipo
            let valor_padrao = match parametro.tipo {
                super::ast::Tipo::Inteiro => ValorAvaliado::Inteiro(0),
                super::ast::Tipo::Texto => ValorAvaliado::Texto(String::new()),
                super::ast::Tipo::Booleano => ValorAvaliado::Booleano(false),
                _ => ValorAvaliado::Inteiro(0), // Valor padrão para outros tipos
            };
            
            self.definir_variavel(parametro.nome.clone(), valor_padrao);
        }

        // Processar corpo da função
        for comando in &funcao.corpo {
            self.compilar_comando(comando)?;
        }

        // Sair do escopo da função
        self.sair_escopo();
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
        // Criar novo escopo para o bloco
        self.entrar_escopo();
        
        for comando in comandos {
            self.compilar_comando(comando)?;
        }
        
        // Sair do escopo do bloco
        self.sair_escopo();
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
                // MODIFICADO: usar buscar_variavel que funciona com escopos
                match self.buscar_variavel(nome) {
                    Some(valor) => Ok(valor),
                    None => Err(format!("Variável '{}' não foi declarada", nome)),
                }
            },
            super::ast::Expressao::Chamada(nome_funcao, argumentos) => {
                // NOVO: Suporte básico para chamadas de função
                self.avaliar_chamada_funcao(nome_funcao, argumentos)
            },
            super::ast::Expressao::NovoObjeto(classe, argumentos) => {
                // NOVO: Suporte básico para criação de objetos
                println!("Criando objeto da classe: {} com {} argumentos", classe, argumentos.len());
                Ok(ValorAvaliado::Texto(format!("Objeto de {}", classe)))
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

    // NOVO: Método para avaliar chamadas de função
    fn avaliar_chamada_funcao(&self, nome_funcao: &str, argumentos: &[super::ast::Expressao]) -> Result<ValorAvaliado, String> {
        // Por enquanto, simular execução de funções conhecidas
        match nome_funcao {
            "abs" => {
                if argumentos.len() != 1 {
                    return Err("Função 'abs' espera exatamente 1 argumento".to_string());
                }
                let valor = self.avaliar_expressao(&argumentos[0])?;
                if let ValorAvaliado::Inteiro(num) = valor {
                    Ok(ValorAvaliado::Inteiro(num.abs()))
                } else {
                    Err("Função 'abs' espera um número inteiro".to_string())
                }
            },
            _ => {
                // Para outras funções, simular execução
                println!("Chamando função '{}' com {} argumentos", nome_funcao, argumentos.len());
                
                // Verificar se a função existe
                if self.funcoes.borrow().contains_key(nome_funcao) {
                    // Simular retorno baseado no nome da função
                    if nome_funcao.contains("obter") || nome_funcao.contains("gerar") {
                        Ok(ValorAvaliado::Texto(format!("Resultado de {}", nome_funcao)))
                    } else if nome_funcao.contains("eh_") {
                        Ok(ValorAvaliado::Booleano(true))
                    } else {
                        Ok(ValorAvaliado::Inteiro(0))
                    }
                } else {
                    Err(format!("Função '{}' não foi declarada", nome_funcao))
                }
            }
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

    fn gerar_declaracao_variavel(&self, tipo: &super::ast::Tipo, nome: &str, valor: Option<&super::ast::Expressao>) -> Result<(), String> {
        let val = if let Some(expr) = valor {
            self.avaliar_expressao(expr)?
        } else {
            // Valor padrão baseado no tipo
            match tipo {
                super::ast::Tipo::Inteiro => ValorAvaliado::Inteiro(0),
                super::ast::Tipo::Texto => ValorAvaliado::Texto(String::new()),
                super::ast::Tipo::Booleano => ValorAvaliado::Booleano(false),
                _ => ValorAvaliado::Inteiro(0),
            }
        };

        // MODIFICADO: usar definir_variavel que funciona com escopos
        self.definir_variavel(nome.to_string(), val);
        Ok(())
    }

    fn gerar_atribuicao(&self, nome: &str, expr: &super::ast::Expressao) -> Result<(), String> {
        let valor = self.avaliar_expressao(expr)?;
        
        // Verificar se a variável existe em algum escopo
        if self.buscar_variavel(nome).is_none() {
            return Err(format!("Variável '{}' não foi declarada", nome));
        }

        // MODIFICADO: usar definir_variavel
        self.definir_variavel(nome.to_string(), valor);
        Ok(())
    }

    fn gerar_retorne(&self, _expr: Option<&super::ast::Expressao>) -> Result<(), String> {
        // Por enquanto, apenas simular o retorno
        Ok(())
    }
}

#[derive(Debug, Clone)]
enum ValorAvaliado {
    Inteiro(i64),
    Texto(String),
    Booleano(bool),
}