pub mod bytecode;
pub mod cil;
pub mod console;
pub mod llvm_ir;

pub use cil::compilador_il::compilar_cil_para_executavel as compilar_cil_com_dotnet;

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

/* ------------------------------------------------------------------ */
/* TLS vendored (mbedTLS) — biblioteca estática cacheada               */
/* ------------------------------------------------------------------ */

const TLS_STAMP: &str = "mbedtls-3.6.7";

/// Compila (uma vez) ou reutiliza `libporsol_tls.a`: mbedTLS + glue
/// HTTPS do runtime. Retorna None se o toolchain falhar — o backend
/// LLVM continua funcionando sem HTTPS.
fn garantir_lib_tls() -> Option<PathBuf> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let runtime = manifest.join("src").join("runtime");
    let tls_dir = runtime.join("tls");
    let mbedtls = tls_dir.join("mbedtls");
    let include = mbedtls.join("include");
    let everest = mbedtls.join("3rdparty").join("everest");
    if !include.is_dir() || !tls_dir.join("porsol_tls.c").is_file() {
        return None;
    }

    let cache = manifest.join("target").join("tls-cache");
    let obj_dir = cache.join("obj");
    let lib = cache.join("libporsol_tls.a");
    let stamp = cache.join("stamp.txt");

    // Chave de cache: versão do mbedTLS + hash dos fontes do glue
    // (porsol_tls.c/porsol_cacert.c) — mudanças neles invalidam o cache.
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    TLS_STAMP.hash(&mut hasher);
    for f in ["porsol_tls.c", "porsol_cacert.c", "porsol_tls.h"] {
        if let Ok(conteudo) = fs::read(tls_dir.join(f)) {
            conteudo.hash(&mut hasher);
        }
    }
    let stamp_esperado = format!("{:016x}", hasher.finish());
    if lib.is_file()
        && fs::read_to_string(&stamp).map(|s| s.trim() == stamp_esperado).unwrap_or(false)
    {
        return Some(lib);
    }

    // Fontes: toda a library/ do mbedTLS + everest (X25519) + glue.
    let mut fontes: Vec<PathBuf> = Vec::new();
    let lib_dir = mbedtls.join("library");
    if let Ok(entries) = fs::read_dir(&lib_dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.extension().map(|x| x == "c").unwrap_or(false) {
                fontes.push(p);
            }
        }
    }
    // everest: mesmo conjunto do CMake upstream (os .c de kremlib são
    // apenas para o caminho "verified uint128", não usado no clang/gcc).
    for rel in [
        "everest.c",
        "x25519.c",
        "Hacl_Curve25519_joined.c",
    ] {
        let p = everest.join("library").join(rel);
        if p.is_file() {
            fontes.push(p);
        }
    }
    fontes.push(tls_dir.join("porsol_tls.c"));
    fontes.push(tls_dir.join("porsol_cacert.c"));
    // Ordem estável: os objetos são nomeados por índice.
    fontes.sort();
    if fontes.is_empty() {
        return None;
    }

    let clang = clang_executable();
    fs::create_dir_all(&obj_dir).ok()?;
    let includes = [
        include.clone(),
        everest.join("include"),
        everest.join("include").join("everest"),
        everest.join("include").join("everest").join("kremlib"),
        // common.h do mbedTLS (usado também pelas fontes everest).
        lib_dir.clone(),
        runtime.clone(),
        tls_dir.clone(),
    ];

    // Compila cada fonte em paralelo (até 8 jobs).
    let fontes = std::sync::Arc::new(fontes);
    let proximo = std::sync::Arc::new(std::sync::Mutex::new(0usize));
    let falhou = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let clang = std::sync::Arc::new(clang);
    let includes = std::sync::Arc::new(includes);
    let obj_dir = std::sync::Arc::new(obj_dir);
    let mut handles = Vec::new();
    for _ in 0..8 {
        let fontes = std::sync::Arc::clone(&fontes);
        let proximo = std::sync::Arc::clone(&proximo);
        let falhou = std::sync::Arc::clone(&falhou);
        let clang = std::sync::Arc::clone(&clang);
        let includes = std::sync::Arc::clone(&includes);
        let obj_dir = std::sync::Arc::clone(&obj_dir);
        handles.push(std::thread::spawn(move || loop {
            if falhou.load(std::sync::atomic::Ordering::Relaxed) {
                return;
            }
            let idx = {
                let mut g = proximo.lock().unwrap();
                if *g >= fontes.len() {
                    return;
                }
                let i = *g;
                *g += 1;
                i
            };
            let src = &fontes[idx];
            let obj = obj_dir.join(format!("{:04}.o", idx));
            // Reaproveita .o mais novo que o fonte (incremental).
            let precisa = match (fs::metadata(src), fs::metadata(&obj)) {
                (Ok(a), Ok(b)) => a.modified().ok() > b.modified().ok(),
                _ => true,
            };
            if !precisa {
                return;
            }
            let mut cmd = Command::new(clang.as_ref());
            cmd.arg("-c").arg(src).arg("-o").arg(&obj)
                .arg("-O2")
                .arg("-ffunction-sections")
                .arg("-fdata-sections")
                .arg("-w")
                .arg("-D_WIN32_WINNT=0x0601");
            for inc in includes.iter() {
                cmd.arg("-I").arg(inc);
            }
            let ok = cmd.output().map(|o| o.status.success()).unwrap_or(false);
            if !ok {
                falhou.store(true, std::sync::atomic::Ordering::Relaxed);
                return;
            }
        }));
    }
    for h in handles {
        let _ = h.join();
    }
    if falhou.load(std::sync::atomic::Ordering::Relaxed) {
        eprintln!("Aviso: falha ao compilar runtime TLS (mbedTLS); HTTPS indisponível no LLVM.");
        return None;
    }

    // Arquiva estática com llvm-ar (ou ar).
    let ar = clang_executable()
        .parent()
        .map(|d| d.join(if cfg!(windows) { "llvm-ar.exe" } else { "llvm-ar" }))
        .filter(|p| p.is_file())
        .unwrap_or_else(|| PathBuf::from(if cfg!(windows) { "llvm-ar" } else { "ar" }));
    let objs: Vec<PathBuf> = (0..fontes.len())
        .map(|i| obj_dir.join(format!("{:04}.o", i)))
        .collect();
    let saida = Command::new(&ar)
        .arg("rcs")
        .arg(&lib)
        .args(&objs)
        .output();
    match saida {
        Ok(o) if o.status.success() => {
            fs::write(&stamp, &stamp_esperado).ok();
            Some(lib)
        }
        _ => {
            eprintln!("Aviso: falha ao arquivar runtime TLS; HTTPS indisponível no LLVM.");
            None
        }
    }
}

