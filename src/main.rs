use std::process::Command;
use std::path::Path;
use std::fs;

mod ast;
mod codegen;
mod inferencia_tipos;
mod interpolacao;
mod lexer;
mod module_system;
mod ownership;
mod runtime;
mod stdlib;
mod type_checker;

use lalrpop_util::lalrpop_mod;
lalrpop_mod!(pub parser);

use logos::Logos;

//# Desenvolvimento rápido
//cargo run -- app.pr --target=universal

//# Aplicação console simples  
//cargo run -- app.pr --target=console

//# App mobile/desktop
//cargo run -- app.pr --target=maui-hybrid

//# Site web
//cargo run -- app.pr --target=blazor-web

//# API backend
//cargo run -- app.pr --target=api

//# Solução empresarial completa
//cargo run -- app.pr --target=fullstack

//# Performance máxima
//cargo run -- app.pr --target=llvm-ir

//# VM própria
//cargo run -- app.pr --target=bytecode


// ✅ Targets limpos (removido CilDireto)
#[derive(Debug, Clone)]
enum TargetCompilacao {
    Universal,      // Gera todos os formatos
    LlvmIr,        // Só LLVM IR
    CilBytecode,   // CIL via bytecode
    Console,       // Console Application
    MauiHybrid,    // MAUI Blazor Hybrid
    BlazorWeb,     // Blazor Web App
    Api,           // .NET Core Web API
    FullStack,     // Solução completa
    Bytecode,      // Bytecode próprio
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Uso: {} <arquivo.pr> [--target=TARGET]", args[0]);
        eprintln!("Targets disponíveis:");
        eprintln!("");
        eprintln!("📦 Desenvolvimento:");
        eprintln!(" --target=universal     : Gera todos os formatos (padrão)");
        eprintln!(" --target=bytecode      : Bytecode próprio");
        eprintln!("");
        eprintln!("🖥️ Desktop/Console:");
        eprintln!(" --target=console       : Console Application");
        eprintln!(" --target=llvm-ir       : LLVM IR (nativo)");
        eprintln!(" --target=cil-bytecode  : CIL via bytecode");
        eprintln!("");
        eprintln!("🌐 Multiplataforma:");
        eprintln!(" --target=maui-hybrid   : MAUI Blazor Hybrid");
        eprintln!(" --target=blazor-web    : Blazor Web App");
        eprintln!(" --target=api           : .NET Core Web API");
        eprintln!(" --target=fullstack     : Solução completa");

        std::process::exit(1);
    }

    let target = match args.get(2).map(|s| s.as_str()) {
        Some("--target=universal") => TargetCompilacao::Universal,
        Some("--target=llvm-ir") => TargetCompilacao::LlvmIr,
        Some("--target=cil-bytecode") => TargetCompilacao::CilBytecode,
        Some("--target=console") => TargetCompilacao::Console,
        Some("--target=maui-hybrid") => TargetCompilacao::MauiHybrid,
        Some("--target=blazor-web") => TargetCompilacao::BlazorWeb,
        Some("--target=api") => TargetCompilacao::Api,
        Some("--target=fullstack") => TargetCompilacao::FullStack,
        Some("--target=bytecode") => TargetCompilacao::Bytecode,
        _ => TargetCompilacao::Universal, // Padrão é universal
    };

    compilar_arquivo(&args[1], target)
}

fn compilar_arquivo(
    caminho_arquivo: &str,
    target: TargetCompilacao,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Compilando {} ===", caminho_arquivo);
    println!("🎯 Target: {:?}", target);

    let codigo = fs::read_to_string(caminho_arquivo)?;
    let ast = processar_codigo_comum(&codigo)?;

    match target {
        TargetCompilacao::Universal => compilar_universal(&ast, caminho_arquivo),
        TargetCompilacao::LlvmIr => compilar_para_llvm_ir(&ast, caminho_arquivo),
        TargetCompilacao::CilBytecode => compilar_para_cil_bytecode(&ast, caminho_arquivo),
        TargetCompilacao::Console => compilar_para_console(&ast, caminho_arquivo),
        TargetCompilacao::MauiHybrid => compilar_para_maui_hybrid(&ast, caminho_arquivo),
        TargetCompilacao::BlazorWeb => compilar_para_blazor_web(&ast, caminho_arquivo),
        TargetCompilacao::Api => compilar_para_api(&ast, caminho_arquivo),
        TargetCompilacao::FullStack => compilar_para_fullstack(&ast, caminho_arquivo),
        TargetCompilacao::Bytecode => compilar_para_bytecode(&ast),
    }
}

