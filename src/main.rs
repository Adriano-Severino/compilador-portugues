// src/main.rs

use std::collections::HashSet;
use std::env;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;

// Declaração dos módulos do projeto
mod ast;
mod codegen;
mod inferencia_tipos;
mod interpolacao;
mod lexer;
mod library_loader; // Novo módulo para carregar bibliotecas
mod module_system;
mod ownership;
mod stdlib;
mod type_checker;

// Parser LALRPOP
use lalrpop_util::lalrpop_mod;
lalrpop_mod!(pub parser);
use logos::Logos;

//LLVM (Já funcional):
//cargo run --bin compilador -- teste.pr --target=llvm-ir
//clang teste.ll -o teste
//./teste

//CIL Bytecode:
//cargo run --bin compilador -- teste.pr --target=cil-bytecode
//# Se tiver o 'ilasm' (parte do .NET Framework ou Mono)
//ilasm teste.il /exe /output:teste-cil.exe
//# Para executar (no Windows)
//./teste-cil.exe
//# Ou com Mono
//mono teste-cil.exe

//Console .NET:
//cargo run --bin compilador -- teste.pr --target=console
//cd teste # Entra no diretório do projeto gerado
//dotnet run

//Bytecode Customizado:
//cargo run -- teste.pr --target=bytecode
//cat teste.pbc # Para ver o bytecode gerado

//para executar o bytecode:
//Gere o Bytecode:
//cargo run --bin compilador -- teste.pr --target=bytecode
//cargo run --bin interpretador -- teste.pbc

//help
//cargo run --bin compilador
//cargo run --bin compilador -- --help

// Struct de erro customizada
#[derive(Debug)]
struct CompilerError(String);

impl fmt::Display for CompilerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl std::error::Error for CompilerError {}

// Enum para os alvos de compilação
#[derive(Debug, Clone)]
enum TargetCompilacao {
    Universal,
    LlvmIr,
    CilBytecode,
    Console,
    Bytecode,
    /// Produz um arquivo .pbl (Biblioteca Por do Sol) — API manifest + bytecode
    Biblioteca,
}

//Função para exibir a ajuda
fn exibir_ajuda() {
    print!(
        "Compilador da Linguagem em Português (v0.1.2)
=============================================

Uso: compilador <arquivo.pr>... [OPÇÕES]

OPÇÕES:
  --target=<alvo>               Define o formato de saída da compilação.
  --stdlib-src-path=<path>      Especifica o caminho para o código-fonte da biblioteca padrão.
  --compilar-biblioteca=<path>  Compila uma biblioteca a partir do diretório especificado.
  --help                        Exibe esta mensagem de ajuda.

ALVOS DISPONÍVEIS:
  llvm-ir            Gera código intermediário LLVM (.ll), otimizado para compilação nativa com Clang.
  cil-bytecode       Gera código CIL (.il) para a plataforma .NET.
  console            Cria um projeto de console .NET completo, pronto para ser executado com 'dotnet run'.
  bytecode           Gera um arquivo de bytecode customizado (.pbc) para ser executado pelo interpretador.
  biblioteca         Gera um arquivo de Biblioteca Por do Sol (.pbl) com manifesto de API e bytecode.
  universal          Executa a compilação para todos os alvos disponíveis (padrão).

EXEMPLOS DE USO:
  # Compilar um programa (a biblioteca padrão é encontrada automaticamente)
  cargo run --bin compilador -- exemplos/meu_programa.pr --target=bytecode

  # Compilar a biblioteca padrão para o formato .pbl
  cargo run --bin compilador -- --compilar-biblioteca=../sistema-padrao --target=biblioteca

  # Modo legado: gerar .pbc da biblioteca
  cargo run --bin compilador -- --compilar-biblioteca=../sistema-padrao
"
    );
}

