use inkwell::{context::Context, module::Linkage, AddressSpace};

pub struct GeradorCodigo<'ctx> {
    pub context: &'ctx Context,
    pub module: inkwell::module::Module<'ctx>,
    pub builder: inkwell::builder::Builder<'ctx>,
}

impl<'ctx> GeradorCodigo<'ctx> {
    pub fn new(context: &'ctx Context) -> Self {
        let module = context.create_module("main");
        let builder = context.create_builder();
        Self {
            context,
            module,
            builder,
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
            super::ast::Comando::Se(_cond, _cmd) => self.gerar_se(_cond, _cmd),
            super::ast::Comando::Imprima(s) => self.gerar_imprima(s),
            super::ast::Comando::Bloco(comandos) => self.gerar_bloco(comandos), // Novo caso
        }
    }

    // Novo método para compilar blocos
    fn gerar_bloco(&self, comandos: &[super::ast::Comando]) -> Result<(), String> {
        for comando in comandos {
            self.compilar_comando(comando)?;
        }
        Ok(())
    }

    fn gerar_imprima(&self, texto: &str) -> Result<(), String> {
        let i8_ptr_type = self.context.ptr_type(AddressSpace::default());
        let i32_type = self.context.i32_type();

        let puts_fn_type = i32_type.fn_type(&[i8_ptr_type.into()], false);
        let puts_func = self.module.get_function("puts").unwrap_or_else(|| {
            self.module
                .add_function("puts", puts_fn_type, Some(Linkage::External))
        });

        let c_string = format!("{}\0", texto);
        let global_string_ptr = self
            .builder
            .build_global_string_ptr(&c_string, "")
            .map_err(|e| format!("Erro ao criar string global: {:?}", e))?
            .as_pointer_value();

        self.builder
            .build_call(puts_func, &[global_string_ptr.into()], "puts_call")
            .map_err(|e| format!("Erro ao gerar call: {:?}", e))?;

        Ok(())
    }

    fn gerar_se(
        &self,
        cond: &super::ast::Expressao,
        cmd: &super::ast::Comando,
    ) -> Result<(), String> {
        // Por enquanto, vamos avaliar a condição estaticamente
        let condicao_verdadeira = self.avaliar_condicao_estatica(cond)?;

        if condicao_verdadeira {
            self.compilar_comando(cmd)?;
        }

        Ok(())
    }

    // Método auxiliar para avaliar condições simples estaticamente
    fn avaliar_condicao_estatica(&self, expr: &super::ast::Expressao) -> Result<bool, String> {
        match expr {
            super::ast::Expressao::Comparacao(op, esq, dir) => {
                let val_esq = self.extrair_valor_inteiro(esq)?;
                let val_dir = self.extrair_valor_inteiro(dir)?;

                match op {
                    super::ast::OperadorComparacao::MaiorQue => Ok(val_esq > val_dir),
                }
            }
            _ => Err("Expressão não suportada para avaliação estática".to_string()),
        }
    }

    fn extrair_valor_inteiro(&self, expr: &super::ast::Expressao) -> Result<i64, String> {
        match expr {
            super::ast::Expressao::Inteiro(val) => Ok(*val),
            _ => Err("Esperado valor inteiro".to_string()),
        }
    }
}
