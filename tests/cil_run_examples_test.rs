use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn have_cil_toolchain() -> bool {
    // 1. Verifica dotnet
    if Command::new("dotnet").arg("--version").output().is_ok() {
        return true;
    }
    // 2. Verifica ilasm no Windows
    #[cfg(windows)]
    {
        if Path::new(r"C:\Windows\Microsoft.NET\Framework64\v4.0.30319\ilasm.exe").is_file() {
            return true;
        }
        if Path::new(r"C:\Windows\Microsoft.NET\Framework\v4.0.30319\ilasm.exe").is_file() {
            return true;
        }
    }
    // 3. ilasm no PATH
    Command::new("ilasm").arg("/?").output().is_ok()
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
                    let rel_str = rel.to_string_lossy().replace('\\', "/");
                    if rel_str.ends_with("_neg.pr") {
                        continue;
                    }
                    v.push(rel_str);
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
fn run_exemplos_cil_end_to_end() {
    if !have_cil_toolchain() {
        eprintln!("Compilador CIL (.NET / ilasm) não encontrado; ignorando teste CIL.");
        return;
    }

    let root = repo_root();
    for pr_rel in list_exemplos() {
        let pr_path = root.join(&pr_rel);
        let stem = pr_path.file_stem().and_then(|s| s.to_str()).unwrap();

        let content = fs::read_to_string(&pr_path).unwrap_or_default();
        if content.contains("usando Sistema;") || content.contains("usando Sistema.") {
            // Exemplos que usam a biblioteca externa do sistema (sistema-padrao) requerem pipeline de stdlib
            continue;
        }

        eprintln!("[cil-e2e] Compilando e executando: {}", pr_rel);

        // programa_principal precisa de biblioteca.pr também
        let args: Vec<String> = if pr_rel.ends_with("programa_principal.pr") {
            vec![
                "exemplos/biblioteca.pr".into(),
                "exemplos/programa_principal.pr".into(),
                "--target=dotnet".into(),
            ]
        } else {
            vec![pr_rel.clone(), "--target=dotnet".into()]
        };

        let output = Command::new(env!("CARGO_BIN_EXE_compilador"))
            .current_dir(&root)
            .args(&args)
            .output()
            .expect("falha ao executar compilador");

        assert!(
            output.status.success(),
            "compilador falhou para {}:\nSTDOUT:\n{}\nSTDERR:\n{}",
            pr_rel,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        let exe = if cfg!(windows) {
            root.join(format!("{}.exe", stem))
        } else {
            root.join(stem)
        };
        assert!(exe.exists(), "executável CIL não encontrado: {:?}", exe);

        let mut cmd = Command::new(&exe);
        cmd.current_dir(&root).stdout(Stdio::piped());
        if needs_stdin(&pr_path) {
            cmd.stdin(Stdio::piped());
        }
        let mut child = cmd.spawn().expect("falha ao executar binário compilado CIL");

        if needs_stdin(&pr_path) {
            use std::io::Write;
            let mut stdin = child.stdin.take().unwrap();
            stdin.write_all(b"adriano\n30\n").unwrap();
            drop(stdin);
        }

        let out = child.wait_with_output().expect("falha ao aguardar saida");
        assert!(
            out.status.success(),
            "execucao CIL retornou erro para {}:\nSTDOUT:\n{}\nSTDERR:\n{}",
            pr_rel,
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        let got = normalize(&String::from_utf8_lossy(&out.stdout));

        if let Some(exp) = expected_for(stem) {
            let want = normalize(strip_bom(&exp));
            assert_eq!(got.trim(), want.trim(), "saida incorreta para {}", pr_rel);
        } else if pr_rel.ends_with("/teste_io.pr") {
            let text = String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n");
            assert!(text.contains("Digite seu nome:"));
            assert!(text.contains("Olá, adriano"));
            assert!(text.contains("Digite sua idade:"));
            assert!(text.contains("Você tem 30 anos."));
        }

        // Limpeza dos binários de teste gerados
        let _ = fs::remove_file(&exe);
        let il_file = root.join(format!("{}.il", stem));
        let _ = fs::remove_file(&il_file);
    }
}

/// Subconjunto da biblioteca padrão mapeado para a BCL .NET
/// (Matematica→System.Math, Texto→System.String, Guid→System.Guid,
/// Ambiente→System.Environment, Conversao→Parse com cultura invariante).
#[test]
fn cil_stdlib_bcl_mapeado() {
    if !have_cil_toolchain() {
        eprintln!("Compilador CIL (.NET / ilasm) não encontrado; ignorando.");
        return;
    }
    let root = repo_root();
    let dir = root.join("target/test-temp-cil-stdlib");
    fs::create_dir_all(&dir).unwrap();
    let pr_path = dir.join("stdlib_bcl.pr");
    let src = r#"
        usando Sistema;

        função Principal() {
            Console.EscreverLinha(Matematica.Abs(-7));
            Console.EscreverLinha(Matematica.Max(3, 9));
            Console.EscreverLinha(Texto.ParaMaiusculas("ola"));
            Console.EscreverLinha(Texto.Tamanho("abc"));
            Console.EscreverLinha(Texto.ComecaCom("por do sol", "por"));
            Console.EscreverLinha(Texto.Subtexto("Portugues", 0, 4));
            Console.EscreverLinha(Texto.Comparar("a", "b"));
            Console.EscreverLinha(Texto.IgualIgnoreCase("Oi", "oI"));
            Console.EscreverLinha(Guid.Vazio());
            Console.EscreverLinha(Conversao.ParaInteiro("42"));
            Console.EscreverLinha(Conversao.ParaBooleano("verdadeiro"));
            Console.EscreverLinha(Ambiente.NomeComputador());
        }
    "#;
    fs::write(&pr_path, src).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_compilador"))
        .current_dir(&root)
        .args([pr_path.to_str().unwrap(), "--target=dotnet"])
        .output()
        .expect("falha ao executar compilador");
    assert!(
        output.status.success(),
        "compilador CIL falhou:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let exe = if cfg!(windows) {
        root.join("stdlib_bcl.exe")
    } else {
        root.join("stdlib_bcl")
    };
    let out = Command::new(&exe).output().expect("falha ao executar exe CIL");
    assert!(
        out.status.success(),
        "execução CIL falhou:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n");
    let lines: Vec<&str> = stdout.lines().collect();
    let esperadas = [
        "7",        // Abs (float64 formatado pela cultura)
        "9",
        "OLA",
        "3",
        "True",
        "Port",
        "-1",
        "True",
        "00000000-0000-0000-0000-000000000000",
        "42",
        "True",
    ];
    for (i, e) in esperadas.iter().enumerate() {
        let linha = lines.get(i).map(|s| s.trim()).unwrap_or("");
        // Abs/Max são float64: aceita "7" ou "7,0" conforme cultura.
        if i < 2 {
            assert!(
                linha.starts_with(e),
                "linha {} divergente: '{}' != prefixo '{}'. stdout:\n{}",
                i, linha, e, stdout
            );
        } else {
            assert_eq!(linha, *e, "linha {} divergente. stdout:\n{}", i, stdout);
        }
    }
    // NomeComputador varia por máquina — basta existir uma 12ª linha.
    assert!(lines.len() >= 12, "saída incompleta:\n{}", stdout);

    let _ = fs::remove_file(&exe);
    let _ = fs::remove_file(root.join("stdlib_bcl.il"));
}

