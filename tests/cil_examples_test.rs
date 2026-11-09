use std::fs;
use std::path::PathBuf;

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

fn parse_and_typecheck(files: &[&str]) -> compilador_portugues::ast::Programa {
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

    let mut tc = compilador_portugues::type_checker::VerificadorTipos::new();
    if let Err(erros) = tc.verificar_programa(&programa_final) {
        let error_strings: Vec<String> = erros.iter().map(|e| e.mensagem.clone()).collect();
        panic!(
            "Erros semânticos em {:?}:\n{}",
            files,
            error_strings.join("\n")
        );
    }
    programa_final
}

fn assert_cil_generates(files: &[&str]) {
    let programa = parse_and_typecheck(files);
    let mut tc = compilador_portugues::type_checker::VerificadorTipos::new();
    tc.verificar_programa(&programa)
        .expect("typecheck deveria passar");

    let cil = compilador_portugues::codegen::gerar_cil_puro(&programa, &tc, "teste_cil");

    assert!(
        cil.contains(".assembly teste_cil {}"),
        "CIL não contém cabeçalho de assembly para {:?}",
        files
    );
    assert!(
        cil.contains(".method public hidebysig static void Main()"),
        "CIL não contém Main() para {:?}",
        files
    );
    assert!(cil.len() > 100, "CIL muito curto/inesperado para {:?}", files);
}

#[test]
fn test_cil_all_exemplos() {
    for pr in list_exemplos() {
        eprintln!("[cil-test] Gerando CIL para: {}", pr);
        let content = fs::read_to_string(repo_root().join(&pr)).unwrap_or_default();
        if pr.ends_with("programa_principal.pr") {
            assert_cil_generates(&["exemplos/biblioteca.pr", "exemplos/programa_principal.pr"]);
        } else if content.contains("usando Sistema;") || content.contains("usando Sistema.") {
            continue;
        } else {
            assert_cil_generates(&[pr.as_str()]);
        }
    }
}

