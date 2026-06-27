// src/runtime/mod.rs
pub mod execution_context;

// Re-exportar tipos principais para facilitar o uso
pub use execution_context::ContextoExecucao;

use crate::ast::*;

/// Executa um programa compilado com verificações completas e suporte a namespaces
pub fn executar_programa_otimizado(programa: &Programa) -> Result<(), String> {
    let mut contexto = ContextoExecucao::new();

    //Registrar classes do nível raiz
    for decl in &programa.declaracoes {
        if let Declaracao::DeclaracaoClasse(classe) = decl {
            contexto.registrar_classe_compilada(classe)?;
        }
    }

    //Também registrar classes dentro de namespaces
    for namespace in &programa.namespaces {
        for decl in &namespace.declaracoes {
            if let Declaracao::DeclaracaoClasse(classe) = decl {
                contexto.registrar_classe_compilada(classe)?;
            }
        }
    }

    // ✅ CORREÇÃO: Procurar função principal em todas as declarações
    // Primeiro, procurar no nível raiz
    for decl in &programa.declaracoes {
        if let Declaracao::DeclaracaoFuncao(funcao) = decl {
            if funcao.nome == "principal" || funcao.nome.ends_with("::principal") {
                return executar_funcao_principal(&mut contexto, funcao);
            }
        }
    }

    // ✅ CORREÇÃO: Depois procurar dentro dos namespaces
    for namespace in &programa.namespaces {
        for decl in &namespace.declaracoes {
            if let Declaracao::DeclaracaoFuncao(funcao) = decl {
                if funcao.nome == "principal" || funcao.nome.ends_with("::principal") {
                    return executar_funcao_principal(&mut contexto, funcao);
                }
            }
        }
    }

    Err("Função 'principal' não encontrada".to_string())
}

/// Executa especificamente a função principal com contexto isolado
fn executar_funcao_principal(
    contexto: &mut ContextoExecucao,
    funcao: &DeclaracaoFuncao,
) -> Result<(), String> {
    println!("🚀 Executando função principal: {}", funcao.nome);

    contexto.entrar_escopo();

    for comando in &funcao.corpo {
        if let Err(e) = contexto.executar_comando(comando) {
            contexto.sair_escopo();
            return Err(format!("Erro na execução: {}", e));
        }
    }

    contexto.sair_escopo();
    println!("✅ Execução concluída com sucesso");
    Ok(())
}

/// Interpreta bytecode com classes dinamicamente carregadas
pub fn interpretar_com_classes(
    bytecode: Vec<String>,
    classes: Vec<&DeclaracaoClasse>,
) -> Result<(), String> {
    println!("🔧 Executando bytecode com classes registradas dinamicamente...");
    println!(" ✓ {} classes carregadas para interpretação", classes.len());
    println!(" ✓ {} instruções de bytecode processadas", bytecode.len());

    // Log das classes registradas com informações detalhadas
    for (i, classe) in classes.iter().enumerate() {
        println!(" └─ Classe {}: {}", i + 1, classe.nome);
        if let Some(pai) = &classe.classe_pai {
            println!("     └─ Herda de: {}", pai);
        }
        if !classe.metodos.is_empty() {
            println!("     └─ {} método(s) disponível(eis)", classe.metodos.len());
        }
        if !classe.propriedades.is_empty() {
            println!(
                "     └─ {} propriedade(s) definida(s)",
                classe.propriedades.len()
            );
        }
    }

    // Log das primeiras instruções do bytecode
    if !bytecode.is_empty() {
        println!(" 📄 Primeiras instruções do bytecode:");
        for (i, instrucao) in bytecode.iter().take(5).enumerate() {
            println!("     {}. {}", i + 1, instrucao);
        }
        if bytecode.len() > 5 {
            println!("     ... e mais {} instruções", bytecode.len() - 5);
        }
    }

    // ✅ IMPLEMENTAÇÃO: Simular execução de bytecode
    let mut contexto = ContextoExecucao::new();

    // Registrar classes no contexto
    for classe in &classes {
        contexto.registrar_classe_compilada(classe)?;
    }

    // Executar instruções de bytecode (implementação básica)
    for (i, instrucao) in bytecode.iter().enumerate() {
        match interpretar_instrucao_bytecode(instrucao, &mut contexto) {
            Ok(_) => {}
            Err(e) => {
                println!("❌ Erro na instrução {}: {}", i + 1, e);
                return Err(format!(
                    "Falha na execução do bytecode na instrução {}: {}",
                    i + 1,
                    e
                ));
            }
        }
    }

    println!(" ✓ Interpretação dinâmica concluída com sucesso");
    Ok(())
}

