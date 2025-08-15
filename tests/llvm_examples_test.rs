use std::fs;
use std::path::{Path, PathBuf};

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

/// Lê N arquivos .pr, parseia, mescla em um único Programa e roda verificação de tipos.
fn parse_and_typecheck(files: &[&str]) -> compilador_portugues::ast::Programa {
    // Carrega e sanitiza código como em main.rs
    fn sanitizar_codigo(orig: String) -> String {
        let mut resultado = String::with_capacity(orig.len());
        for linha in orig.lines() {
            let mut corte = linha.len();
            for marcador in [
                ";cargo ",
                ";dotnet ",
                ";clang ",
                ";echo ",
                ";./",
                ";interpretador ",
            ] {
                if let Some(idx) = linha.find(marcador) {
                    if idx < corte {
                        corte = idx + 1;
                    }
                }
            }
            if corte < linha.len() {
                resultado.push_str(&linha[..corte]);
            } else {
                resultado.push_str(linha);
            }
            resultado.push('\n');
        }
        resultado
    }

    let root = repo_root();
    let mut programas = Vec::new();
    for f in files {
        let p = root.join(f);
        let codigo = fs::read_to_string(&p).expect("falha ao ler exemplo");
        let mut compilador = compilador_portugues::CompiladorPortugues::new();
        let programa = compilador
            .compilar_codigo(&sanitizar_codigo(codigo))
            .expect("parse falhou");
        programas.push(programa);
    }

    // Mescla como em main.rs
    let mut programa_final = compilador_portugues::ast::Programa {
        usings: vec![],
        namespaces: vec![],
        declaracoes: vec![],
    };
    for mut ast in programas {
        programa_final.declaracoes.extend(ast.declaracoes);
        programa_final.usings.extend(ast.usings);
        for ns_para_mesclar in ast.namespaces.drain(..) {
            if let Some(ns_existente) = programa_final
                .namespaces
                .iter_mut()
                .find(|n| n.nome == ns_para_mesclar.nome)
            {
                ns_existente.declaracoes.extend(ns_para_mesclar.declaracoes);
            } else {
                programa_final.namespaces.push(ns_para_mesclar);
            }
        }
    }

    // Verificação de tipos
    let mut tc = compilador_portugues::type_checker::VerificadorTipos::new();
    if let Err(erros) = tc.verificar_programa(&programa_final) {
        panic!("Erros semânticos em {:?}:\n{}", files, erros.join("\n"));
    }
    programa_final
}

/// Gera LLVM IR diretamente (sem invocar clang) e valida que não panica e não é vazio.
fn assert_llvm_ir_generates(files: &[&str]) {
    let programa = parse_and_typecheck(files);
    let mut tc = compilador_portugues::type_checker::VerificadorTipos::new();
    tc.verificar_programa(&programa)
        .expect("typecheck deveria passar");

    let ir = compilador_portugues::codegen::gerar_llvm_ir_puro(&programa, &mut tc);

    assert!(
        ir.contains("define i32 @main()"),
        "IR não contém main() para {:?}",
        files
    );
    assert!(ir.len() > 100, "IR muito curto/inesperado para {:?}", files);
}

#[test]
fn test_llvm_ir_all_exemplos() {
    // Descobre todos os exemplos e gera LLVM IR para cada um.
    for pr in list_exemplos() {
        eprintln!("[llvm-test] Gerando IR para: {}", pr);
        if pr.ends_with("programa_principal.pr") {
            assert_llvm_ir_generates(&["exemplos/programa_principal.pr", "exemplos/biblioteca.pr"]);
        } else {
            assert_llvm_ir_generates(&[pr.as_str()]);
        }
    }
}
