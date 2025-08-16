use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn list_exemplos() -> Vec<String> {
    let root = repo_root();
    let dir = root.join("exemplos");
    let mut v = Vec::new();
    if let Ok(read_dir) = fs::read_dir(&dir) {
        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("pr") {
                if let Ok(rel) = path.strip_prefix(&root) {
                    v.push(rel.to_string_lossy().replace('\\', "/"));
                }
            }
        }
    }
    v.sort();
    v
}

fn normalize(s: &str) -> String {
    s.replace("\r\n", "\n")
        .chars()
        .filter(|&c| c != ' ' && c != '\t')
        .collect()
}

fn expected_for(stem: &str) -> Option<String> {
    let p = repo_root().join(format!("{}.out.txt", stem));
    fs::read_to_string(&p).ok()
}

fn strip_bom(s: &str) -> &str {
    s.strip_prefix('\u{feff}').unwrap_or(s)
}

fn needs_stdin(pr: &Path) -> bool {
    fs::read_to_string(pr)
        .map(|s| s.contains("LerLinha"))
        .unwrap_or(false)
}

#[test]
fn run_exemplos_llvm_end_to_end() {
    let root = repo_root();
    for pr_rel in list_exemplos() {
        let pr_path = root.join(&pr_rel);
        let stem = pr_path.file_stem().and_then(|s| s.to_str()).unwrap();

        // programa_principal precisa de biblioteca.pr também
        let args: Vec<String> = if pr_rel.ends_with("programa_principal.pr") {
            vec![
                "exemplos/programa_principal.pr".into(),
                "exemplos/biblioteca.pr".into(),
                "--target=llvm-ir".into(),
            ]
        } else {
            vec![pr_rel.clone(), "--target=llvm-ir".into()]
        };
        let output = Command::new(env!("CARGO_BIN_EXE_compilador"))
            .current_dir(&root)
            .args(args)
            .output()
            .expect("falha ao executar compilador");
        assert!(output.status.success(), "compilador falhou para {}", pr_rel);

        // Executável esperado na raiz: <stem>.exe (Windows) ou <stem>
        let exe = if cfg!(windows) {
            root.join(format!("{}.exe", stem))
        } else {
            root.join(stem)
        };
        assert!(exe.exists(), "executável não encontrado: {:?}", exe);

        let mut cmd = Command::new(&exe);
        cmd.current_dir(&root).stdout(Stdio::piped());
        if needs_stdin(&pr_path) {
            cmd.stdin(Stdio::piped());
        }
        let mut child = cmd.spawn().expect("falha ao executar exemplo");

        if needs_stdin(&pr_path) {
            use std::io::Write;
            let stdin = child.stdin.as_mut().unwrap();
            // entrada-padrão usada no teste de IO
            stdin.write_all(b"adriano\n30\n").unwrap();
            // fecha stdin para sinalizar EOF
            drop(stdin);
        }

        let out = child.wait_with_output().expect("falha ao aguardar saida");
        assert!(
            out.status.success(),
            "execucao retornou erro para {}",
            pr_rel
        );
        let got = normalize(&String::from_utf8_lossy(&out.stdout));

        if let Some(exp) = expected_for(stem) {
            let want = normalize(strip_bom(&exp));
            assert_eq!(got.trim(), want.trim(), "saida incorreta para {}", pr_rel);
        } else if pr_rel.ends_with("/teste_io.pr") {
            // Validação específica do IO
            let text = String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n");
            assert!(text.contains("Digite seu nome:"));
            assert!(text.contains("Olá, adriano"));
            assert!(text.contains("Digite sua idade:"));
            assert!(text.contains("Você tem 30 anos."));
        }
    }
}
