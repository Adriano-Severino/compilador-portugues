mod lexer;
mod ast;
mod codegen;
mod type_checker;
mod ownership;
mod module_system;
mod stdlib;

lalrpop_mod!(parser);

use std::fs;
use logos::Logos;
use inkwell::context::Context;
use lalrpop_util::lalrpop_mod;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Uso: {} <comando> [argumentos]", args[0]);
        eprintln!("Comandos:");
        eprintln!("  compilar <arquivo.pr>     - Compila um arquivo");
        eprintln!("  verificar <arquivo.pr>    - Verifica tipos e ownership");
        eprintln!("  modulos <diretorio>       - Lista módulos disponíveis");
        std::process::exit(1);
    }

    match args[1].as_str() {
        "compilar" => {
            if args.len() < 3 {
                eprintln!("Uso: {} compilar <arquivo.pr>", args[0]);
                std::process::exit(1);
            }
            compilar_arquivo(&args[2])
        },
        "verificar" => {
            if args.len() < 3 {
                eprintln!("Uso: {} verificar <arquivo.pr>", args[0]);
                std::process::exit(1);
            }
            verificar_arquivo(&args[2])
        },
        "modulos" => {
            let diretorio = if args.len() > 2 { &args[2] } else { "." };
            listar_modulos(diretorio)
        },
        _ => {
            eprintln!("Comando desconhecido: {}", args[1]);
            std::process::exit(1);
        }
    }
}

fn compilar_arquivo(caminho_arquivo: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Compilando {} ===", caminho_arquivo);

    let codigo = fs::read_to_string(caminho_arquivo)?;

    // 1. Análise Léxica
    println!("1. Análise Léxica...");
    let lex = lexer::Token::lexer(&codigo);
    let tokens: Vec<_> = lex.spanned()
        .filter_map(|(tok_res, span)| {
            match tok_res {
                Ok(tok) => Some((span.start, tok, span.end)),
                Err(e) => {
                    eprintln!("Erro léxico na posição {}: {:?}", span.start, e);
                    None
                }
            }
        })
        .collect();

    if tokens.is_empty() {
        return Err("Nenhum token válido encontrado".into());
    }

    println!("   {} tokens processados", tokens.len());

    // 2. Análise Sintática
    println!("2. Análise Sintática...");
    // CORRIGIDO: Usar parser::ArquivoParser
    let parser = parser::ArquivoParser::new();
    let mut ast = parser.parse(tokens.iter().cloned())
        .map_err(|e| format!("Erro sintático: {:?}", e))?;

    // 3. Adicionar biblioteca padrão
    println!("3. Carregando biblioteca padrão...");
    let mut stdlib = stdlib::criar_biblioteca_padrao();
    ast.declaracoes.append(&mut stdlib);

    // 4. Sistema de Módulos
    println!("4. Resolvendo módulos...");
    let _sistema_modulos = module_system::SistemaModulos::new();

    // 5. Verificação de Tipos
    println!("5. Verificação de tipos...");
    let mut verificador_tipos = type_checker::VerificadorTipos::new();
    match verificador_tipos.verificar_programa(&ast) {
        Ok(()) => println!("   ✓ Tipos verificados com sucesso"),
        Err(erros) => {
            eprintln!("   ✗ Erros de tipo encontrados:");
            for erro in &erros {
                eprintln!("     - {}", erro);
            }
            return Err("Falha na verificação de tipos".into());
        }
    }

    // 6. Análise de Ownership
    println!("6. Análise de ownership...");
    let mut analisador_ownership = ownership::AnalisadorOwnership::new();
    match analisador_ownership.analisar_programa(&ast) {
        Ok(warnings) => {
            println!("   ✓ Ownership verificado com sucesso");
            if !warnings.is_empty() {
                println!("   Avisos:");
                for warning in &warnings {
                    println!("     - {}", warning);
                }
            }
        },
        Err(erros) => {
            eprintln!("   ✗ Erros de ownership encontrados:");
            for erro in &erros {
                eprintln!("     - {}", erro);
            }
            return Err("Falha na análise de ownership".into());
        }
    }
    
    // 7. Geração de Código LLVM
    println!("7. Geração de código...");
    let context = Context::create();
    let gerador = codegen::GeradorCodigo::new(&context);
    
    let i32_type = context.i32_type();
    let function_type = i32_type.fn_type(&[], false);
    let function = gerador.module.add_function("main", function_type, None);
    let basic_block = context.append_basic_block(function, "entry");
    gerador.builder.position_at_end(basic_block);

    // Compilar o programa
    gerador.compilar_programa(&ast)?;
    let _ = gerador.builder.build_return(Some(&i32_type.const_int(0, false)));

    // 8. Verificação e Saída
    println!("8. Verificação final...");
    gerador.module.verify()
        .map_err(|e| format!("Erro na verificação do módulo LLVM: {}", e))?;

    let output_path = format!("{}.ll", caminho_arquivo.trim_end_matches(".pr"));
    gerador.module.print_to_file(&output_path)
        .map_err(|e| format!("Erro ao escrever arquivo: {}", e))?;

    println!("✓ Compilação concluída! Arquivo gerado: {}", output_path);
    println!("\nPara executar:");
    println!("  clang {} -o {}", output_path, caminho_arquivo.trim_end_matches(".pr"));
    println!("  ./{}", caminho_arquivo.trim_end_matches(".pr"));

    Ok(())
}

