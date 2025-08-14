use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn have_clang() -> bool {
    Command::new("clang").arg("--version").output().is_ok()
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

fn normalize_newlines(s: &str) -> String {
    s.replace("\r\n", "\n")
}

#[test]
fn llvm_virtual_dispatch_base_ref() {
    if !have_clang() {
        eprintln!("clang não encontrado; ignorando teste LLVM.");
        return;
    }

    let root = repo_root();
    let dir = root.join("target/test-temp-llvm");
    fs::create_dir_all(&dir).unwrap();
    let pr_path = dir.join("dispatch_base_ref.pr");
    let src = r#"
        usando D;
        espaco D {
            publico classe Base { publico redefinível vazio F() { imprima("B"); } }
            publico classe Derivada : Base { publico sobrescreve vazio F() { imprima("D"); } }
        }
        publico função vazio Principal() {
            Base x = novo Derivada();
            x.F();
        }
    "#;
    fs::write(&pr_path, src).unwrap();

    let (code, _out, err) = run_compiler(&[pr_path.to_str().unwrap(), "--target=llvm-ir"]);
    assert_eq!(code, 0, "compilador falhou: {}", err);

    // Executável gerado no diretório raiz, com nome do stem
    let exe_stem = pr_path.file_stem().unwrap().to_string_lossy().into_owned();
    let exe = if cfg!(windows) {
        root.join(format!("{}.exe", exe_stem))
    } else {
        root.join(exe_stem)
    };
    assert!(exe.exists(), "executável não gerado: {}", exe.display());

    let out = Command::new(&exe).output().expect("falha ao executar exe");
    let stdout = normalize_newlines(&String::from_utf8_lossy(&out.stdout));
    assert_eq!(stdout.trim(), "D");
}

#[test]
fn override_signature_mismatch_should_fail() {
    // Gera um programa inválido: override muda assinatura
    let root = repo_root();
    let dir = root.join("target/test-temp-override");
    fs::create_dir_all(&dir).unwrap();
    let pr_path = dir.join("override_invalido2.pr");
    let src = r#"
        espaco O {
            publico classe B { publico redefinível vazio F(inteiro a) { imprima("B"); } }
            publico classe D : B { publico sobrescreve vazio F() { imprima("D"); } }
        }
        publico função vazio Principal() { }
    "#;
    fs::write(&pr_path, src).unwrap();

    let (code, _o, e) = run_compiler(&[pr_path.to_str().unwrap(), "--target=bytecode"]);
    assert_ne!(code, 0, "esperava falha do compilador, mas foi sucesso");
    assert!(
        e.contains("Assinatura incompatível") || e.to_lowercase().contains("erro"),
        "mensagem inesperada: {}",
        e
    );
}