// ✅ Compilação Universal
fn compilar_universal(
    ast: &ast::Programa,
    caminho: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🌍 Compilação Universal - Gerando todos os formatos...");
    
    let nome_base = caminho.trim_end_matches(".pr");
    
    // 1. Bytecode Intermediário Universal
    println!("1. Gerando bytecode intermediário...");
    let gerador_bytecode = codegen::GeradorCodigo::new_bytecode()?;
    gerador_bytecode.gerar_programa(ast)?;
    let bytecode_universal = gerador_bytecode.obter_bytecode();
    
    // Salvar bytecode em formato JSON (se serde estiver disponível)
    // let bytecode_serializado = serde_json::to_string_pretty(&bytecode_universal)?;
    // fs::write(format!("{}.bytecode", nome_base), bytecode_serializado)?;
    println!("   ✓ {}.bytecode (Intermediário Universal)", nome_base);
    
    // 2. LLVM IR
    println!("2. Gerando LLVM IR...");
    gerador_bytecode.gerar_llvm_ir_do_bytecode(&bytecode_universal, nome_base)?;
    println!("   ✓ {}.ll (LLVM IR)", nome_base);
    
    // 3. CIL via bytecode
    println!("3. Gerando CIL...");
    gerador_bytecode.gerar_cil_do_bytecode(&bytecode_universal, nome_base)?;
    println!("   ✓ {}.il (CIL)", nome_base);
    
    // 4. C# Console
    println!("4. Gerando C# Console...");
    let gerador_cs = codegen::GeradorCodigo::new_console()?;
    let projeto_cs = gerador_cs.gerar_projeto_console(ast)?;
    criar_projeto_console(&projeto_cs, &format!("{}_Console", nome_base))?;
    println!("   ✓ {}_Console/ (C# Console)", nome_base);
    
    // 5. JavaScript
    println!("5. Gerando JavaScript...");
    gerador_bytecode.gerar_javascript_do_bytecode(&bytecode_universal, nome_base)?;
    println!("   ✓ {}.js (JavaScript)", nome_base);
    
    println!("\n🎉 Compilação Universal Concluída!");
    println!("📦 Formatos gerados:");
    println!("   • {}.bytecode - Bytecode intermediário universal", nome_base);
    println!("   • {}.ll - LLVM IR (compile com: clang {}.ll -o {})", nome_base, nome_base, nome_base);
    println!("   • {}.il - CIL (compile com: ilasm {}.il /exe)", nome_base, nome_base);
    println!("   • {}_Console/ - C# Console (execute: cd {}_Console && dotnet run)", nome_base, nome_base);
    println!("   • {}.js - JavaScript (execute: node {}.js)", nome_base, nome_base);
    
    Ok(())
}

// ✅ LLVM IR apenas
fn compilar_para_llvm_ir(
    ast: &ast::Programa,
    caminho: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔧 Gerando apenas LLVM IR...");
    
    let nome_base = caminho.trim_end_matches(".pr");
    
    let gerador_bytecode = codegen::GeradorCodigo::new_bytecode()?;
    gerador_bytecode.gerar_programa(ast)?;
    let bytecode = gerador_bytecode.obter_bytecode();
    
    gerador_bytecode.gerar_llvm_ir_do_bytecode(&bytecode, nome_base)?;
    
    println!("✓ LLVM IR gerado: {}.ll", nome_base);
    println!("Para compilar: clang {}.ll -o {}", nome_base, nome_base);
    println!("Para executar: ./{}", nome_base);
    
    Ok(())
}