/// Encontra o caminho para o código-fonte da biblioteca padrão.
/// A ordem de prioridade é:
/// 1. Argumento de linha de comando `--stdlib-src-path=<path>`.
/// 2. Variável de ambiente `PORTUGOL_STDLIB_PATH`.
/// 3. Caminho relativo ao executável do compilador (`../../sistema-padrao`).
fn find_stdlib_source_path(args: &[String]) -> Option<PathBuf> {
    // 1. Argumento de linha de comando
    if let Some(path_str) = args
        .iter()
        .find(|arg| arg.starts_with("--stdlib-src-path="))
        .and_then(|arg| arg.split('=').nth(1))
    {
        let path = PathBuf::from(path_str);
        if path.exists() && path.is_dir() {
            return Some(path);
        } else {
            eprintln!(
                "Aviso: O caminho da biblioteca padrão especificado em --stdlib-src-path não existe: {}",
                path.display()
            );
        }
    }

    // 2. Variável de ambiente
    if let Ok(path_str) = env::var("PORTUGOL_STDLIB_PATH") {
        let path = PathBuf::from(path_str);
        if path.exists() && path.is_dir() {
            return Some(path);
        } else {
            eprintln!(
                "Aviso: O caminho da biblioteca padrão especificado em PORTUGOL_STDLIB_PATH não existe: {}",
                path.display()
            );
        }
    }

    // 3. Caminho relativo ao executável
    if let Ok(mut exe_path) = env::current_exe() {
        exe_path.pop(); // Remove o nome do executável
                        // Para debug (target/debug/compilador) e release (target/release/compilador)
        if exe_path.ends_with("debug") || exe_path.ends_with("release") {
            exe_path.pop(); // Remove 'debug' ou 'release'
            exe_path.pop(); // Remove 'target'
        }
        let rel_path = exe_path.join("../sistema-padrao");
        if let Ok(path) = rel_path.canonicalize() {
            if path.exists() && path.is_dir() {
                return Some(path);
            }
        }
    }

    // Fallback para um caminho relativo comum em desenvolvimento
    let dev_path = PathBuf::from("../sistema-padrao");
    if dev_path.exists() && dev_path.is_dir() {
        return Some(dev_path);
    }

    None
}

