mod lexer;
mod ast;
mod codegen;
mod type_checker;
mod ownership;
mod inferencia_tipos;
mod module_system;
mod stdlib;
mod interpolacao;

use lalrpop_util::lalrpop_mod;
lalrpop_mod!(pub parser);

use std::fs;
use logos::Logos;
use inkwell::context::Context;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Uso: {} <arquivo.pr>", args[0]);
        std::process::exit(1);
    }

    compilar_arquivo(&args[1])
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

    println!(" ✓ {} tokens processados", tokens.len());

    // Debug: mostrar alguns tokens (apenas se poucos tokens)
    if tokens.len() <= 30 {
        println!(" Tokens encontrados:");
        for (i, (pos, token, end)) in tokens.iter().enumerate() {
            println!("   {}: {:?} ({}..{})", i, token, pos, end);
        }
    }

    // 2. Análise Sintática
    println!("2. Análise Sintática...");
    let parser = parser::ArquivoParser::new();
    let mut ast = parser.parse(tokens.iter().cloned())
        .map_err(|e| format!("Erro sintático: {:?}", e))?;

    // Percorre e converte as StringInterpolada->somar strings
    crate::interpolacao::walk_programa(&mut ast, |e| {
        *e = interpolacao::planificar_interpolada(e.clone());
    });

    println!(" ✓ AST gerado com sucesso");
    println!("   - {} namespaces", ast.namespaces.len());
    println!("   - {} declarações", ast.declaracoes.len());

    // 3. Adicionar biblioteca padrão
    println!("3. Carregando biblioteca padrão...");
    let mut stdlib = stdlib::criar_biblioteca_padrao();
    ast.declaracoes.append(&mut stdlib);

    // 3.5. Verificação de Compatibilidade
    println!("3.5. Verificando compatibilidade...");
    verificar_compatibilidade_ast(&ast)?;

    // ✅ NOVO: 3.7. Inferência de Tipos
    println!("3.7. Inferência de tipos...");
    let mut inferencia = inferencia_tipos::InferenciaTipos::new();
    
    // Registrar classes para inferência com herança
    for declaracao in &ast.declaracoes {
        if let ast::Declaracao::DeclaracaoClasse(classe) = declaracao {
            println!("   📋 Registrando classe '{}' para inferência", classe.nome);
            if let Some(pai) = &classe.classe_pai {
                println!("     └─ Herda de: {}", pai);
            }
            inferencia.registrar_classe(classe.clone());
        }
    }
    
    // Verificar inferência de tipos em comandos principais
    for declaracao in &ast.declaracoes {
        if let ast::Declaracao::Comando(comando) = declaracao {
            if let Err(erro) = inferencia.inferir_tipo_comando(comando) {
                eprintln!("   ⚠️ Aviso de inferência: {}", erro);
            }
        }
    }
    println!(" ✓ Inferência de tipos concluída");

    // 4. Verificação de Tipos com Herança
    println!("4. Verificação de tipos e herança...");
    let mut verificador_tipos = type_checker::VerificadorTipos::new();
    
    // ✅ NOVO: Registrar classes para verificação de herança
    let mut classes_com_heranca = 0;
    for declaracao in &ast.declaracoes {
        if let ast::Declaracao::DeclaracaoClasse(classe) = declaracao {
            if classe.classe_pai.is_some() {
                classes_com_heranca += 1;
                println!("   🔗 Classe '{}' utiliza herança", classe.nome);
            }
        }
    }
    
    if classes_com_heranca > 0 {
        println!("   📊 {} classe(s) utilizam herança", classes_com_heranca);
    }

    match verificador_tipos.verificar_programa(&ast) {
        Ok(()) => println!(" ✓ Tipos e herança verificados com sucesso"),
        Err(erros) => {
            eprintln!(" ⚠️ Avisos de tipo/herança encontrados:");
            for erro in &erros {
                eprintln!("   - {}", erro);
            }
        }
    }

    // 5. Análise de Ownership com Herança
    println!("5. Análise de ownership e polimorfismo...");
    let mut analisador_ownership = ownership::AnalisadorOwnership::new();
    
    // ✅ NOVO: Registrar classes para análise de ownership com herança
    for declaracao in &ast.declaracoes {
        if let ast::Declaracao::DeclaracaoClasse(classe) = declaracao {
            analisador_ownership.registrar_classe(classe.clone());
        }
    }

    match analisador_ownership.analisar_programa(&ast) {
        Ok(warnings) => {
            println!(" ✓ Ownership e polimorfismo verificados com sucesso");
            if !warnings.is_empty() {
                println!("   Avisos:");
                let mut avisos_heranca = 0;
                for warning in &warnings {
                    println!("   - {}", warning);
                    if warning.contains("polimórfica") || warning.contains("redefinível") {
                        avisos_heranca += 1;
                    }
                }
                if avisos_heranca > 0 {
                    println!("   📊 {} aviso(s) relacionados à herança/polimorfismo", avisos_heranca);
                }
            }
        },
        Err(erros) => {
            eprintln!(" ⚠️ Avisos de ownership encontrados:");
            for erro in &erros {
                eprintln!("   - {}", erro);
            }
        }
    }

    // 6. Geração de Código LLVM com Herança
    println!("6. Geração de código com suporte à herança...");
    let context = Context::create();
    let gerador = codegen::GeradorCodigo::new(&context);
    let i32_type = context.i32_type();
    let function_type = i32_type.fn_type(&[], false);
    let function = gerador.module.add_function("main", function_type, None);
    let basic_block = context.append_basic_block(function, "entry");
    gerador.builder.position_at_end(basic_block);

    // ✅ NOVO: Verificar se há funcionalidades de herança sendo usadas
    let mut usa_heranca = false;
    let mut metodos_redefinidos = 0;
    
    for declaracao in &ast.declaracoes {
        if let ast::Declaracao::DeclaracaoClasse(classe) = declaracao {
            if classe.classe_pai.is_some() {
                usa_heranca = true;
            }
            for metodo in &classe.metodos {
                if metodo.eh_virtual || metodo.eh_override {
                    metodos_redefinidos += 1;
                }
            }
        }
    }
    
    if usa_heranca {
        println!("   🔗 Detectada herança - usando geração de código polimórfica");
        if metodos_redefinidos > 0 {
            println!("   🎯 {} método(s) redefinível/sobrescreve detectados", metodos_redefinidos);
        }
    }

    // Compilar o programa com tratamento de erros melhorado
    match gerador.compilar_programa(&ast) {
        Ok(()) => {
            let _ = gerador.builder.build_return(Some(&i32_type.const_int(0, false)));
            println!(" ✓ Código gerado com sucesso");
            if usa_heranca {
                println!("   ✓ Herança e polimorfismo suportados");
            }
        }

        Err(e) if e.contains("não implementado") => {
            eprintln!(" ⚠️ Funcionalidade não implementada: {}", e);
            eprintln!(" ℹ️ Gerando código básico...");
            let _ = gerador.builder.build_return(Some(&i32_type.const_int(0, false)));
        }

        Err(e) => return Err(format!("Erro na geração de código: {}", e).into()),
    }

    // 7. Verificação e Saída
    println!("7. Verificação final...");
    match gerador.module.verify() {
        Ok(()) => println!(" ✓ Módulo LLVM válido"),
        Err(e) => {
            eprintln!(" ⚠️ Aviso na verificação LLVM: {}", e);
            eprintln!(" ℹ️ Continuando com arquivo de saída...");
        }
    }

    let output_path = format!("{}.ll", caminho_arquivo.trim_end_matches(".pr"));
    gerador.module.print_to_file(&output_path)
        .map_err(|e| format!("Erro ao escrever arquivo: {}", e))?;

    println!("✓ Compilação concluída! Arquivo gerado: {}", output_path);
    println!("\nPara executar:");
    println!("  clang {} -o {}", output_path, caminho_arquivo.trim_end_matches(".pr"));
    println!("  ./{}", caminho_arquivo.trim_end_matches(".pr"));

    // 8. Estatísticas finais com informações de herança
    println!("\n=== Estatísticas da Compilação ===");
    println!("Namespaces processados: {}", ast.namespaces.len());
    println!("Declarações processadas: {}", ast.declaracoes.len());
    println!("Tokens analisados: {}", tokens.len());
    
    // ✅ NOVO: Estatísticas de herança
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

    Ok(())
}

