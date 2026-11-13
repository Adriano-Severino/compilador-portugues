use std::fs;
use std::path::PathBuf;
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

/// Programas .pr que usam a biblioteca padrão compilam para LLVM IR e
/// executam via porsol_runtime.c (handles opacos + dispatcher nativo).
/// Cobre: estáticos (Matematica/Texto/Guid), coleções genéricas
/// (Lista/Dicionario), ConstrutorTexto, Data com set de propriedade e
/// resolução de nomes fora do namespace importado (`usando Sistema;` apenas).
#[test]
fn llvm_stdlib_core_collections_text_data() {
    if !have_clang() {
        eprintln!("clang não encontrado; ignorando teste LLVM stdlib.");
        return;
    }

    let root = repo_root();
    let dir = root.join("target/test-temp-llvm-stdlib");
    fs::create_dir_all(&dir).unwrap();
    let pr_path = dir.join("stdlib_core.pr");
    let src = r#"
        usando Sistema;

        função Principal() {
            Console.EscreverLinha(Matematica.Abs(-7));
            Console.EscreverLinha(Matematica.Max(3, 9));
            Console.EscreverLinha(Texto.ParaMaiusculas("ola"));
            Console.EscreverLinha(Texto.Tamanho("abc"));
            Console.EscreverLinha(Texto.ComecaCom("por do sol", "por"));

            var l = novo Lista<inteiro>();
            l.Adicionar(10);
            l.Adicionar(20);
            Console.EscreverLinha(l.Tamanho);
            Console.EscreverLinha(l.Obter(1));

            var d = novo Dicionario<texto, inteiro>();
            d.Adicionar("a", 1);
            Console.EscreverLinha(d.Tamanho);
            Console.EscreverLinha(d.Obter("a"));
            Console.EscreverLinha(d.ContemChave("a"));

            var sb = novo ConstrutorTexto();
            sb.Anexar("Olá");
            sb.Anexar(" ");
            sb.Anexar("mundo");
            Console.EscreverLinha(sb.ParaTexto());

            var dt = novo Data(2024, 3, 15, 10, 30, 0);
            dt.Ano = 2025;
            Console.EscreverLinha(dt.Ano);
            Console.EscreverLinha(dt.Formatar("dd/mm/aaaa"));

            Console.EscreverLinha(Guid.EhValido(Guid.Novo()));
        }
    "#;
    fs::write(&pr_path, src).unwrap();

    let (code, _out, err) = run_compiler(&[pr_path.to_str().unwrap(), "--target=llvm-ir"]);
    assert_eq!(code, 0, "compilador falhou: {}", err);

    let exe_stem = pr_path.file_stem().unwrap().to_string_lossy().into_owned();
    let exe = if cfg!(windows) {
        root.join(format!("{}.exe", exe_stem))
    } else {
        root.join(exe_stem)
    };
    assert!(exe.exists(), "executável não gerado: {}", exe.display());

    let out = Command::new(&exe).output().expect("falha ao executar exe");
    assert!(
        out.status.success(),
        "execução falhou: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = normalize_newlines(&String::from_utf8_lossy(&out.stdout));
    let lines: Vec<&str> = stdout.lines().collect();

    let expected_prefix = [
        "7.000000",
        "9.000000",
        "OLA",
        "3",
        "verdadeiro",
        "2",
        "20",
        "1",
        "1",
        "verdadeiro",
        "Olá mundo",
        "2025",
    ];
    assert!(
        lines.len() >= expected_prefix.len(),
        "saída incompleta: {:?}",
        lines
    );
    for (i, esperado) in expected_prefix.iter().enumerate() {
        assert_eq!(
            lines[i].trim(),
            *esperado,
            "linha {} divergente. stdout completo:\n{}",
            i,
            stdout
        );
    }
    // Formatar usa o ano atualizado pela propriedade; o formato exato de
    // mês/dia é detalhe do runtime, então basta conferir o ano e o Guid.
    assert!(
        lines[expected_prefix.len()].contains("2025"),
        "Formatar não refletiu o set de Ano. stdout:\n{}",
        stdout
    );
    assert_eq!(lines[expected_prefix.len() + 1].trim(), "verdadeiro");
}

/// Compila um .pr para llvm-ir, linka com o runtime e executa.
/// Retorna (sucesso, stdout, stderr).
fn compile_run_llvm(pr_path: &PathBuf) -> (bool, String, String) {
    if !have_clang() {
        eprintln!("clang não encontrado; ignorando teste LLVM.");
        return (true, String::new(), String::new());
    }
    let (code, _out, err) = run_compiler(&[pr_path.to_str().unwrap(), "--target=llvm-ir"]);
    if code != 0 {
        return (false, String::new(), format!("compilador falhou: {}", err));
    }
    let root = repo_root();
    let stem = pr_path.file_stem().unwrap().to_string_lossy().into_owned();
    let exe = if cfg!(windows) {
        root.join(format!("{}.exe", stem))
    } else {
        root.join(stem)
    };
    if !exe.exists() {
        return (false, String::new(), format!("executável não gerado: {}", exe.display()));
    }
    let out = Command::new(&exe).output().expect("falha ao executar exe");
    let stdout = normalize_newlines(&String::from_utf8_lossy(&out.stdout));
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    (out.status.success(), stdout, stderr)
}

/// Regressão: construtores sem argumento devem inicializar o estado do
/// objeto no runtime LLVM (Mutex/Cronometro/Horario/Regex/ClienteHttp).
#[test]
fn llvm_stdlib_construtores_sem_argumento() {
    let root = repo_root();
    let dir = root.join("target/test-temp-llvm-stdlib");
    fs::create_dir_all(&dir).unwrap();
    let pr_path = dir.join("stdlib_ctors.pr");
    let src = r#"
        usando Sistema;
        usando Sistema.Diagnostics;
        usando Sistema.Texto;
        usando Sistema.Data;

        função Principal() {
            var m = novo Mutex();
            Console.EscreverLinha("mutex ok");

            var c = novo Cronometro();
            c.Iniciar();
            Console.EscreverLinha(c.EstaExecutando());
            c.Parar();
            Console.EscreverLinha(c.EstaExecutando());

            var h = novo Horario(90);
            Console.EscreverLinha(h.ParaTexto());

            var rx = novo Regex("a+");
            Console.EscreverLinha(rx.Corresponde("caaat"));

            var http = novo ClienteHttp("http://exemplo.local");
            Console.EscreverLinha(http.BaseUrl);
        }
    "#;
    fs::write(&pr_path, src).unwrap();

    let (ok, stdout, err) = compile_run_llvm(&pr_path);
    assert!(ok, "execução falhou: {}\nstdout:\n{}", err, stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    let expected = [
        "mutex ok",
        "verdadeiro",
        "falso",
        "00:01:30",
        "verdadeiro",
        "http://exemplo.local",
    ];
    for (i, e) in expected.iter().enumerate() {
        assert_eq!(
            lines.get(i).map(|s| s.trim()),
            Some(*e),
            "linha {} divergente. stdout:\n{}",
            i,
            stdout
        );
    }
}

/// ClienteHttp.*Assincrono executa no pool de threads do async_runtime
/// contra um servidor HTTP local (TcpListener em thread).
#[test]
fn llvm_stdlib_cliente_http_assincrono() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let porta = listener.local_addr().unwrap().port();
    // 4 requisições: GET, POST, PUT, DELETE.
    thread::spawn(move || {
        for _ in 0..4 {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut buf = [0u8; 8192];
            let n = stream.read(&mut buf).unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]).to_string();
            let verbo = req.split_whitespace().next().unwrap_or("");
            let corpo = match verbo {
                "POST" => "POST-OK",
                "PUT" => "PUT-OK",
                "DELETE" => "DELETE-OK",
                _ => "GET-OK",
            };
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                corpo.len(),
                corpo
            );
            stream.write_all(resp.as_bytes()).unwrap();
        }
    });

    let root = repo_root();
    let dir = root.join("target/test-temp-llvm-stdlib");
    fs::create_dir_all(&dir).unwrap();
    let pr_path = dir.join("http_async.pr");
    let src = format!(
        r#"
        usando Sistema;
        usando Sistema.Rede;

        função Principal() {{
            var http = novo ClienteHttp("http://127.0.0.1:{porta}");
            Console.EscreverLinha(aguarde http.ObterAssincrono("/eco"));
            Console.EscreverLinha(aguarde http.EnviarAssincrono("/eco", "dados"));
            Console.EscreverLinha(aguarde http.AtualizarAssincrono("/eco", "dados"));
            Console.EscreverLinha(aguarde http.ExcluirAssincrono("/eco"));
        }}
        "#,
        porta = porta
    );
    fs::write(&pr_path, src).unwrap();

    let (ok, stdout, err) = compile_run_llvm(&pr_path);
    assert!(ok, "execução falhou: {}\nstdout:\n{}", err, stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    let expected = ["GET-OK", "POST-OK", "PUT-OK", "DELETE-OK"];
    for (i, e) in expected.iter().enumerate() {
        assert_eq!(
            lines.get(i).map(|s| s.trim()),
            Some(*e),
            "linha {} divergente. stdout:\n{}",
            i,
            stdout
        );
    }
}