/// Compila o LLVM IR gerado e vincula o runtime nativo de async/await.
/// Se existir LLVM IR da biblioteca padrão, também faz linkagem automática.
///
/// O caminho do runtime é resolvido a partir do manifesto do compilador, para
/// que a compilação funcione independentemente do diretório atual do processo.
pub fn compilar_llvm_ir_com_runtime(
    ll_path: &Path,
    nome_base: &str,
    stdlib_path: Option<&Path>,
) -> Result<(), String> {
    let runtime_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("runtime")
        .join("async_runtime.c");
    let stdlib_runtime_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("runtime")
        .join("porsol_runtime.c");

    if !runtime_path.is_file() {
        return Err(format!(
            "Runtime async nativo não encontrado: {}",
            runtime_path.display()
        ));
    }
    if !stdlib_runtime_path.is_file() {
        return Err(format!(
            "Runtime da biblioteca padrão não encontrado: {}",
            stdlib_runtime_path.display()
        ));
    }

    let mut command = Command::new(clang_executable());
    command.arg(ll_path);
    command.arg(&runtime_path);
    command.arg(&stdlib_runtime_path);
    // Seções por função: permite ao linker remover código não usado
    // (inclui o runtime TLS inteiro quando o programa não usa https).
    command.arg("-ffunction-sections");
    command.arg("-fdata-sections");

    // Se existe LLVM IR da biblioteca padrão, adiciona para linkagem
    if let Some(stdlib_dir) = stdlib_path {
        let stdlib_ll_path = stdlib_dir.join("dist").join("sistema.ll");
        if stdlib_ll_path.exists() {
            println!("  📦 Linkando com biblioteca padrão LLVM IR...");
            command.arg(&stdlib_ll_path);
        }
    }

    // Runtime TLS (mbedTLS) — biblioteca estática cacheada. Só é linkada
    // quando o programa usa ClienteHttp (o dispatch http/https é em runtime,
    // então qualquer uso de ClienteHttp pode receber uma URL https://).
    // Programas sem ClienteHttp não carregam nada de TLS.
    // Só conta chamadas reais: os literais nativos são emitidos como
    // c"ClienteHttp::..." (os declares usam Sistema_Rede_ClienteHttp::).
    let usa_cliente_http = fs::read_to_string(ll_path)
        .map(|ir| ir.contains("c\"ClienteHttp::"))
        .unwrap_or(false);
    if usa_cliente_http {
        if let Some(tls_lib) = garantir_lib_tls() {
            command.arg("-DPOR_DO_SOL_HAS_TLS");
            // Dead-strip do TLS não referenciado: /opt:ref no lld-link/MSVC,
            // --gc-sections no ld ELF.
            if cfg!(windows) {
                command.arg("-Wl,/opt:ref");
                command.arg("-lbcrypt");
            } else {
                command.arg("-Wl,--gc-sections");
            }
            command.arg(&tls_lib);
        }
    }

    let mut output_file = nome_base.to_string();
    #[cfg(windows)]
    if !output_file.ends_with(".exe") {
        output_file.push_str(".exe");
    }

    command.arg("-o").arg(&output_file);

    #[cfg(windows)]
    {
        command.arg("-llegacy_stdio_definitions");
        // Winsock (rede/DNS) e CommandLineToArgvW (Ambiente.Argumentos).
        command.arg("-lws2_32");
        command.arg("-lshell32");
    }

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

        compilar_llvm_ir_com_runtime(Path::new(&ll_path), nome_base, None)
    }

    pub fn gerar_cil<'a>(
        &self,
        programa: &'a ast::Programa,
        type_checker: &'a crate::type_checker::VerificadorTipos<'a>,
        nome_base: &str,
    ) -> Result<String, String> {
        let mut generator = cil::CilGenerator::new(programa, type_checker, nome_base.to_string());
        let code = generator.generate();
        let il_path = format!("{}.il", nome_base);
        // BOM UTF-8: ilasm/ilasm-core interpretam arquivos sem BOM na codepage ANSI,
        // corrompendo literais ldstr com acentos.
        let code_com_bom = format!("\u{FEFF}{}", code);
        fs::write(&il_path, code_com_bom).map_err(|e| e.to_string())?;
        Ok(il_path)
    }

    pub fn compilar_cil_com_dotnet(
        il_path: &Path,
        nome_base: &str,
        output_dir: Option<&Path>,
    ) -> Result<PathBuf, String> {
        cil::compilador_il::compilar_cil_para_executavel(il_path, nome_base, output_dir)
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

    pub fn gerar_bytecode_para_arquivo<'a>(
        &mut self,
        programa: &'a ast::Programa,
        type_checker: &'a crate::type_checker::VerificadorTipos,
        output_path: &Path,
    ) -> Result<(), String> {
        let mut generator = bytecode::BytecodeGenerator::new(programa, type_checker);
        let bytecode = generator.generate();
        fs::write(output_path, bytecode.join("\n")).map_err(|e| e.to_string())
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
                                let flag_async = if metodo.eh_assincrona { " ASSINCRONO" } else { "" };
                                writeln!(
                                    manifesto,
                                    "DEFINE_STATIC_NATIVE_METHOD {} {} {} {}{} {}",
                                    fqn,
                                    metodo.nome,
                                    ret,
                                    chave,
                                    flag_async,
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
                                let flag_async = if metodo.eh_assincrona { " ASSINCRONO" } else { "" };
                                writeln!(
                                    manifesto,
                                    "DEFINE_NATIVE_METHOD {} {} {} {}{} {}",
                                    fqn,
                                    metodo.nome,
                                    ret,
                                    chave,
                                    flag_async,
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

/// Gera apenas o código CIL (.il) em string, sem invocar dotnet ou ilasm.
/// Útil para testes que validam a emissão de CIL puro sem compilação nativa.
pub fn gerar_cil_puro<'a>(
    programa: &'a ast::Programa,
    type_checker: &'a crate::type_checker::VerificadorTipos<'a>,
    assembly_name: &str,
) -> String {
    let mut generator = cil::CilGenerator::new(programa, type_checker, assembly_name.to_string());
    generator.generate()
}

/// Gera LLVM IR para biblioteca (sem função main) e salva em arquivo.
pub fn gerar_llvm_ir_para_biblioteca<'a>(
    programa: &'a ast::Programa,
    type_checker: &'a mut crate::type_checker::VerificadorTipos<'a>,
    nome_base: &str,
) -> Result<(), String> {
    let mut generator =
        llvm_ir::LlvmGenerator::new(programa, type_checker, &type_checker.resolved_classes);
    let code = generator.generate_for_library();
    // nome_base já deve incluir o caminho completo sem extensão
    let ll_path = format!("{}.ll", nome_base);
    fs::write(&ll_path, code).map_err(|e| e.to_string())?;
    Ok(())
}