fn compilar_biblioteca(
    caminho_lib: &Path,
    formato_pbl: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let formato_nome = if formato_pbl { ".pbl" } else { ".pbc" };
    println!(
        "=== Compilando Biblioteca: {} → {} ===",
        caminho_lib.display(), formato_nome
    );

    let caminho_src = caminho_lib.join("src");
    let mut caminhos_arquivos = Vec::new();
    for entrada in WalkDir::new(caminho_src) {
        let entrada = entrada?;
        if entrada.path().extension().and_then(|s| s.to_str()) == Some("pr") {
            caminhos_arquivos.push(entrada.path().to_path_buf());
        }
    }

    if caminhos_arquivos.is_empty() {
        return Err(Box::new(CompilerError(
            "Nenhum arquivo .pr encontrado na biblioteca.".into(),
        )));
    }

    let codigos: Vec<String> = caminhos_arquivos
        .iter()
        .map(|p| fs::read_to_string(p))
        .collect::<Result<_, _>>()?;

    let mut asts = Vec::new();
    for (_caminho, codigo) in caminhos_arquivos.iter().zip(codigos.iter()) {
        let lx = lexer::Token::lexer(codigo);
        let tokens: Vec<_> = lx
            .spanned()
            .map(|(tok, span)| {
                tok.map(|t| (span.start, t, span.end))
                    .map_err(|_| Box::new(CompilerError(format!(
                        "Erro léxico na biblioteca (arquivo {}): posição {}:{}",
                        _caminho.display(), span.start, span.end
                    ))) as Box<dyn std::error::Error>)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut ast = parser::ArquivoParser::new()
            .parse(tokens.iter().cloned())
            .map_err(|e| Box::new(CompilerError(format!(
                "Erro sintático na biblioteca (arquivo {}): {:?}",
                _caminho.display(), e
            ))))?;
        crate::interpolacao::walk_programa(&mut ast, |e| {
            *e = crate::interpolacao::planificar_interpolada(e.clone());
        });
        asts.push(ast);
    }

    let mut programa_final = ast::Programa {
        usings: vec![],
        namespaces: vec![],
        declaracoes: vec![],
    };
    for mut ast in asts {
        programa_final.declaracoes.extend(ast.declaracoes);
        programa_final.usings.extend(ast.usings);
        for ns in ast.namespaces.drain(..) {
            if let Some(existing_ns) = programa_final.namespaces.iter_mut().find(|n| n.nome == ns.nome) {
                existing_ns.declaracoes.extend(ns.declaracoes);
            } else {
                programa_final.namespaces.push(ns);
            }
        }
    }

    let mut tc = type_checker::VerificadorTipos::new();
    // Registra os próprios namespaces da biblioteca como stdlib para não verificar corpos nativos
    for ns in &programa_final.namespaces {
        tc.registrar_namespace_stdlib(&ns.nome);
    }
    if let Err(erros) = tc.verificar_programa(&programa_final) {
        for erro in &erros {
            eprintln!("Erro Semântico na biblioteca: {}", erro);
        }
        return Err(Box::new(CompilerError(
            "Houve erros semânticos na compilação da biblioteca.".into(),
        )));
    }

    let caminho_dist = caminho_lib.join("dist");
    fs::create_dir_all(&caminho_dist)?;

    let mut gerador = codegen::GeradorCodigo::new()?;

    if formato_pbl {
        // Lê o nome e versão do Sistema.toml se disponível
        let (nome_lib, versao_lib) = ler_metadados_biblioteca(caminho_lib);
        let conteudo_pbl = gerador.gerar_pbl(
            &programa_final, &mut tc, &nome_lib, &versao_lib
        )?;
        let caminho_saida = caminho_dist.join(format!("{}.pbl", nome_lib.to_lowercase()));
        fs::write(&caminho_saida, conteudo_pbl)?;
        println!("✅ Biblioteca .pbl gerada em: {}", caminho_saida.display());
    } else {
        let bytecode = gerador.gerar_bytecode_para_biblioteca(&programa_final, &mut tc)?;
        let caminho_saida = caminho_dist.join("sistema.pbc");
        fs::write(&caminho_saida, bytecode)?;
        println!("✅ Biblioteca .pbc gerada em: {}", caminho_saida.display());
    }

    Ok(())
}

/// Lê nome e versão do arquivo `Sistema.toml` (ou equivalente) da biblioteca.
fn ler_metadados_biblioteca(caminho_lib: &Path) -> (String, String) {
    let toml_path = caminho_lib.join("Sistema.toml");
    if let Ok(conteudo) = fs::read_to_string(&toml_path) {
        let mut nome = "Sistema".to_string();
        let mut versao = "1.0.0".to_string();
        for linha in conteudo.lines() {
            if let Some(v) = linha.strip_prefix("nome = ") {
                nome = v.trim_matches('"').to_string();
            }
            if let Some(v) = linha.strip_prefix("versao = ") {
                versao = v.trim_matches('"').to_string();
            }
        }
        (nome, versao)
    } else {
        ("Sistema".to_string(), "1.0.0".to_string())
    }
}

/// Carrega todos os arquivos .pr do sistema-padrão, parseia e retorna um AST combinado.
/// Equivalente a como o compilador C# lê reference assemblies (.dll) para análise semântica:
/// os tipos ficam disponíveis para verificação sem gerar código para eles.
fn carregar_fontes_stdlib(
    stdlib_path: &Path,
) -> Result<(ast::Programa, HashSet<String>), Box<dyn std::error::Error>> {
    let caminho_src = stdlib_path.join("src");
    let mut caminhos_arquivos = Vec::new();
    for entrada in WalkDir::new(&caminho_src) {
        let entrada = entrada?;
        if entrada.path().extension().and_then(|s| s.to_str()) == Some("pr") {
            caminhos_arquivos.push(entrada.path().to_path_buf());
        }
    }

    let mut programa_stdlib = ast::Programa {
        usings: vec![],
        namespaces: vec![],
        declaracoes: vec![],
    };

    // Coleta os namespaces declarados pela stdlib para informar o type_checker
    let mut namespaces_stdlib: HashSet<String> = HashSet::new();

    for caminho in &caminhos_arquivos {
        let codigo = match fs::read_to_string(caminho) {
            Ok(c) => c,
            Err(e) => {
                eprintln!(
                    "Aviso: Falha ao ler arquivo stdlib '{}': {}",
                    caminho.display(),
                    e
                );
                continue;
            }
        };

        let lx = lexer::Token::lexer(&codigo);
        let tokens_result: Result<Vec<_>, _> = lx
            .spanned()
            .map(|(tok, span)| tok.map(|t| (span.start, t, span.end)))
            .collect();

        let tokens = match tokens_result {
            Ok(t) => t,
            Err(_) => {
                eprintln!(
                    "Aviso: Erro léxico em arquivo stdlib '{}', ignorando.",
                    caminho.display()
                );
                continue;
            }
        };

        let mut ast_arquivo = match parser::ArquivoParser::new().parse(tokens.iter().cloned()) {
            Ok(a) => a,
            Err(e) => {
                eprintln!(
                    "Aviso: Erro sintático em arquivo stdlib '{}': {:?}, ignorando.",
                    caminho.display(),
                    e
                );
                continue;
            }
        };

        crate::interpolacao::walk_programa(&mut ast_arquivo, |e| {
            *e = interpolacao::planificar_interpolada(e.clone());
        });

        // Coleta namespaces declarados
        for ns in &ast_arquivo.namespaces {
            namespaces_stdlib.insert(ns.nome.clone());
            // Adiciona também sub-namespaces (ex: "Sistema.Rede" → "Sistema.Rede" e "Sistema")
            let partes: Vec<&str> = ns.nome.split('.').collect();
            let mut acumulado = String::new();
            for parte in &partes {
                if !acumulado.is_empty() {
                    acumulado.push('.');
                }
                acumulado.push_str(parte);
                namespaces_stdlib.insert(acumulado.clone());
            }
        }

        // Mescla no programa stdlib
        programa_stdlib.declaracoes.extend(ast_arquivo.declaracoes);
        programa_stdlib.usings.extend(ast_arquivo.usings);
        for ns in ast_arquivo.namespaces.drain(..) {
            if let Some(ns_existente) = programa_stdlib
                .namespaces
                .iter_mut()
                .find(|n| n.nome == ns.nome)
            {
                ns_existente.declaracoes.extend(ns.declaracoes);
            } else {
                programa_stdlib.namespaces.push(ns);
            }
        }
    }

    println!(
        "📚 Biblioteca padrão carregada: {} namespace(s) — {:?}",
        namespaces_stdlib.len(),
        namespaces_stdlib
    );

    Ok((programa_stdlib, namespaces_stdlib))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("DEBUG: O compilador iniciou a execução.");
    let args: Vec<String> = env::args().collect();

    if let Some(lib_path) = args
        .iter()
        .find(|arg| arg.starts_with("--compilar-biblioteca="))
    {
        let path = lib_path.split('=').nth(1).unwrap_or(".");
        // Produz .pbl se --target=biblioteca (ou --target=pbl) for especificado; caso contrário .pbc legado
        let formato_pbl = args.iter().any(|a| a == "--target=biblioteca" || a == "--target=pbl");
        return compilar_biblioteca(Path::new(path), formato_pbl);
    }

    if args.len() <= 1 || args.contains(&"--help".to_string()) {
        exibir_ajuda();
        return Ok(());
    }

    // Coleta arquivos de entrada do usuário
    let caminhos_arquivos: Vec<PathBuf> = args
        .iter()
        .skip(1)
        .filter(|arg| arg.trim_matches('"').ends_with(".pr"))
        .map(|arg| PathBuf::from(arg.trim_matches('"')))
        .collect();

    if caminhos_arquivos.is_empty() {
        eprintln!("Erro: Nenhum arquivo de entrada (.pr) especificado.");
        exibir_ajuda();
        return Err(Box::new(CompilerError("Nenhum arquivo de entrada".into())));
    }

    // Carrega a biblioteca padrão — strategy:
    //   1. Procura um .pbl pré-compilado em sistema-padrao/dist/ (mais rápido, sem re-parse)
    //   2. Procura um .pbc legado em sistema-padrao/dist/ (compatibilidade)
    //   3. Cai de volta para parsear as fontes .pr (desenvolvimento)
    // Equivalente ao mecanismo de Reference Assemblies do .NET.
    let stdlib_info: Option<(ast::Programa, HashSet<String>, Option<library_loader::Biblioteca>)> = if let Some(stdlib_path) = find_stdlib_source_path(&args) {
        // Tenta carregar .pbl pré-compilado primeiro
        let pbl_path = stdlib_path.join("dist").join("sistema.pbl");
        let pbc_path = stdlib_path.join("dist").join("sistema.pbc");

        if pbl_path.exists() {
            println!("📦 Carregando biblioteca padrão pré-compilada: {}", pbl_path.display());
            match library_loader::carregar_biblioteca(&pbl_path) {
                Ok(bib) => {
                    // Extrai namespaces da biblioteca
                    let mut ns_set: HashSet<String> = HashSet::new();
                    for fqn in bib.simbolos.keys() {
                        // Ex: "Sistema.IO.Arquivo" → "Sistema.IO", "Sistema"
                        let partes: Vec<&str> = fqn.split('.').collect();
                        let mut acum = String::new();
                        for (i, p) in partes.iter().enumerate() {
                            if i == partes.len() - 1 { break; } // ignora o nome da classe
                            if !acum.is_empty() { acum.push('.'); }
                            acum.push_str(p);
                            ns_set.insert(acum.clone());
                        }
                    }
                    // Não carrega no tc_temp para evitar stack overflow
                    // Apenas retorna os namespaces para o type_checker principal
                    let prog_vazio = ast::Programa { usings: vec![], namespaces: vec![], declaracoes: vec![] };
                    Some((prog_vazio, ns_set, Some(bib)))
                }
                Err(e) => {
                    eprintln!("Aviso: Falha ao carregar .pbl: {}. Tentando .pbc...", e);
                    None
                }
            }
        } else if pbc_path.exists() {
            println!("📦 Carregando biblioteca padrão .pbc: {}", pbc_path.display());
            match library_loader::carregar_biblioteca(&pbc_path) {
                Ok(bib) => {
                    let mut ns_set: HashSet<String> = HashSet::new();
                    for fqn in bib.simbolos.keys() {
                        let partes: Vec<&str> = fqn.split('.').collect();
                        let mut acum = String::new();
                        for (i, p) in partes.iter().enumerate() {
                            if i == partes.len() - 1 { break; }
                            if !acum.is_empty() { acum.push('.'); }
                            acum.push_str(p);
                            ns_set.insert(acum.clone());
                        }
                    }
                    let prog_vazio = ast::Programa { usings: vec![], namespaces: vec![], declaracoes: vec![] };
                    Some((prog_vazio, ns_set, Some(bib)))
                }
                Err(e) => {
                    eprintln!("Aviso: Falha ao carregar .pbc: {}. Carregando fontes...", e);
                    None
                }
            }
        } else {
            None
        }
        .or_else(|| {
            // Fallback: parseia os fontes .pr
            match carregar_fontes_stdlib(&stdlib_path) {
                Ok((prog, ns)) => Some((prog, ns, None)),
                Err(e) => {
                    eprintln!("Aviso: Falha ao carregar biblioteca padrão: {}", e);
                    None
                }
            }
        })
    } else {
        eprintln!(
            "Aviso: Diretório do sistema-padrão não encontrado. Compilando sem biblioteca padrão."
        );
        None
    };

    let target = args
        .iter()
        .find(|arg| arg.starts_with("--target="))
        .map(|arg| arg.split('=').nth(1).unwrap_or("universal"))
        .map(|t| match t {
            "llvm-ir" => TargetCompilacao::LlvmIr,
            "cil-bytecode" => TargetCompilacao::CilBytecode,
            "console" => TargetCompilacao::Console,
            "bytecode" => TargetCompilacao::Bytecode,
            "biblioteca" | "pbl" => TargetCompilacao::Biblioteca,
            _ => TargetCompilacao::Universal,
        })
        .unwrap_or(TargetCompilacao::Universal);

    // --- Nova Lógica de Compilação em Fases ---

    // Fase 1: Ler todos os arquivos de código fonte para a memória.
    fn sanitizar_codigo(orig: String) -> String {
        let mut resultado = String::with_capacity(orig.len());
        for linha in orig.lines() {
            let mut corte = linha.len();
            for marcador in [
                ";cargo ",
                ";dotnet ",
                ";clang ",
                ";echo ",
                ";./",
                ";interpretador ",
            ] {
                if let Some(idx) = linha.find(marcador) {
                    if idx < corte {
                        corte = idx + 1; // mantém o ';'
                    }
                }
            }
            if corte < linha.len() {
                resultado.push_str(&linha[..corte]);
            } else {
                resultado.push_str(linha);
            }
            resultado.push('\n');
        }
        resultado
    }

    let codigos: Vec<String> = caminhos_arquivos
        .iter()
        .map(|p| fs::read_to_string(p).map(sanitizar_codigo))
        .collect::<Result<_, _>>()?;

    // Fase 2: Parsear todos os arquivos para ASTs.
    let mut asts = Vec::new();
    for (caminho, codigo) in caminhos_arquivos.iter().zip(codigos.iter()) {
        let lexer = lexer::Token::lexer(codigo);
        let tokens_result: Result<Vec<_>, _> = lexer
            .spanned()
            .map(|(token, span)| token.map(|t| (span.start, t, span.end)))
            .collect();

        let tokens = match tokens_result {
            Ok(tokens) => tokens,
            Err(_) => {
                return Err(Box::new(CompilerError(format!(
                    "Erro Léxico: Token inválido encontrado em '{}'",
                    caminho.display()
                ))))
            }
        };

        let parser = parser::ArquivoParser::new();
        let mut ast = parser.parse(tokens.iter().cloned()).map_err(|e| {
            Box::new(CompilerError(format!(
                "Erro sintático em '{}': {:?}",
                caminho.display(),
                e
            )))
        })?;

        crate::interpolacao::walk_programa(&mut ast, |e| {
            *e = interpolacao::planificar_interpolada(e.clone());
        });
        asts.push(ast);
    }

    // Fase 3: Juntar ASTs para uma análise semântica unificada.
    let mut programa_final = ast::Programa {
        usings: vec![],
        namespaces: vec![],
        declaracoes: vec![],
    };
    for mut ast in asts {
        programa_final.declaracoes.extend(ast.declaracoes);
        programa_final.usings.extend(ast.usings);

        for ns_para_mesclar in ast.namespaces.drain(..) {
            if let Some(ns_existente) = programa_final
                .namespaces
                .iter_mut()
                .find(|n| n.nome == ns_para_mesclar.nome)
            {
                ns_existente.declaracoes.extend(ns_para_mesclar.declaracoes);
            } else {
                programa_final.namespaces.push(ns_para_mesclar);
            }
        }
    }

    // Fase 4: Análise semântica no AST combinado.
    let mut type_checker = type_checker::VerificadorTipos::new();

    // Fase 3.5: Injetar stdlib no contexto de tipo — dois modos:
    //   a) Via .pbl/.pbc pré-compilado: stdlib_tc já tem os tipos carregados; apenas regista namespaces
    //   b) Via fontes .pr: mescla namespaces/declarações no AST para análise semântica unificada
    let stdlib_namespaces: HashSet<String>;

    if let Some((programa_stdlib, ns_stdlib, bib_opt)) = stdlib_info {
        stdlib_namespaces = ns_stdlib;

        // Passa biblioteca externa para o type_checker se disponível
        if let Some(ref bib) = bib_opt {
            type_checker.definir_biblioteca_externa(bib.clone());
        }

        // Se não houver biblioteca externa, usa modo legado (carregar fontes .pr)
        if bib_opt.is_none() {
            // Modo fonte: mescla no AST (comportamento legado)
            for ns in programa_stdlib.namespaces {
                if let Some(ns_existente) = programa_final
                    .namespaces
                    .iter_mut()
                    .find(|n| n.nome == ns.nome)
                {
                    ns_existente.declaracoes.extend(ns.declaracoes);
                } else {
                    programa_final.namespaces.push(ns);
                }
            }
            programa_final.declaracoes.extend(programa_stdlib.declaracoes);
        }
    } else {
        stdlib_namespaces = HashSet::new();
    }

    // Informa ao verificador de tipos quais namespaces pertencem à stdlib
    for ns in &stdlib_namespaces {
        type_checker.registrar_namespace_stdlib(ns);
        eprintln!("DEBUG: Registrando namespace stdlib: {}", ns);
    }

    // NOTA: Não chamamos carregar_biblioteca no type_checker principal para evitar stack overflow
    // O type_checker confia que tipos de namespaces stdlib são válidos (verificado em eh_classe_stdlib)
    // Os metadados da biblioteca estão em stdlib_biblioteca e podem ser usados na geração de código se necessário

    if let Err(erros) = type_checker.verificar_programa(&programa_final) {
        for erro in erros {
            eprintln!("Erro Semântico: {}", erro);
        }
        return Err(Box::new(CompilerError(
            "Houve erros semânticos.".to_string(),
        )));
    }

    // Fase 5: Geração de código.
    let nome_base = caminhos_arquivos
        .last() // Usa o último arquivo (provavelmente o principal do usuário) para o nome base
        .and_then(|p| p.file_stem())
        .and_then(|s| s.to_str())
        .unwrap_or("saida");

    match target {
        TargetCompilacao::Universal => {
            compilar_universal(&programa_final, &mut type_checker, nome_base)
        }
        TargetCompilacao::LlvmIr => {
            compilar_para_llvm_ir(&programa_final, &mut type_checker, nome_base)?;
            println!("Compilando com clang...");
            let output = Command::new("clang")
                .arg(format!("{}.ll", nome_base))
                .arg("-o")
                .arg(nome_base)
                .output()
                .map_err(|e| Box::new(CompilerError(format!("Falha ao executar clang: {}", e))))?;

            if !output.status.success() {
                return Err(Box::new(CompilerError(format!(
                    "Erro ao compilar LLVM IR com clang:\nstdout: {}\nstderr: {}\n",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                ))));
            }
            println!("Executável gerado: ./{}", nome_base);
            Ok(())
        }
        TargetCompilacao::CilBytecode => compilar_para_cil_bytecode(&programa_final, nome_base),
        TargetCompilacao::Console => compilar_para_console(&programa_final, nome_base),
        TargetCompilacao::Bytecode => {
            compilar_para_bytecode(&programa_final, &mut type_checker, nome_base)
        }
        TargetCompilacao::Biblioteca => {
            // Produz .pbl a partir dos arquivos de entrada (usa a própria lógica de biblioteca)
            let mut gerador = codegen::GeradorCodigo::new()?;
            let conteudo = gerador.gerar_pbl(
                &programa_final, &mut type_checker, nome_base, "1.0.0"
            )?;
            let caminho_saida = format!("{}.pbl", nome_base);
            fs::write(&caminho_saida, conteudo)?;
            println!("✅ Biblioteca .pbl gerada em: {}", caminho_saida);
            Ok(())
        }
    }
}

fn compilar_universal<'a>(
    ast: &'a ast::Programa,
    type_checker: &'a mut type_checker::VerificadorTipos<'a>,
    nome_base: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🌍 Iniciando Compilação Universal...");
    compilar_para_llvm_ir(ast, &mut type_checker.clone(), nome_base)?;
    compilar_para_cil_bytecode(ast, nome_base)?;
    compilar_para_console(ast, nome_base)?;
    compilar_para_bytecode(ast, type_checker, nome_base)?;
    println!("\n🎉 Compilação Universal Concluída!");
    Ok(())
}

fn compilar_para_llvm_ir<'a>(
    programa: &'a ast::Programa,
    type_checker: &'a mut type_checker::VerificadorTipos<'a>,
    nome_base: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔧 Gerando LLVM IR...");
    let mut gerador = codegen::llvm_ir::LlvmGenerator::new(
        programa,
        type_checker,
        &type_checker.resolved_classes,
    );
    let llvm_ir = gerador.generate();
    fs::write(format!("{}.ll", nome_base), llvm_ir)?;
    println!("  ✓ {}.ll gerado.", nome_base);
    println!("  Para compilar: clang {0}.ll -o {0}", nome_base);
    println!("🎯 Pipeline LLVM: AST → LLVM IR → Código de Máquina");
    println!("Para executar: ./{}", nome_base);
    Ok(())
}

