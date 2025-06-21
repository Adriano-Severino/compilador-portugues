use std::path::Path;
use std::fs;

#[test]
fn test_arquivos_gerados_universal() {
    // Assumindo que já compilamos um arquivo teste
    let nome_base = "tests/test_files/output_test";
    
    // Verificar se os arquivos foram gerados
    assert!(Path::new(&format!("{}.ll", nome_base)).exists(), "Arquivo LLVM IR não foi gerado");
    assert!(Path::new(&format!("{}.il", nome_base)).exists(), "Arquivo CIL não foi gerado");
    assert!(Path::new(&format!("{}.js", nome_base)).exists(), "Arquivo JavaScript não foi gerado");
    assert!(Path::new(&format!("{}_Console", nome_base)).exists(), "Projeto Console não foi gerado");
}

#[test]
fn test_conteudo_llvm_ir() {
    let arquivo_llvm = "tests/test_files/output_test.ll";
    if Path::new(arquivo_llvm).exists() {
        let conteudo = fs::read_to_string(arquivo_llvm).unwrap();
        assert!(conteudo.contains("define i32 @main"), "LLVM IR deve conter função main");
        assert!(conteudo.contains("@printf"), "LLVM IR deve conter chamada printf");
    }
}

#[test]
fn test_conteudo_cil() {
    let arquivo_cil = "tests/test_files/output_test.il";
    if Path::new(arquivo_cil).exists() {
        let conteudo = fs::read_to_string(arquivo_cil).unwrap();
        assert!(conteudo.contains(".assembly"), "CIL deve conter definição de assembly");
        assert!(conteudo.contains("System.Console::WriteLine"), "CIL deve conter chamada WriteLine");
    }
}
