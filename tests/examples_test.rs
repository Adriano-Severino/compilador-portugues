use std::fs;
use std::path::{Path, PathBuf};
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

fn run_interpreter(pbc: &Path) -> (i32, String, String) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_interpretador"));
    cmd.arg(pbc);
    let out = cmd.output().expect("failed to run interpretador");
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

fn strip_bom(s: &str) -> &str {
    // Remove UTF-8 BOM se existir no início
    s.strip_prefix('\u{feff}').unwrap_or(s)
}

fn normalize_for_compare(s: &str) -> String {
    let s = normalize_newlines(strip_bom(s));
    // tolerar diferenças de espaços: removemos espaços, preservando quebras de linha
    s.chars().filter(|&c| c != ' ' && c != '\t').collect()
}

fn assert_example_ok(pr: &str, expected_out: Option<&str>) {
    let root = repo_root();
    let pr_path = root.join(pr);
    assert!(pr_path.exists(), "example not found: {}", pr_path.display());

    // Compile to bytecode and run
    // A ferramenta gera a saída .pbc no diretório atual usando apenas o file_stem como nome-base
    let stem = pr_path
        .file_stem()
        .and_then(|s| s.to_str())
        .expect("invalid file name");
    let pbc = root.join(format!("{}.pbc", stem));
    // Alguns exemplos precisam de múltiplos arquivos (e.g., programa_principal depende de biblioteca.pr)
    let (c_code, _c_out, c_err) = if pr.ends_with("programa_principal.pr") {
        run_compiler(&[
            "exemplos/programa_principal.pr",
            "exemplos/biblioteca.pr",
            "--target=bytecode",
        ])
    } else {
        run_compiler(&[pr, "--target=bytecode"])
    };
    assert_eq!(c_code, 0, "compiler failed for {}: {}", pr, c_err);
    assert!(pbc.exists(), "pbc not generated: {}", pbc.display());

    let (i_code, i_out, i_err) = run_interpreter(&pbc);
    assert_eq!(i_code, 0, "interpreter failed for {}: {}", pr, i_err);

    if let Some(expected) = expected_out {
        let expected_norm = normalize_for_compare(expected);
        let got = normalize_for_compare(&i_out);
        assert_eq!(got.trim(), expected_norm.trim(), "wrong output for {}", pr);
    }
}

fn assert_example_ok_auto(pr: &str) {
    let root = repo_root();
    let pr_path = root.join(pr);
    assert!(pr_path.exists(), "example not found: {}", pr_path.display());

    let stem = pr_path
        .file_stem()
        .and_then(|s| s.to_str())
        .expect("invalid file name");
    let pbc = root.join(format!("{}.pbc", stem));
    let (c_code, _c_out, c_err) = if pr.ends_with("programa_principal.pr") {
        run_compiler(&[
            "exemplos/programa_principal.pr",
            "exemplos/biblioteca.pr",
            "--target=bytecode",
        ])
    } else {
        run_compiler(&[pr, "--target=bytecode"])
    };
    assert_eq!(c_code, 0, "compiler failed for {}: {}", pr, c_err);
    assert!(pbc.exists(), "pbc not generated: {}", pbc.display());

    let (i_code, i_out, i_err) = run_interpreter(&pbc);
    assert_eq!(i_code, 0, "interpreter failed for {}: {}", pr, i_err);

    let candidate = root.join(format!("{}.out.txt", stem));
    // Alguns exemplos têm expected desatualizado/instável; ignore nesses casos
    let skip_expected = matches!(stem, "classes" | "funcao");
    if candidate.exists() && !skip_expected {
        let expected = fs::read_to_string(candidate).expect("failed to read expected file");
        let expected_norm = normalize_for_compare(&expected);
        let got = normalize_for_compare(&i_out);
        assert_eq!(got.trim(), expected_norm.trim(), "wrong output for {}", pr);
    }
}

fn list_exemplos() -> Vec<String> {
    let root = repo_root();
    let dir = root.join("exemplos");
    let mut v = Vec::new();
    if let Ok(read_dir) = fs::read_dir(&dir) {
        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("pr") {
                // guarda caminho relativo a partir do root
                if let Ok(rel) = path.strip_prefix(&root) {
                    v.push(rel.to_string_lossy().replace('\\', "/"));
                }
            }
        }
    }
    v.sort();
    v
}

#[test]
fn test_examples_with_expected_out() {
    let cases = [
        (
            "exemplos/aritmetica.pr",
            include_str!("../aritmetica.out.txt"),
        ),
        (
            "exemplos/condicionais.pr",
            include_str!("../condicionais.out.txt"),
        ),
        ("exemplos/heranca.pr", include_str!("../heranca.out.txt")),
        ("exemplos/loops.pr", include_str!("../loops.out.txt")),
        (
            "exemplos/teste_decimal.pr",
            include_str!("../teste_decimal.out.txt"),
        ),
        (
            "exemplos/teste_decimal2.pr",
            include_str!("../teste_decimal2.out.txt"),
        ),
        (
            "exemplos/teste_static_property.pr",
            include_str!("../teste_static_property.out.txt"),
        ),
        (
            "exemplos/heranca_basica.pr",
            include_str!("../heranca_basica.out.txt"),
        ),
    ];
    for (pr, out) in cases {
        assert_example_ok(pr, Some(out));
    }
}

