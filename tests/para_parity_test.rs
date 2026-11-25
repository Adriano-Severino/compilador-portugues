use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn have_clang() -> bool {
    Command::new("clang").arg("--version").output().is_ok()
}

fn have_cil_toolchain() -> bool {
    if Command::new("dotnet").arg("--version").output().is_ok() {
        return true;
    }
    #[cfg(windows)]
    {
        if Path::new(r"C:\Windows\Microsoft.NET\Framework64\v4.0.30319\ilasm.exe").is_file() {
            return true;
        }
        if Path::new(r"C:\Windows\Microsoft.NET\Framework\v4.0.30319\ilasm.exe").is_file() {
            return true;
        }
    }
    Command::new("ilasm").arg("/?").output().is_ok()
}

fn run_compiler(args: &[&str]) -> (i32, String, String) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_compilador"));
    cmd.args(args);
    let out = cmd.output().expect("failed to run compilador");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

fn run_interpreter(pbc: &Path) -> (i32, String, String) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_interpretador"));
    cmd.arg(pbc);
    let out = cmd.output().expect("failed to run interpretador");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

fn run_exe(exe: &Path) -> (i32, String, String) {
    let out = Command::new(exe)
        .current_dir(repo_root())
        .stdout(Stdio::piped())
        .output()
        .expect("failed to run compiled exe");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

fn normalize(s: &str) -> String {
    s.replace("\r\n", "\n")
        .chars()
        .filter(|&c| c != ' ' && c != '\t')
        .collect()
}

fn exe_path(stem: &str) -> PathBuf {
    let root = repo_root();
    if cfg!(windows) {
        root.join(format!("{}.exe", stem))
    } else {
        root.join(stem)
    }
}

/// A.1.4 / A.2.2 — o mesmo programa com `para`/`para cada` deve produzir
/// saída idêntica nos backends bytecode, LLVM IR e CIL (.NET).
#[test]
fn para_paridade_entre_backends() {
    for pr in ["exemplos/teste_para.pr", "exemplos/para_cada.pr"] {
        paridade_backend(pr);
    }
}

fn paridade_backend(pr: &str) {
    let root = repo_root();
    let pr_path = root.join(pr);
    assert!(pr_path.exists(), "exemplo não encontrado: {}", pr_path.display());
    let stem = pr_path.file_stem().unwrap().to_str().unwrap().to_string();

    // --- bytecode ---
    let (bc_code, _bc_out, bc_err) = run_compiler(&[pr, "--target=bytecode"]);
    assert_eq!(bc_code, 0, "compilador bytecode falhou: {}", bc_err);
    let pbc = root.join("build").join(format!("{}.pbc", stem));
    assert!(pbc.exists(), "pbc não gerado: {}", pbc.display());
    let (bi_code, bi_out, bi_err) = run_interpreter(&pbc);
    assert_eq!(bi_code, 0, "interpretador falhou: {}", bi_err);
    let saida_bytecode = normalize(&bi_out);
    assert!(
        !saida_bytecode.trim().is_empty(),
        "bytecode não produziu saída"
    );

    // --- LLVM IR ---
    let mut saida_llvm = None;
    if have_clang() {
        let (ll_code, _ll_out, ll_err) = run_compiler(&[pr, "--target=llvm-ir"]);
        assert_eq!(ll_code, 0, "compilador llvm-ir falhou: {}", ll_err);
        let exe = exe_path(&stem);
        assert!(exe.exists(), "executável LLVM não encontrado: {:?}", exe);
        let (ll_run_code, ll_out, ll_err) = run_exe(&exe);
        assert_eq!(ll_run_code, 0, "execução LLVM falhou: {}", ll_err);
        saida_llvm = Some(normalize(&ll_out));
        let _ = fs::remove_file(&exe);
    } else {
        eprintln!("clang não encontrado; paridade LLVM ignorada.");
    }

    // --- CIL / .NET ---
    let mut saida_cil = None;
    if have_cil_toolchain() {
        let (cil_code, _cil_out, cil_err) = run_compiler(&[pr, "--target=dotnet"]);
        assert_eq!(cil_code, 0, "compilador cil/dotnet falhou: {}", cil_err);
        let exe = exe_path(&stem);
        assert!(exe.exists(), "executável CIL não encontrado: {:?}", exe);
        let (cil_run_code, cil_out, cil_err) = run_exe(&exe);
        assert_eq!(cil_run_code, 0, "execução CIL falhou: {}", cil_err);
        saida_cil = Some(normalize(&cil_out));
        let _ = fs::remove_file(&exe);
        let _ = fs::remove_file(root.join(format!("{}.il", stem)));
    } else {
        eprintln!("toolchain CIL não encontrada; paridade CIL ignorada.");
    }

    let esperada = saida_bytecode.trim().to_string();
    if let Some(ll) = &saida_llvm {
        assert_eq!(
            ll.trim(),
            esperada,
            "saída LLVM diverge do bytecode para {}",
            pr
        );
    }
    if let Some(cil) = &saida_cil {
        assert_eq!(
            cil.trim(),
            esperada,
            "saída CIL diverge do bytecode para {}",
            pr
        );
    }
}