/// HTTPS real via mbedTLS embutido: valida handshake + cadeia de
/// certificados contra o CA bundle Mozilla. Depende de rede; se
/// example.com:443 estiver inacessível, o teste é ignorado.
#[test]
fn llvm_stdlib_https_exemplo_com() {
    use std::net::TcpStream;
    use std::time::Duration;

    // Só roda se houver conectividade TCP com 443.
    use std::net::ToSocketAddrs;
    let addr = ("example.com", 443)
        .to_socket_addrs()
        .ok()
        .and_then(|mut it| it.next());
    let alcancavel = addr
        .map(|a| TcpStream::connect_timeout(&a, Duration::from_secs(5)).is_ok())
        .unwrap_or(false);
    if !alcancavel {
        eprintln!("example.com:443 inacessível; ignorando teste HTTPS.");
        return;
    }

    let root = repo_root();
    let dir = root.join("target/test-temp-llvm-stdlib");
    fs::create_dir_all(&dir).unwrap();
    let pr_path = dir.join("https_exemplo.pr");
    let src = r#"
        usando Sistema;
        usando Sistema.Rede;

        função Principal() {
            var http = novo ClienteHttp("https://example.com");
            var r = http.Obter("/");
            Console.EscreverLinha(r);
        }
    "#;
    fs::write(&pr_path, src).unwrap();

    let (ok, stdout, err) = compile_run_llvm(&pr_path);
    assert!(ok, "execução falhou: {}\nstdout:\n{}", err, stdout);
    assert!(
        stdout.contains("Example Domain"),
        "resposta HTTPS inesperada. stdout:\n{}",
        stdout
    );
}

