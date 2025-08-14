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

fn assert_example_ok(pr: &str, expected_out: Option<&str>) {
    let root = repo_root();
    let pr_path = root.join(pr);
    assert!(pr_path.exists(), "example not found: {}", pr_path.display());

    // Compile to bytecode and run
    let pbc = pr_path.with_extension("pbc");
    let (c_code, _c_out, c_err) = run_compiler(&[pr, "--target=bytecode"]);
    assert_eq!(c_code, 0, "compiler failed for {}: {}", pr, c_err);
    assert!(pbc.exists(), "pbc not generated: {}", pbc.display());

    let (i_code, i_out, i_err) = run_interpreter(&pbc);
    assert_eq!(i_code, 0, "interpreter failed for {}: {}", pr, i_err);

    if let Some(expected) = expected_out {
        let expected_norm = normalize_newlines(expected);
        let got = normalize_newlines(&i_out);
        assert_eq!(got.trim(), expected_norm.trim(), "wrong output for {}", pr);
    }
}

#[test]
fn test_examples_with_expected_out() {
    let cases = [
        (
            "exemplos/aritmetica.pr",
            include_str!("../../aritmetica.out.txt"),
        ),
        ("exemplos/classes.pr", include_str!("../../classes.out.txt")),
        (
            "exemplos/condicionais.pr",
            include_str!("../../condicionais.out.txt"),
        ),
        ("exemplos/funcao.pr", include_str!("../../funcao.out.txt")),
        ("exemplos/heranca.pr", include_str!("../../heranca.out.txt")),
        ("exemplos/loops.pr", include_str!("../../loops.out.txt")),
        (
            "exemplos/teste_decimal.pr",
            include_str!("../../teste_decimal.out.txt"),
        ),
        (
            "exemplos/teste_decimal2.pr",
            include_str!("../../teste_decimal2.out.txt"),
        ),
        (
            "exemplos/teste_static_property.pr",
            include_str!("../../teste_static_property.out.txt"),
        ),
    ];
    for (pr, out) in cases {
        assert_example_ok(pr, Some(out));
    }
}

#[test]
fn test_examples_without_expected_out() {
    let cases = [
        "exemplos/teste.pr",
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
    ];
    for pr in cases {
        assert_example_ok(pr, None);
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