/// Interpreta uma única instrução de bytecode
fn interpretar_instrucao_bytecode(
    instrucao: &str,
    _contexto: &mut ContextoExecucao,
) -> Result<(), String> {
    // Implementação básica de interpretação de bytecode
    let partes: Vec<&str> = instrucao.split_whitespace().collect();

    match partes.get(0) {
        Some(&"LOAD") => {
            if let Some(valor) = partes.get(1) {
                println!("   Carregando: {}", valor);
                Ok(())
            } else {
                Err("LOAD requer um valor".to_string())
            }
        }
        Some(&"STORE") => {
            if let Some(var) = partes.get(1) {
                println!("   Armazenando em: {}", var);
                Ok(())
            } else {
                Err("STORE requer uma variável".to_string())
            }
        }
        Some(&"CALL") => {
            if let Some(funcao) = partes.get(1) {
                println!("   Chamando função: {}", funcao);
                Ok(())
            } else {
                Err("CALL requer uma função".to_string())
            }
        }
        Some(&"PRINT") => {
            if let Some(texto) = partes.get(1) {
                println!("   Saída: {}", texto);
                Ok(())
            } else {
                Err("PRINT requer um texto".to_string())
            }
        }
        Some(op) => {
            println!("   Operação não implementada: {}", op);
            Ok(()) // Não falhar em operações desconhecidas por enquanto
        }
        None => Err("Instrução vazia".to_string()),
    }
}

/// Executa programa em modo debug com informações detalhadas
pub fn executar_programa_debug(programa: &Programa) -> Result<(), String> {
    println!("🐛 Modo DEBUG ativado");
    println!("📊 Estatísticas do programa:");
    println!("   • Namespaces: {}", programa.namespaces.len());
    println!(
        "   • Declarações no nível raiz: {}",
        programa.declaracoes.len()
    );

    // Contar declarações dentro de namespaces
    let mut total_decl_namespaces = 0;
    for ns in &programa.namespaces {
        total_decl_namespaces += ns.declaracoes.len();
        println!(
            "   • Namespace '{}': {} declarações",
            ns.nome,
            ns.declaracoes.len()
        );
    }

    println!(
        "   • Total de declarações em namespaces: {}",
        total_decl_namespaces
    );

    executar_programa_otimizado(programa)
}

/// Função utilitária para validar estrutura do programa
pub fn validar_programa(programa: &Programa) -> Result<(), Vec<String>> {
    let mut erros = Vec::new();

    // Verificar se há função principal
    let mut tem_principal = false;

    // Verificar no nível raiz
    for decl in &programa.declaracoes {
        if let Declaracao::DeclaracaoFuncao(funcao) = decl {
            if funcao.nome == "principal" || funcao.nome.ends_with("::principal") {
                tem_principal = true;
                break;
            }
        }
    }

    // Verificar em namespaces se não encontrou no nível raiz
    if !tem_principal {
        for namespace in &programa.namespaces {
            for decl in &namespace.declaracoes {
                if let Declaracao::DeclaracaoFuncao(funcao) = decl {
                    if funcao.nome == "principal" || funcao.nome.ends_with("::principal") {
                        tem_principal = true;
                        break;
                    }
                }
            }
            if tem_principal {
                break;
            }
        }
    }

    if !tem_principal {
        erros.push("Função 'principal' não encontrada no programa".to_string());
    }

    // Verificar classes duplicadas
    let mut nomes_classes = std::collections::HashSet::new();

    for decl in &programa.declaracoes {
        if let Declaracao::DeclaracaoClasse(classe) = decl {
            if !nomes_classes.insert(classe.nome.clone()) {
                erros.push(format!("Classe '{}' definida mais de uma vez", classe.nome));
            }
        }
    }

    for namespace in &programa.namespaces {
        for decl in &namespace.declaracoes {
            if let Declaracao::DeclaracaoClasse(classe) = decl {
                let nome_qualificado = format!("{}::{}", namespace.nome, classe.nome);
                if !nomes_classes.insert(nome_qualificado.clone()) {
                    erros.push(format!(
                        "Classe '{}' definida mais de uma vez",
                        nome_qualificado
                    ));
                }
            }
        }
    }

    if erros.is_empty() {
        Ok(())
    } else {
        Err(erros)
    }
}
