use inkwell::context::Context;

pub struct GeradorCodigo<'ctx> {
    context: &'ctx Context,
    module: inkwell::module::Module<'ctx>,
    builder: inkwell::builder::Builder<'ctx>,
}

impl<'ctx> GeradorCodigo<'ctx> {
    pub fn new(context: &'ctx Context) -> Self {
        let module = context.create_module("main");
        let builder = context.create_builder();
        Self { context, module, builder }
    }

    pub fn compilar_comando(&self, comando: &ast::Comando) {
        match comando {
            ast::Comando::Se(cond, cmd) => self.gerar_se(cond, cmd),
            ast::Comando::Imprima(s) => self.gerar_imprima(s),
        }
    }

    fn gerar_imprima(&self, texto: &str) {
        // Implementação de função de impressão
    }

    fn gerar_se(&self, cond: &ast::Expressao, cmd: &ast::Comando) {
        // Implementação de condicional
    }
}