fn verificar_arquivo(caminho_arquivo: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Verificando {} ===", caminho_arquivo);

    let codigo = fs::read_to_string(caminho_arquivo)?;

    // Análise básica
    let lex = lexer::Token::lexer(&codigo);
    let tokens: Vec<_> = lex.spanned()
        .filter_map(|(tok_res, span)| tok_res.ok().map(|tok| (span.start, tok, span.end)))
        .collect();

    let parser = parser::ArquivoParser::new();
    let mut ast = parser.parse(tokens.iter().cloned())
        .map_err(|e| format!("Erro sintático: {:?}", e))?;

    // Adicionar stdlib para verificação
    let mut stdlib = stdlib::criar_biblioteca_padrao();
    ast.declaracoes.append(&mut stdlib);

    // Verificar tipos
    let mut verificador_tipos = type_checker::VerificadorTipos::new();
    let mut tem_erros = false;

    match verificador_tipos.verificar_programa(&ast) {
        Ok(()) => println!("✓ Tipos: OK"),
        Err(erros) => {
            println!("✗ Tipos: {} erro(s)", erros.len());
            for erro in &erros {
                println!("  - {}", erro);
            }
            tem_erros = true;
        }
    }

    // Verificar ownership
    let mut analisador_ownership = ownership::AnalisadorOwnership::new();
    match analisador_ownership.analisar_programa(&ast) {
        Ok(warnings) => {
            println!("✓ Ownership: OK");
            if !warnings.is_empty() {
                println!("  Avisos:");
                for warning in &warnings {
                    println!("    - {}", warning);
                }
            }
        },
        Err(erros) => {
            println!("✗ Ownership: {} erro(s)", erros.len());
            for erro in &erros {
                println!("  - {}", erro);
            }
            tem_erros = true;
        }
    }

    if tem_erros {
        Err("Verificação falhou".into())
    } else {
        println!("✓ Arquivo válido!");
        Ok(())
    }
}

fn listar_modulos(diretorio: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Módulos em {} ===", diretorio);

    let entries = fs::read_dir(diretorio)?;
    let mut modulos = Vec::new();

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if let Some(ext) = path.extension() {
            if ext == "pr" {
                if let Some(nome) = path.file_stem().and_then(|s| s.to_str()) {
                    modulos.push(nome.to_string());
                }
            }
        }
    }

    if modulos.is_empty() {
        println!("Nenhum módulo encontrado");
    } else {
        modulos.sort();
        for modulo in modulos {
            println!("  📄 {}", modulo);
        }
    }

    Ok(())
}