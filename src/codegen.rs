use inkwell::{
    context::Context,
    module::Linkage,
    AddressSpace,
};

pub struct GeradorCodigo<'ctx> {
    pub context: &'ctx Context,
    pub module: inkwell::module::Module<'ctx>,
    pub builder: inkwell::builder::Builder<'ctx>,
}

impl<'ctx> GeradorCodigo<'ctx> {
    pub fn new(context: &'ctx Context) -> Self {
        let module = context.create_module("main");
        let builder = context.create_builder();
        Self { context, module, builder }
    }

    // Novo método para compilar programa completo
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
        }
    }

    fn gerar_imprima(&self, texto: &str) -> Result<(), String> {
        let i8_ptr_type = self.context.ptr_type(AddressSpace::default());
        let i32_type = self.context.i32_type();

        let puts_fn_type = i32_type.fn_type(&[i8_ptr_type.into()], false);
        let puts_func = self.module.get_function("puts").unwrap_or_else(|| {
            self.module.add_function("puts", puts_fn_type, Some(Linkage::External))
        });

        let c_string = format!("{}\0", texto);
        let global_string_ptr = self.builder
            .build_global_string_ptr(&c_string, "")
            .map_err(|e| format!("Erro ao criar string global: {:?}", e))?
            .as_pointer_value();

        self.builder.build_call(puts_func, &[global_string_ptr.into()], "puts_call")
            .map_err(|e| format!("Erro ao gerar call: {:?}", e))?;

        Ok(())
    }

    fn gerar_se(&self, _cond: &super::ast::Expressao, _cmd: &super::ast::Comando) -> Result<(), String> {
        Ok(())
    }
}