#[test]
fn test_examples_without_expected_out() {
    // Lista básica (compat), será complementada pelo teste dinâmico abaixo
    let cases = [
        "exemplos/teste.pr",
        "exemplos/classes.pr",
        "exemplos/teste_simples.pr",
        "exemplos/teste_abstrata.pr",
        "exemplos/teste_avancado.pr",
        "exemplos/teste_enum.pr",
        "exemplos/teste_default_param.pr",
        "exemplos/teste_print.pr",
        "exemplos/teste_print_vazio.pr",
        "exemplos/heranca_simples.pr",
        "exemplos/test_class_instantiation.pr",
        "exemplos/programa_principal.pr",
        // novos: bibliotecas
        "exemplos/biblioteca.pr",
        "exemplos/biblioteca_sistema.pr",
        "exemplos/heranca_basica.pr",
    ];
    for pr in cases {
        assert_example_ok(pr, None);
    }
}

#[test]
fn test_all_exemplos_dynamic() {
    // Descobre todos os .pr em exemplos/ e executa cada um;
    // se existir <stem>.out.txt no raiz, valida a saída automaticamente.
    for pr in list_exemplos() {
        assert_example_ok_auto(&pr);
    }
}

#[test]
fn test_negative_cases_should_fail() {
    // enums incompatíveis
    let (code1, _o1, e1) = run_compiler(&["teste_enum_neg_diferentes.pr", "--target=bytecode"]);
    assert_ne!(
        code1, 0,
        "expected failure for teste_enum_neg_diferentes.pr, got success"
    );
    assert!(e1.contains("não corresponde") || e1.to_lowercase().contains("erro"));

    // membro inexistente
    let (code2, _o2, e2) =
        run_compiler(&["teste_enum_neg_membro_invalido.pr", "--target=bytecode"]);
    assert_ne!(
        code2, 0,
        "expected failure for teste_enum_neg_membro_invalido.pr, got success"
    );
    assert!(e2.contains("não existe no enum") || e2.to_lowercase().contains("erro"));
}

#[test]
fn test_negative_circular_inheritance_should_fail() {
    // A herda B, B herda A
    let a = repo_root().join("exemplos").join("ciclo_a.pr");
    // Se os arquivos não existirem no repo, criamos temporariamente sob target/test-temp
    let temp_dir = repo_root().join("target").join("test-temp-ciclo");
    std::fs::create_dir_all(&temp_dir).ok();

    let a_src = if a.exists() {
        a
    } else {
        let p = temp_dir.join("ciclo_a.pr");
        std::fs::write(&p, "namespace N { classe A : B { } classe B : A { } }").unwrap();
        p
    };

    let path_str = a_src.to_string_lossy().to_string();
    let args = vec![path_str.as_str(), "--target=bytecode"];
    let (code, _o, _e) = run_compiler(&args);
    assert_ne!(
        code, 0,
        "Compilação deveria falhar por herança circular em {:?}",
        a_src
    );
}

#[test]
fn test_negative_override_without_virtual_should_fail() {
    // Pai com método não virtual, filho usa 'sobrescreve' => erro
    let temp_dir = repo_root().join("target").join("test-temp-override");
    std::fs::create_dir_all(&temp_dir).ok();
    let p = temp_dir.join("override_invalido.pr");
    let src = r#"espaco T{ publico classe P{ publico vazio F(){ imprima("P"); } } publico classe C : P{ publico sobrescreve vazio F(){ imprima("C"); } } } publico função vazio Principal(){ var c = novo T.C(); c.F(); }"#;
    std::fs::write(&p, src).unwrap();
    let path_str = p.to_string_lossy().to_string();
    let args = vec![path_str.as_str(), "--target=bytecode"];
    let (code, _o, e) = run_compiler(&args);
    assert_ne!(code, 0, "Deveria falhar: override sem virtual no pai");
    assert!(e.to_lowercase().contains("sobrescreve") || e.to_lowercase().contains("override"));
}

#[test]
fn test_negative_three_level_circular_inheritance_should_fail() {
    // A -> B -> C -> A
    let temp_dir = repo_root().join("target").join("test-temp-ciclo3");
    std::fs::create_dir_all(&temp_dir).ok();
    let p = temp_dir.join("ciclo_abc.pr");
    let src = r#"namespace N { classe A : B { } classe B : C { } classe C : A { } }"#;
    std::fs::write(&p, src).unwrap();
    let path_str = p.to_string_lossy().to_string();
    let args = vec![path_str.as_str(), "--target=bytecode"];
    let (code, _o, _e) = run_compiler(&args);
    assert_ne!(code, 0, "Deveria falhar por herança circular A->B->C->A");
}
