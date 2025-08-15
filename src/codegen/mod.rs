pub mod bytecode;
pub mod cil;
pub mod console;
pub mod llvm_ir;

use crate::ast;
use std::fs;

pub struct GeradorCodigo;

impl GeradorCodigo {
    pub fn new() -> Result<Self, String> {
        Ok(Self)
    }

    pub fn gerar_llvm_ir<'a>(
        &self,
        programa: &'a ast::Programa,
        type_checker: &'a mut crate::type_checker::VerificadorTipos<'a>,
        nome_base: &str,
    ) -> Result<(), String> {
        let mut generator =
            llvm_ir::LlvmGenerator::new(programa, type_checker, &type_checker.resolved_classes);
        let code = generator.generate();
        let ll_path = format!("{}.ll", nome_base);
        fs::write(&ll_path, code).map_err(|e| e.to_string())?;

        // Compila o LLVM IR para código de máquina usando clang
        let output = std::process::Command::new("clang")
            .arg(&ll_path)
            .arg("-o")
            .arg(nome_base)
            .output()
            .map_err(|e| format!("Falha ao executar o clang: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "Erro do Clang: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        Ok(())
    }

    pub fn gerar_cil(&self, programa: &ast::Programa, nome_base: &str) -> Result<(), String> {
        let generator = cil::CilGenerator::new(programa, nome_base.to_string());
        let code = generator.generate();
        fs::write(format!("{}.il", nome_base), code).map_err(|e| e.to_string())
    }

    pub fn gerar_console(&self, programa: &ast::Programa, nome_base: &str) -> Result<(), String> {
        let generator = console::ConsoleGenerator::new(programa);
        let main_body = generator.generate();

        let dir_projeto = format!("./{}", nome_base);
        fs::create_dir_all(&dir_projeto).map_err(|e| e.to_string())?;

        let csproj = format!(
            "<Project Sdk=\"Microsoft.NET.Sdk\">\n  <PropertyGroup>\n    <OutputType>Exe</OutputType>\n    <TargetFramework>net8.0</TargetFramework>\n    <ImplicitUsings>enable</ImplicitUsings>\n    <Nullable>enable</Nullable>\n  </PropertyGroup>\n</Project>"
        );
        fs::write(format!("{}/{}.csproj", dir_projeto, nome_base), csproj)
            .map_err(|e| e.to_string())?;

        let program_cs = format!(
            r#"namespace {}\n{{\n    class Program\n    {{\n        static void Main(string[] args)\n        {{\n{}\n        }}\n    }}\n}}"#,
            nome_base, main_body
        );
        fs::write(format!("{}/Program.cs", dir_projeto), program_cs).map_err(|e| e.to_string())
    }

    pub fn gerar_bytecode<'a>(
        &mut self,
        programa: &'a ast::Programa,
        type_checker: &'a crate::type_checker::VerificadorTipos,
        nome_base: &str,
    ) -> Result<(), String> {
        let mut generator = bytecode::BytecodeGenerator::new(programa, type_checker);
        let bytecode = generator.generate();
        fs::write(format!("{}.pbc", nome_base), bytecode.join("\n")).map_err(|e| e.to_string())
    }
}

/// Gera apenas o LLVM IR (string), sem invocar o clang.
/// Útil para testes que validam a geração de IR sem dependências externas.
pub fn gerar_llvm_ir_puro<'a>(
    programa: &'a ast::Programa,
    type_checker: &'a mut crate::type_checker::VerificadorTipos<'a>,
) -> String {
    let mut generator =
        llvm_ir::LlvmGenerator::new(programa, type_checker, &type_checker.resolved_classes);
    generator.generate()
}
