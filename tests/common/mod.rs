use std::process::Command;
use std::fs;
use std::path::Path;

pub fn compilar_arquivo_teste(nome_arquivo: &str, target: &str) -> bool {
    // Garantir que o diretório existe
    let test_dir = "tests/test_files";
    if !Path::new(test_dir).exists() {
        fs::create_dir_all(test_dir).expect("Falha ao criar diretório de teste");
    }
    
    let arquivo_path = format!("{}/{}", test_dir, nome_arquivo);
    
    // Verificar se arquivo existe
    if !Path::new(&arquivo_path).exists() {
        eprintln!("Arquivo de teste não encontrado: {}", arquivo_path);
        return false;
    }

    // Executar o compilador
    let output = Command::new("cargo")
        .args(&["run", "--", &arquivo_path, &format!("--target={}", target)])
        .current_dir(".")
        .output();

    match output {
        Ok(result) => {
            if !result.status.success() {
                eprintln!("Erro na compilação:");
                eprintln!("stdout: {}", String::from_utf8_lossy(&result.stdout));
                eprintln!("stderr: {}", String::from_utf8_lossy(&result.stderr));
                false
            } else {
                true
            }
        }
        Err(e) => {
            eprintln!("Falha ao executar comando: {}", e);
            false
        }
    }
}

pub fn criar_arquivo_teste(nome: &str, conteudo: &str) {
    let test_dir = "tests/test_files";
    if !Path::new(test_dir).exists() {
        fs::create_dir_all(test_dir).expect("Falha ao criar diretório de teste");
    }
    
    let arquivo_path = format!("{}/{}", test_dir, nome);
    fs::write(&arquivo_path, conteudo)
        .unwrap_or_else(|e| panic!("Falha ao criar arquivo de teste {}: {}", arquivo_path, e));
    
    println!("Arquivo de teste criado: {}", arquivo_path);
}