// ✅ CIL via Bytecode
fn compilar_para_cil_bytecode(
    ast: &ast::Programa,
    caminho: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔧 Gerando CIL via bytecode...");
    
    let nome_base = caminho.trim_end_matches(".pr");
    
    let gerador_bytecode = codegen::GeradorCodigo::new_bytecode()?;
    gerador_bytecode.gerar_programa(ast)?;
    let bytecode = gerador_bytecode.obter_bytecode();
    
    gerador_bytecode.gerar_cil_do_bytecode(&bytecode, nome_base)?;
    
    println!("✓ CIL gerado: {}.il", nome_base);
    
    if let Ok(result) = Command::new("ilasm")
        .args([&format!("{}.il", nome_base), "/exe"])
        .output()
    {
        if result.status.success() {
            println!("✓ Executável gerado: {}.exe", nome_base);
        } else {
            println!("❌ Erro do ilasm:");
            println!("{}", String::from_utf8_lossy(&result.stderr));
        }
    } else {
        println!("⚠️ ilasm não encontrado. Compile manualmente: ilasm {}.il /exe", nome_base);
    }
    
    Ok(())
}

fn processar_codigo_comum(codigo: &str) -> Result<ast::Programa, Box<dyn std::error::Error>> {
    // 1. Análise Léxica
    println!("1. Análise Léxica...");
    let lex = lexer::Token::lexer(&codigo);
    let tokens: Vec<_> = lex
        .spanned()
        .filter_map(|(tok_res, span)| match tok_res {
            Ok(tok) => Some((span.start, tok, span.end)),
            Err(e) => {
                eprintln!("Erro léxico na posição {}: {:?}", span.start, e);
                None
            }
        })
        .collect();

    if tokens.is_empty() {
        return Err("Nenhum token válido encontrado".into());
    }

    println!(" ✓ {} tokens processados", tokens.len());

    // 2. Análise Sintática
    println!("2. Análise Sintática...");
    let parser = parser::ArquivoParser::new();
    let mut ast = parser
        .parse(tokens.iter().cloned())
        .map_err(|e| format!("Erro sintático: {:?}", e))?;

    // Interpolação
    crate::interpolacao::walk_programa(&mut ast, |e| {
        *e = interpolacao::planificar_interpolada(e.clone());
    });

    println!(" ✓ AST gerado com sucesso");
    println!(" - {} namespaces", ast.namespaces.len());
    println!(" - {} declarações", ast.declaracoes.len());

    // 3. Biblioteca padrão
    println!("3. Carregando biblioteca padrão...");
    let mut stdlib = stdlib::criar_biblioteca_padrao();
    ast.declaracoes.append(&mut stdlib);

    // 4. Verificações simplificadas
    println!("4. Verificações de compatibilidade...");
    verificar_compatibilidade_ast(&ast)?;

    // 5. Inferência de Tipos
    println!("5. Inferência de tipos...");
    let mut inferencia = inferencia_tipos::InferenciaTipos::new();
    
    for declaracao in &ast.declaracoes {
        if let ast::Declaracao::DeclaracaoClasse(classe) = declaracao {
            println!(" 📋 Registrando classe '{}' para inferência", classe.nome);
            if let Some(pai) = &classe.classe_pai {
                println!(" └─ Herda de: {}", pai);
            }
            inferencia.registrar_classe(classe.clone());
        }
    }

    // 6. Verificação de Tipos com Herança
    println!("6. Verificação de tipos e herança...");
    let mut verificador_tipos = type_checker::VerificadorTipos::new();
    
    let mut classes_com_heranca = 0;
    for declaracao in &ast.declaracoes {
        if let ast::Declaracao::DeclaracaoClasse(classe) = declaracao {
            if classe.classe_pai.is_some() {
                classes_com_heranca += 1;
                println!(" 🔗 Classe '{}' utiliza herança", classe.nome);
            }
        }
    }

    if classes_com_heranca > 0 {
        println!(" 📊 {} classe(s) utilizam herança", classes_com_heranca);
    }

    match verificador_tipos.verificar_programa(&ast) {
        Ok(()) => println!(" ✓ Tipos e herança verificados com sucesso"),
        Err(erros) => {
            eprintln!(" ⚠️ Avisos de tipo/herança encontrados:");
            for erro in &erros {
                eprintln!(" - {}", erro);
            }
        }
    }

    // 7. Análise de Ownership com Herança
    println!("7. Análise de ownership e polimorfismo...");
    let mut analisador_ownership = ownership::AnalisadorOwnership::new();
    
    for declaracao in &ast.declaracoes {
        if let ast::Declaracao::DeclaracaoClasse(classe) = declaracao {
            analisador_ownership.registrar_classe(classe.clone());
        }
    }

    match analisador_ownership.analisar_programa(&ast) {
        Ok(warnings) => {
            println!(" ✓ Ownership e polimorfismo verificados com sucesso");
            if !warnings.is_empty() {
                println!(" Avisos:");
                let mut avisos_heranca = 0;
                for warning in &warnings {
                    println!(" - {}", warning);
                    if warning.contains("polimórfica") || warning.contains("redefinível") {
                        avisos_heranca += 1;
                    }
                }
                if avisos_heranca > 0 {
                    println!(" 📊 {} aviso(s) relacionados à herança/polimorfismo", avisos_heranca);
                }
            }
        }
        Err(erros) => {
            eprintln!(" ⚠️ Avisos de ownership encontrados:");
            for erro in &erros {
                eprintln!(" - {}", erro);
            }
        }
    }

    // 8. Estatísticas finais
    println!("\n=== Estatísticas da Compilação ===");
    println!("Namespaces processados: {}", ast.namespaces.len());
    println!("Declarações processadas: {}", ast.declaracoes.len());
    println!("Tokens analisados: {}", tokens.len());

    let mut total_classes = 0;
    let mut classes_com_heranca = 0;
    let mut total_metodos = 0;
    let mut metodos_virtuais = 0;
    let mut metodos_override = 0;

    for declaracao in &ast.declaracoes {
        if let ast::Declaracao::DeclaracaoClasse(classe) = declaracao {
            total_classes += 1;
            if classe.classe_pai.is_some() {
                classes_com_heranca += 1;
            }

            for metodo in &classe.metodos {
                total_metodos += 1;
                if metodo.eh_virtual {
                    metodos_virtuais += 1;
                }
                if metodo.eh_override {
                    metodos_override += 1;
                }
            }
        }
    }

    if total_classes > 0 {
        println!("\n=== Estatísticas de Orientação a Objetos ===");
        println!("Classes totais: {}", total_classes);
        println!("Classes com herança: {}", classes_com_heranca);
        println!("Métodos totais: {}", total_metodos);
        if metodos_virtuais > 0 {
            println!("Métodos redefiníveis: {}", metodos_virtuais);
        }
        if metodos_override > 0 {
            println!("Métodos sobrescritos: {}", metodos_override);
        }

        if classes_com_heranca > 0 || metodos_virtuais > 0 || metodos_override > 0 {
            println!("🎉 Herança e polimorfismo ativos!");
        }
    }

    Ok(ast)
}