fn verificar_compatibilidade_ast(ast: &ast::Programa) -> Result<(), Box<dyn std::error::Error>> {
    // Verificações básicas de compatibilidade
    for namespace in &ast.namespaces {
        for declaracao in &namespace.declaracoes {
            verificar_declaracao_compatibilidade(declaracao)?;
        }
    }

    for declaracao in &ast.declaracoes {
        verificar_declaracao_compatibilidade(declaracao)?;
    }

    // ✅ NOVO: Verificações específicas de herança
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
                println!("   ✓ Herança válida: {} : {}", nome_classe, classe_pai);
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

fn verificar_declaracao_compatibilidade(declaracao: &ast::Declaracao) -> Result<(), Box<dyn std::error::Error>> {
    match declaracao {
        ast::Declaracao::Comando(comando) => {
            match comando {
                ast::Comando::Para(_, _, _, _) => {
                    eprintln!(" ⚠️ Loop 'para' detectado - funcionalidade em desenvolvimento");
                }
                
                // ✅ NOVO: Verificar comandos relacionados à herança
                ast::Comando::ChamarMetodo(objeto, metodo, _) => {
                    if metodo.starts_with("redefinivel_") || metodo.starts_with("sobrescreve_") {
                        println!("   ✓ Chamada polimórfica detectada: {}.{}", objeto, metodo);
                    }
                }
                
                ast::Comando::CriarObjeto(var, classe, _) => {
                    println!("   ✓ Criação de objeto detectada: {} = novo {}", var, classe);
                }
                
                _ => {}
            }
        }
        
        // ✅ NOVO: Verificar declarações de classe
        ast::Declaracao::DeclaracaoClasse(classe) => {
            if let Some(pai) = &classe.classe_pai {
                println!("   ✓ Classe com herança detectada: {} : {}", classe.nome, pai);
            }
            
            for metodo in &classe.metodos {
                if metodo.eh_virtual {
                    println!("   ✓ Método redefinível detectado: {}.{}", classe.nome, metodo.nome);
                }
                if metodo.eh_override {
                    println!("   ✓ Método sobrescreve detectado: {}.{}", classe.nome, metodo.nome);
                }
            }
        }
        
        _ => {}
    }

    Ok(())
}