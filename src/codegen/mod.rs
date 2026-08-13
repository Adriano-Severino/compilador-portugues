pub mod bytecode;
pub mod cil;
pub mod console;
pub mod llvm_ir;

use crate::ast;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn clang_executable() -> PathBuf {
    if let Some(path) = env::var_os("CLANG") {
        return PathBuf::from(path);
    }

    #[cfg(windows)]
    {
        if let Some(program_files) = env::var_os("ProgramFiles") {
            let installed = PathBuf::from(program_files)
                .join("LLVM")
                .join("bin")
                .join("clang.exe");
            if installed.is_file() {
                return installed;
            }
        }
    }

    PathBuf::from("clang")
}

/// Compila o LLVM IR gerado e vincula o runtime nativo de async/await.
///
/// O caminho do runtime é resolvido a partir do manifesto do compilador, para
/// que a compilação funcione independentemente do diretório atual do processo.
pub fn compilar_llvm_ir_com_runtime(ll_path: &Path, nome_base: &str) -> Result<(), String> {
    let runtime_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("runtime")
        .join("async_runtime.c");

    if !runtime_path.is_file() {
        return Err(format!(
            "Runtime async nativo não encontrado: {}",
            runtime_path.display()
        ));
    }

    let mut command = Command::new(clang_executable());
    command
        .arg(ll_path)
        .arg(&runtime_path)
        .arg("-o")
        .arg(nome_base);

    #[cfg(not(windows))]
    command.arg("-pthread");

    let output = command
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

        compilar_llvm_ir_com_runtime(Path::new(&ll_path), nome_base)
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

    pub fn gerar_bytecode_para_biblioteca<'a>(
        &mut self,
        programa: &'a ast::Programa,
        type_checker: &'a mut crate::type_checker::VerificadorTipos,
    ) -> Result<String, String> {
        let mut generator = bytecode::BytecodeGenerator::new(programa, type_checker);
        let bytecode = generator.generate_for_library();
        Ok(bytecode.join("\n"))
    }

    /// Gera um arquivo `.pbl` (Biblioteca Por do Sol) composto de:
    ///   1. Seção `[MANIFESTO]` — metadados de tipos públicos (usado pelo compilador para análise semântica)
    ///   2. Seção `[BYTECODE]`  — bytecode dos métodos com corpo (carregado pelo runtime quando necessário)
    ///
    /// O formato é inspirado no modelo de Reference Assemblies do .NET:
    ///   • O compilador lê apenas o [MANIFESTO] para verificação de tipos.
    ///   • O runtime carrega o [BYTECODE] sob demanda (tree-shaking futuro).
    pub fn gerar_pbl<'a>(
        &mut self,
        programa: &'a ast::Programa,
        type_checker: &'a mut crate::type_checker::VerificadorTipos,
        nome_biblioteca: &str,
        versao: &str,
    ) -> Result<String, String> {
        use std::fmt::Write as FmtWrite;
        let mut manifesto = String::new();
        let mut bytecode_secao = String::new();

        // Cabeçalho do arquivo .pbl
        writeln!(manifesto, "[PBL]").ok();
        writeln!(manifesto, "nome={}", nome_biblioteca).ok();
        writeln!(manifesto, "versao={}", versao).ok();
        writeln!(manifesto).ok();
        writeln!(manifesto, "[MANIFESTO]").ok();

        // Itera namespaces e classes para gerar o manifesto
        for ns in &programa.namespaces {
            let ns_nome = &ns.nome;
            for decl in &ns.declaracoes {
                if let ast::Declaracao::DeclaracaoClasse(cl) = decl {
                    let fqn = format!("{}.{}", ns_nome, cl.nome);
                    if cl.eh_estatica {
                        writeln!(manifesto, "DEFINE_STATIC_CLASS {}", fqn).ok();
                    } else {
                        writeln!(manifesto, "DEFINE_CLASS {} NULO", fqn).ok();
                    }
                    for prop in &cl.propriedades {
                        writeln!(manifesto, "PROPERTY {} {} {}", fqn, prop.nome, prop.tipo).ok();
                    }
                    for campo in &cl.campos {
                        if campo.modificador == ast::ModificadorAcesso::Publico {
                            writeln!(manifesto, "FIELD {} {} {}", fqn, campo.nome, campo.tipo).ok();
                        }
                    }
                    for metodo in &cl.metodos {
                        let ret = metodo
                            .tipo_retorno
                            .as_ref()
                            .map(|t| t.to_string())
                            .unwrap_or_else(|| "vazio".to_string());
                        let params: Vec<String> = metodo
                            .parametros
                            .iter()
                            .map(|p| format!("{}:{}", p.tipo, p.nome))
                            .collect();
                        let is_nativo = metodo.attributes.iter().any(|a| a.name == "Nativo");
                        // Extrai a chave nativa se presente
                        let chave_nativa = metodo
                            .attributes
                            .iter()
                            .find(|a| a.name == "Nativo")
                            .and_then(|a| a.arguments.first())
                            .and_then(|e| {
                                if let ast::Expressao::Texto(s) = e {
                                    Some(s.clone())
                                } else {
                                    None
                                }
                            });
                        if metodo.eh_estatica {
                            if let Some(chave) = &chave_nativa {
                                writeln!(
                                    manifesto,
                                    "DEFINE_STATIC_NATIVE_METHOD {} {} {} {} {}",
                                    fqn,
                                    metodo.nome,
                                    ret,
                                    chave,
                                    params.join(" ")
                                )
                                .ok();
                            } else {
                                writeln!(
                                    manifesto,
                                    "DEFINE_STATIC_METHOD {} {} {} {} {}",
                                    fqn,
                                    metodo.nome,
                                    ret,
                                    params.len(),
                                    params.join(" ")
                                )
                                .ok();
                            }
                        } else {
                            if let Some(chave) = &chave_nativa {
                                writeln!(
                                    manifesto,
                                    "DEFINE_NATIVE_METHOD {} {} {} {} {}",
                                    fqn,
                                    metodo.nome,
                                    ret,
                                    chave,
                                    params.join(" ")
                                )
                                .ok();
                            } else {
                                writeln!(
                                    manifesto,
                                    "DEFINE_METHOD {} {} {} {} {}",
                                    fqn,
                                    metodo.nome,
                                    ret,
                                    params.len(),
                                    params.join(" ")
                                )
                                .ok();
                            }
                        }
                        // Métodos com corpo entram no bytecode
                        if !is_nativo && !metodo.eh_abstrato && !metodo.corpo.is_empty() {
                            // Placeholder - o bytecode completo será gerado abaixo
                            // Não movemos o type_checker aqui
                        }
                    }
                }
            }
        }

        writeln!(manifesto).ok();
        writeln!(manifesto, "[BYTECODE]").ok();

        // Gera o bytecode completo para a seção [BYTECODE]
        let mut gen = bytecode::BytecodeGenerator::new(programa, type_checker);
        let bc = gen.generate_for_library();
        for linha in &bc {
            writeln!(bytecode_secao, "{}", linha).ok();
        }

        Ok(format!("{}\n{}", manifesto, bytecode_secao))
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