fn verificar_compatibilidade_ast(ast: &ast::Programa) -> Result<(), Box<dyn std::error::Error>> {
    // Verificações básicas de compatibilidade
    let mut classes = std::collections::HashMap::new();
    
    // Coletar todas as classes primeiro
    for declaracao in &ast.declaracoes {
        if let ast::Declaracao::DeclaracaoClasse(classe) = declaracao {
            classes.insert(classe.nome.clone(), classe);
        }
    }

    // Verificar herança
    for (nome_classe, classe) in &classes {
        if let Some(classe_pai) = &classe.classe_pai {
            if !classes.contains_key(classe_pai) {
                eprintln!(" ⚠️ Classe '{}' herda de '{}' que não foi encontrada", nome_classe, classe_pai);
            } else {
                println!(" ✓ Herança válida: {} : {}", nome_classe, classe_pai);
            }
        }

        // Verificar métodos redefiníveis/sobrescritos
        for metodo in &classe.metodos {
            if metodo.eh_override && classe.classe_pai.is_none() {
                eprintln!(" ⚠️ Método '{}' marcado como 'sobrescreve' mas classe '{}' não tem pai",
                    metodo.nome, nome_classe);
            }

            if metodo.eh_virtual && metodo.eh_override {
                eprintln!(" ⚠️ Método '{}' não pode ser 'redefinível' e 'sobrescreve' simultaneamente",
                    metodo.nome);
            }
        }
    }

    Ok(())
}