fn compilar_para_cil_bytecode<'a>(
    ast: &'a ast::Programa,
    nome_base: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔧 Gerando CIL Bytecode...");
    let gerador = codegen::GeradorCodigo::new()?;
    gerador
        .gerar_cil(ast, nome_base)
        .map_err(|e| Box::new(CompilerError(e)))?;
    println!("  ✓ {}.il gerado.", nome_base);
    println!(
        "  Para compilar: ilasm {0}.il /exe /output:{0}.exe",
        nome_base
    );
    Ok(())
}

fn compilar_para_console<'a>(
    ast: &'a ast::Programa,
    nome_base: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔧 Gerando Projeto de Console .NET...");
    let gerador = codegen::GeradorCodigo::new()?;
    gerador
        .gerar_console(ast, nome_base)
        .map_err(|e| Box::new(CompilerError(e)))?;
    println!("  ✓ Projeto '{}' gerado.", nome_base);
    println!("  Para executar: cd {} && dotnet run", nome_base);
    Ok(())
}

fn compilar_para_bytecode<'a>(
    ast: &'a ast::Programa,
    type_checker: &'a mut type_checker::VerificadorTipos,
    nome_base: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔧 Gerando Bytecode Customizado...");
    let mut gerador = codegen::GeradorCodigo::new()?;
    gerador
        .gerar_bytecode(ast, type_checker, nome_base)
        .map_err(|e| Box::new(CompilerError(e)))?;
    println!("  ✓ {}.pbc gerado.", nome_base);
    println!(" ✓ Executando o bytecode...");
    println!("Você pode executar o bytecode usando o interpretador personalizado.");
    println!(
        "Execute: cargo run --bin interpretador -- {}.pbc",
        nome_base
    );
    println!("ou use o comando:");
    println!("Para executar: interpretador {}.pbc", nome_base);
    Ok(())
}
