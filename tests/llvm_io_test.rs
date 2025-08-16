use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn teste_io_llvm_execucao() {
    // Compila o exemplo para LLVM IR e executável, então executa com stdin simulado.
    let root = repo_root();
    let exemplo = root.join("exemplos").join("teste_io.pr");

    // Invoca o binário do compilador para gerar e compilar via clang.
    let output = Command::new(env!("CARGO_BIN_EXE_compilador"))
        .current_dir(&root)
        .args([exemplo.to_string_lossy().as_ref(), "--target=llvm-ir"])
        .output()
        .expect("falha ao executar compilador");
    assert!(
        output.status.success(),
        "compilador falhou: stdout=\n{}\n-- stderr=\n{}\n",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    // Executa o binário gerado (teste_io.exe no Windows)
    let exe = if cfg!(windows) {
        root.join("teste_io.exe")
    } else {
        root.join("teste_io")
    };
    assert!(exe.exists(), "executável não encontrado: {:?}", exe);

    let mut child = Command::new(&exe)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("falha ao iniciar execução do exemplo");

    use std::io::Write;
    let stdin = child.stdin.as_mut().expect("sem stdin");
    stdin
        .write_all(b"adriano\n30\n")
        .expect("falha ao escrever input");
    drop(stdin);

    let out = child.wait_with_output().expect("falha ao aguardar saida");
    assert!(out.status.success(), "execucao retornou erro");

    let texto = String::from_utf8_lossy(&out.stdout);
    // Normaliza quebra de linha CRLF/LF
    let norm = texto.replace("\r\n", "\n");
    assert!(norm.contains("Digite seu nome:"));
    assert!(norm.contains("Olá, adriano"));
    assert!(norm.contains("Digite sua idade:"));
    assert!(norm.contains("Você tem 30 anos."));
}