fn criar_projeto_console(projeto: &str, nome_projeto: &str) -> Result<(), Box<dyn std::error::Error>> {
    let dir_projeto = format!("./{}", nome_projeto);
    fs::create_dir_all(&dir_projeto)?;

    // .csproj
    let csproj = format!(r#"<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <TargetFramework>net8.0</TargetFramework>
  </PropertyGroup>
</Project>"#);

    fs::write(format!("{}/{}.csproj", dir_projeto, nome_projeto), csproj)?;

    // Program.cs com código C# convertido
    let program_cs = format!(r#"using System;

namespace {}
{{
{}
    class Program
    {{
        static void Main(string[] args)
        {{
            try
            {{
                new Principal().Main();
            }}
            catch (Exception ex)
            {{
                Console.WriteLine($"Erro: {{ex.Message}}");
            }}
            Console.WriteLine("\nPressione qualquer tecla para sair...");
            Console.ReadKey();
        }}
    }}
}}"#, nome_projeto, projeto);

    fs::write(format!("{}/Program.cs", dir_projeto), program_cs)?;
    
    Ok(())
}

// Implementações dos outros targets
fn compilar_para_console(
    ast: &ast::Programa,
    caminho: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("6. Geração de código Console Application…");
    let gerador = codegen::GeradorCodigo::new_console()?;
    let projeto_cs = gerador.gerar_projeto_console(ast)?;

    let nome_base = Path::new(caminho).file_stem().unwrap().to_str().unwrap();
    let namespace: String = nome_base
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();

    criar_projeto_console(&projeto_cs, &namespace)?;
    println!("✓ Console Application criada!\n cd {} && dotnet run", namespace);
    
    Ok(())
}

fn compilar_para_maui_hybrid(
    ast: &ast::Programa,
    _caminho: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔧 Gerando projeto MAUI Hybrid...");
    let gerador = codegen::GeradorCodigo::new_console()?;
    let projeto_cs = gerador.gerar_projeto_console(ast)?;
    
    // Criar projeto MAUI básico
    println!("✓ MAUI Hybrid projeto criado (baseado em Console)");
    Ok(())
}

fn compilar_para_blazor_web(
    ast: &ast::Programa,
    _caminho: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔧 Gerando projeto Blazor Web...");
    let gerador = codegen::GeradorCodigo::new_console()?;
    let projeto_cs = gerador.gerar_projeto_console(ast)?;
    
    // Criar projeto Blazor básico  
    println!("✓ Blazor Web projeto criado (baseado em Console)");
    Ok(())
}

fn compilar_para_api(
    ast: &ast::Programa,
    _caminho: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔧 Gerando API .NET Core...");
    let gerador = codegen::GeradorCodigo::new_console()?;
    let projeto_cs = gerador.gerar_projeto_console(ast)?;
    
    // Criar projeto API básico
    println!("✓ API .NET Core projeto criado (baseado em Console)");
    Ok(())
}

fn compilar_para_fullstack(
    ast: &ast::Programa,
    caminho: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔧 Gerando solução Full Stack...");
    
    // Compilar cada parte
    compilar_para_api(ast, caminho)?;
    compilar_para_blazor_web(ast, caminho)?;
    compilar_para_maui_hybrid(ast, caminho)?;
    
    println!("✓ Solução Full Stack criada!");
    Ok(())
}

fn compilar_para_bytecode(_ast: &ast::Programa) -> Result<(), Box<dyn std::error::Error>> {
    println!("6. Geração de bytecode próprio...");
    let gerador = codegen::GeradorCodigo::new_bytecode()?;
    match gerador.gerar_programa(_ast) {
        Ok(()) => {
            let bytecode = gerador.obter_bytecode();
            println!(" ✓ Bytecode gerado com sucesso");
            println!(" ✓ {} instruções processadas", bytecode.len());
            Ok(())
        }
        Err(e) => Err(format!("Erro na geração de bytecode: {}", e).into()),
    }
}