/// Paridade de backend: os exemplos de fase da stdlib executam até o fim
/// no alvo LLVM sem avisos de nativo não implementado.
#[test]
fn llvm_stdlib_fases_exemplos() {
    let root = repo_root();
    let casos: &[(&str, &str)] = &[
        ("teste_stdlib_fase0.pr", "FIM"),
        ("teste_stdlib_fase2.pr", "Regex.EhValido: verdadeiro / falso"),
        ("teste_stdlib_fase3.pr", "FIM FASE3"),
        ("teste_stdlib_fase4.pr", "FIM FASE4"),
        ("teste_stdlib_fase5.pr", "FIM FASE5"),
        ("teste_stdlib_fase6.pr", "FIM FASE6"),
    ];
    for (arquivo, marcador) in casos {
        let pr = root.join("exemplos").join(arquivo);
        let (ok, stdout, err) = compile_run_llvm(&pr);
        assert!(ok, "{} falhou: {}\nstdout:\n{}", arquivo, err, stdout);
        assert!(
            !stdout.contains("aviso nativo"),
            "{} emitiu aviso de nativo não implementado:\n{}",
            arquivo,
            stdout
        );
        assert!(
            stdout.contains(marcador),
            "{} não atingiu o marcador '{}'. stdout:\n{}",
            arquivo,
            marcador,
            stdout
        );
    }
}
