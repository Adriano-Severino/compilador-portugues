// Implementação mínima de JIT com Cranelift.
// Objetivo inicial: compilar uma função somatória como PoC.

use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{FuncId, Linkage, Module};
use cranelift_native;

use super::JitError;

pub struct JitHandle {
    pub fn_ptr: *const u8,
}

pub struct CraneliftJit {
    module: JITModule,
    ctx: cranelift_codegen::Context,
    builder_ctx: FunctionBuilderContext,
}

impl CraneliftJit {
    pub fn new() -> Result<Self, JitError> {
        let mut flag_builder = settings::builder();
        flag_builder.set("opt_level", "speed").ok();
        let flags = settings::Flags::new(flag_builder);
        let isa = cranelift_native::builder()
            .map_err(|e| JitError::Interno(e.to_string()))?
            .finish(flags)
            .map_err(|e| JitError::Interno(e.to_string()))?;
        let mut jit_builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());
        let module = JITModule::new(jit_builder);
        Ok(Self {
            module,
            ctx: cranelift_codegen::Context::new(),
            builder_ctx: FunctionBuilderContext::new(),
        })
    }

    // Compila uma função que soma dois i32: fn(i32, i32) -> i32
    pub fn compilar_soma_i32(&mut self) -> Result<JitHandle, JitError> {
        use cranelift_codegen::ir::{types, AbiParam, Function, InstBuilder, Signature};

        let mut sig = Signature::new(self.module.isa().default_call_conv());
        sig.params.push(AbiParam::new(types::I32));
        sig.params.push(AbiParam::new(types::I32));
        sig.returns.push(AbiParam::new(types::I32));

        let func_id: FuncId = self
            .module
            .declare_function("soma_i32", Linkage::Local, &sig)
            .map_err(|e| JitError::Interno(e.to_string()))?;

        let mut func = Function::with_name_signature(
            cranelift_codegen::ir::UserFuncName::user(0, func_id.as_u32()),
            sig,
        );
        self.ctx.func = func;

        let mut builder = FunctionBuilder::new(&mut self.ctx.func, &mut self.builder_ctx);
        let block = builder.create_block();
        builder.append_block_params_for_function_params(block);
        builder.switch_to_block(block);
        builder.seal_block(block);

        let a = builder.block_params(block)[0];
        let b = builder.block_params(block)[1];
        let sum = builder.ins().iadd(a, b);
        builder.ins().return_(&[sum]);
        builder.finalize();

        self.module
            .define_function(func_id, &mut self.ctx)
            .map_err(|e| JitError::Interno(e.to_string()))?;
        self.module.clear_context(&mut self.ctx);
        self.module.finalize_definitions();

        let code = self.module.get_finalized_function(func_id);
        Ok(JitHandle { fn_ptr: code })
    }

    // Executa soma_i32 compilada: safety: o ponteiro é válido para a assinatura declarada.
    pub unsafe fn chamar_soma_i32(&self, handle: &JitHandle, x: i32, y: i32) -> i32 {
        let f: extern "C" fn(i32, i32) -> i32 = std::mem::transmute(handle.fn_ptr);
        f(x, y)
    }
}
