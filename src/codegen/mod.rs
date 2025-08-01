pub mod llvm_ir;
pub mod cil;
pub mod console;
pub mod bytecode;

use crate::ast;
use std::fs;

pub struct GeradorCodigo;

impl GeradorCodigo {
    pub fn new() -> Result<Self, String> {
        Ok(Self)
    }

    pub fn gerar_llvm_ir(&self, programa: &ast::Programa, nome_base: &str) -> Result<(), String> {
        let mut generator = llvm_ir::LlvmGenerator::new(programa);
        let code = generator.generate();
        fs::write(format!("{}.ll", nome_base), code).map_err(|e| e.to_string())
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
            r#"<Project Sdk=\"Microsoft.NET.Sdk\">\n  <PropertyGroup>\n    <OutputType>Exe</OutputType>\n    <TargetFramework>net8.0</TargetFramework>\n    <ImplicitUsings>enable</ImplicitUsings>\n    <Nullable>enable</Nullable>\n  </PropertyGroup>\n</Project>"#
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
