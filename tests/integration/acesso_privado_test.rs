use std::path::PathBuf;
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn run_compiler(args: &[&str]) -> (i32, String, String) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_compilador"));
    cmd.args(args);
    let out = cmd.output().expect("failed to run compilador");
    let code = out.status.code().unwrap_or(-1);
    (
        code,
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

#[test]
fn test_acesso_metodo_privado_falha() {
    let temp_dir = repo_root().join("target").join("test-temp-acesso-privado");
    std::fs::create_dir_all(&temp_dir).ok();
    let p = temp_dir.join("acesso_invalido.pr");
    let src = r#"
        classe Pessoa {
            vazio MeuMetodo() {
                imprima("Metodo privado");
            }
        }

        funcao vazio Principal() {
            var p = novo Pessoa();
            p.MeuMetodo();
        }
    "#;
    std::fs::write(&p, src).unwrap();

    let path_str = p.to_string_lossy().to_string();
    let args = vec![path_str.as_str(), "--target=bytecode"];
    let (code, _o, e) = run_compiler(&args);

    assert_ne!(code, 0, "A compilação deveria falhar devido à tentativa de acesso a um método privado.");
    assert!(e.to_lowercase().contains("inacessível") || e.to_lowercase().contains("private"), "A mensagem de erro deve indicar que o método é inacessível. Erro: {}", e);
}
