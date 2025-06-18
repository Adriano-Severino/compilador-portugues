mod common;

use common::*;

#[test]
fn test_compilacao_simples() {
    let codigo = r#"publico classe Principal {
    publico vazio Main() {
        imprima("Hello, World!");
    }
}"#;
    
    criar_arquivo_teste("simples.pr", codigo);
    
    // Testar apenas targets que sabemos que funcionam
    assert!(compilar_arquivo_teste("simples.pr", "console"), "Falha no target console");
    assert!(compilar_arquivo_teste("simples.pr", "bytecode"), "Falha no target bytecode");
}

#[test]
fn test_heranca() {
    let codigo = r#"publico classe Animal {
    publico texto Nome;
    
    publico construtor(texto nome) {
        este.Nome = nome;
    }
    
    publico vazio som() {
        imprima("Animal faz som");
    }
}

publico classe Cachorro : Animal {
    publico construtor(texto nome) {
        este.Nome = nome;
    }
    
    publico vazio som() {
        imprima("Au au!");
    }
}

publico classe Principal {
    publico vazio Main() {
        var animal = novo Cachorro("Rex");
        animal.som();
    }
}"#;
    
    criar_arquivo_teste("heranca.pr", codigo);
    assert!(compilar_arquivo_teste("heranca.pr", "console"), "Falha no target console para herança");
}

#[test] 
fn test_targets_basicos() {
    let codigo = r#"publico classe Teste {
    publico vazio Main() {
        imprima("Teste básico");
    }
}"#;
    
    criar_arquivo_teste("targets_basicos.pr", codigo);
    
    // Testar apenas os targets que estão implementados
    let targets = ["console", "bytecode"];
    
    for target in &targets {
        assert!(compilar_arquivo_teste("targets_basicos.pr", target), 
                "Falha no target: {}", target);
    }
}
