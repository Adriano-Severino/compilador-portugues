use crate::carregador::*;
use crate::debug::*;
use crate::objetos::*;
use crate::tipos::VM;
use crate::tipos::*;
use crate::util::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rust_decimal::Decimal;

// ===== Utilitários compartilhados entre os dispatchers nativos =====

/// Converte um Valor para String preservando Texto sem formatação extra.
fn texto_de(v: &Valor) -> String {
    match v {
        Valor::Texto(s) => s.clone(),
        outro => outro.to_string(),
    }
}

/// Extrai um f64 de qualquer Valor numérico.
fn numero_de(v: &Valor) -> f64 {
    match v {
        Valor::Inteiro(i) => *i as f64,
        Valor::Flutuante(f) => *f as f64,
        Valor::Duplo(d) => *d,
        Valor::Decimal(d) => d.to_string().parse().unwrap_or(0.0),
        _ => 0.0,
    }
}

/// Retorna os campos mutáveis de um Valor::Objeto, se for o caso.
fn campos_de(este: &Valor) -> Option<Rc<RefCell<HashMap<String, Valor>>>> {
    if let Valor::Objeto { campos, .. } = este {
        Some(campos.clone())
    } else {
        None
    }
}

/// Lê um array armazenado em um campo interno do objeto (ex.: "__itens__").
fn ler_array_interno(campos: &Rc<RefCell<HashMap<String, Valor>>>, chave: &str) -> Vec<Valor> {
    match campos.borrow().get(chave) {
        Some(Valor::Array(v)) => v.clone(),
        _ => Vec::new(),
    }
}

/// Escreve um array em um campo interno do objeto.
fn escrever_array_interno(
    campos: &Rc<RefCell<HashMap<String, Valor>>>,
    chave: &str,
    itens: Vec<Valor>,
) {
    campos
        .borrow_mut()
        .insert(chave.to_string(), Valor::Array(itens));
}

/// Extrai itens de um Valor que pode ser Lista (Objeto) ou Array puro.
fn itens_de_lista(v: &Valor) -> Vec<Valor> {
    match v {
        Valor::Objeto { campos, .. } => ler_array_interno(campos, "__itens__"),
        Valor::Array(itens) => itens.clone(),
        _ => Vec::new(),
    }
}

/// Compila o padrão de um objeto Regex a partir do campo interno "__padrao__".
fn regex_do_objeto(este: &Valor) -> Option<regex::Regex> {
    if let Some(campos) = campos_de(este) {
        if let Some(Valor::Texto(padrao)) = campos.borrow().get("__padrao__") {
            return regex::Regex::new(padrao).ok();
        }
    }
    None
}

// ===== Codificação Base64 / hexadecimal (sem dependências externas) =====

const BASE64_ALFABETO: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_codificar(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(BASE64_ALFABETO[(n >> 18) as usize & 63] as char);
        out.push(BASE64_ALFABETO[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            BASE64_ALFABETO[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            BASE64_ALFABETO[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

fn base64_valor(c: u8) -> Option<u8> {
    match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

/// Decodifica Base64; retorna None se houver caractere inválido.
fn base64_decodificar(texto: &str) -> Option<Vec<u8>> {
    let limpo: Vec<u8> = texto.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    if limpo.is_empty() {
        return Some(Vec::new());
    }
    if limpo.len() % 4 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(limpo.len() / 4 * 3);
    for chunk in limpo.chunks(4) {
        let mut vals = [0u8; 4];
        let mut padding = 0;
        for (i, &c) in chunk.iter().enumerate() {
            if c == b'=' {
                if i < 2 {
                    return None;
                }
                padding += 1;
                vals[i] = 0;
            } else {
                if padding > 0 {
                    return None;
                }
                vals[i] = base64_valor(c)?;
            }
        }
        if padding > 2 {
            return None;
        }
        let n = ((vals[0] as u32) << 18)
            | ((vals[1] as u32) << 12)
            | ((vals[2] as u32) << 6)
            | vals[3] as u32;
        out.push((n >> 16) as u8);
        if padding < 2 {
            out.push((n >> 8) as u8);
        }
        if padding < 1 {
            out.push(n as u8);
        }
    }
    Some(out)
}

fn hex_valor(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

fn hex_codificar(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(char::from_digit((b >> 4) as u32, 16).unwrap());
        out.push(char::from_digit((b & 0xF) as u32, 16).unwrap());
    }
    out
}

/// Decodifica hexadecimal; retorna None se houver caractere inválido ou
/// quantidade ímpar de dígitos.
fn hex_decodificar(texto: &str) -> Option<Vec<u8>> {
    let limpo: Vec<u8> = texto.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    if limpo.len() % 2 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(limpo.len() / 2);
    for par in limpo.chunks(2) {
        out.push((hex_valor(par[0])? << 4) | hex_valor(par[1])?);
    }
    Some(out)
}

/// Comparação total para ordenação de coleções (numéricos de tipos mistos comparáveis).
fn numero_opcional(v: &Valor) -> Option<f64> {
    match v {
        Valor::Inteiro(i) => Some(*i as f64),
        Valor::Flutuante(f) => Some(*f as f64),
        Valor::Duplo(d) => Some(*d),
        Valor::Decimal(d) => d.to_string().parse().ok(),
        _ => None,
    }
}

/// Milissegundos desde a época Unix (usado pelo Cronometro).
fn agora_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

// ===== Registro global de sockets TCP =====
// `Valor` não comporta TcpStream/TcpListener; os handles nativos ficam em
// registros globais indexados por um id guardado nos campos internos do objeto.

static PROXIMO_SOCKET_ID: AtomicU64 = AtomicU64::new(1);
static SOCKETS_TCP: Mutex<Option<HashMap<u64, TcpStream>>> = Mutex::new(None);
static SERVIDORES_TCP: Mutex<Option<HashMap<u64, TcpListener>>> = Mutex::new(None);

fn proximo_id_socket() -> u64 {
    PROXIMO_SOCKET_ID.fetch_add(1, AtomicOrdering::Relaxed)
}

fn registrar_socket(stream: TcpStream) -> u64 {
    let id = proximo_id_socket();
    let mut guard = SOCKETS_TCP.lock().unwrap();
    guard.get_or_insert_with(HashMap::new).insert(id, stream);
    id
}

fn remover_socket(id: u64) {
    if let Some(mapa) = SOCKETS_TCP.lock().unwrap().as_mut() {
        mapa.remove(&id);
    }
}

fn com_socket<R>(id: u64, f: impl FnOnce(&mut TcpStream) -> R) -> Option<R> {
    let mut guard = SOCKETS_TCP.lock().unwrap();
    guard
        .as_mut()
        .and_then(|mapa| mapa.get_mut(&id))
        .map(f)
}

fn registrar_servidor(listener: TcpListener) -> u64 {
    let id = proximo_id_socket();
    let mut guard = SERVIDORES_TCP.lock().unwrap();
    guard
        .get_or_insert_with(HashMap::new)
        .insert(id, listener);
    id
}

fn remover_servidor(id: u64) {
    if let Some(mapa) = SERVIDORES_TCP.lock().unwrap().as_mut() {
        mapa.remove(&id);
    }
}

fn com_servidor<R>(id: u64, f: impl FnOnce(&mut TcpListener) -> R) -> Option<R> {
    let mut guard = SERVIDORES_TCP.lock().unwrap();
    guard
        .as_mut()
        .and_then(|mapa| mapa.get_mut(&id))
        .map(f)
}

fn objeto_socket_tcp(id: u64) -> Valor {
    let mut campos = HashMap::new();
    campos.insert("__socket_id__".to_string(), Valor::Inteiro(id as i64));
    Valor::Objeto {
        nome_classe: "Sistema.Rede.SocketTcp".to_string(),
        campos: Rc::new(RefCell::new(campos)),
        metodos: HashMap::new(),
    }
}

fn objeto_servidor_tcp(id: u64) -> Valor {
    let mut campos = HashMap::new();
    campos.insert("__servidor_id__".to_string(), Valor::Inteiro(id as i64));
    Valor::Objeto {
        nome_classe: "Sistema.Rede.ServidorTcp".to_string(),
        campos: Rc::new(RefCell::new(campos)),
        metodos: HashMap::new(),
    }
}

fn socket_id_de(este: &Valor) -> Option<u64> {
    if let Some(campos) = campos_de(este) {
        match campos.borrow().get("__socket_id__") {
            Some(Valor::Inteiro(n)) if *n > 0 => return Some(*n as u64),
            _ => {}
        }
    }
    None
}

/// Cria um objeto de coleção da stdlib com o array interno `__itens__`.
fn objeto_com_itens(nome_classe: &str, itens: Vec<Valor>) -> Valor {
    let mut campos = HashMap::new();
    campos.insert("__itens__".to_string(), Valor::Array(itens));
    Valor::Objeto {
        nome_classe: nome_classe.to_string(),
        campos: Rc::new(RefCell::new(campos)),
        metodos: HashMap::new(),
    }
}

/// Cria um objeto Horario (Sistema.Data.Horario) a partir do total de segundos.
fn objeto_horario(segundos: i64) -> Valor {
    let mut campos = HashMap::new();
    campos.insert("__segundos__".to_string(), Valor::Inteiro(segundos));
    Valor::Objeto {
        nome_classe: "Sistema.Data.Horario".to_string(),
        campos: Rc::new(RefCell::new(campos)),
        metodos: HashMap::new(),
    }
}

/// Extrai o total de segundos de um objeto Horario.
fn segundos_de_horario(v: &Valor) -> i64 {
    if let Valor::Objeto { campos, .. } = v {
        if let Some(Valor::Inteiro(n)) = campos.borrow().get("__segundos__") {
            return *n;
        }
    }
    0
}

/// Formata um Horario como "HH:MM:SS" ou "d.HH:MM:SS".
fn horario_para_texto(segundos: i64) -> String {
    let total = segundos.unsigned_abs();
    let dias = total / 86400;
    let horas = (total / 3600) % 24;
    let minutos = (total / 60) % 60;
    let segs = total % 60;
    let base = if dias > 0 {
        format!("{}.{:02}:{:02}:{:02}", dias, horas, minutos, segs)
    } else {
        format!("{:02}:{:02}:{:02}", horas, minutos, segs)
    };
    if segundos < 0 {
        format!("-{}", base)
    } else {
        base
    }
}

fn servidor_id_de(este: &Valor) -> Option<u64> {
    if let Some(campos) = campos_de(este) {
        match campos.borrow().get("__servidor_id__") {
            Some(Valor::Inteiro(n)) if *n > 0 => return Some(*n as u64),
            _ => {}
        }
    }
    None
}

fn comparar_valores(a: &Valor, b: &Valor) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match (a, b) {
        (Valor::Texto(x), Valor::Texto(y)) => x.cmp(y),
        (Valor::Booleano(x), Valor::Booleano(y)) => x.cmp(y),
        (Valor::Decimal(x), Valor::Decimal(y)) => x.cmp(y),
        _ => match (numero_opcional(a), numero_opcional(b)) {
            (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(Ordering::Equal),
            _ => Ordering::Equal,
        },
    }
}

// ===== Dicionário: __itens__ = Array de pares Array[chave, valor] =====

fn dicionario_encontrar(itens: &[Valor], chave: &Valor) -> Option<usize> {
    itens.iter().position(|par| match par {
        Valor::Array(v) if v.len() >= 2 => v[0] == *chave,
        _ => false,
    })
}

fn dicionario_obter(itens: &[Valor], chave: &Valor) -> Valor {
    match dicionario_encontrar(itens, chave) {
        Some(i) => match &itens[i] {
            Valor::Array(v) => v.get(1).cloned().unwrap_or(Valor::Nulo),
            _ => Valor::Nulo,
        },
        None => Valor::Nulo,
    }
}

fn dicionario_adicionar(itens: &mut Vec<Valor>, chave: Valor, valor: Valor) {
    if let Some(i) = dicionario_encontrar(itens, &chave) {
        itens[i] = Valor::Array(vec![chave, valor]);
    } else {
        itens.push(Valor::Array(vec![chave, valor]));
    }
}

// ===== JSON: conversão Valor <-> serde_json::Value =====

fn valor_para_json(v: &Valor) -> serde_json::Value {
    match v {
        Valor::Inteiro(i) => serde_json::Value::from(*i),
        Valor::Flutuante(f) => serde_json::json!(*f as f64),
        Valor::Duplo(d) => serde_json::json!(*d),
        Valor::Decimal(d) => d
            .to_string()
            .parse::<f64>()
            .map(serde_json::Value::from)
            .unwrap_or_else(|_| serde_json::Value::from(d.to_string())),
        Valor::Texto(s) => serde_json::Value::from(s.clone()),
        Valor::Booleano(b) => serde_json::Value::from(*b),
        Valor::Nulo => serde_json::Value::Null,
        Valor::Array(items) => {
            serde_json::Value::Array(items.iter().map(valor_para_json).collect())
        }
        Valor::Objeto { campos, .. } => {
            let borrow = campos.borrow();
            // Dicionário: __itens__ = pares [chave, valor]; serializa como objeto JSON
            // quando todas as chaves são texto; caso contrário, como array de pares.
            if let Some(Valor::Array(pares)) = borrow.get("__itens__") {
                let todas_chaves_texto = pares.iter().all(|par| match par {
                    Valor::Array(v) if v.len() >= 2 => matches!(v[0], Valor::Texto(_)),
                    _ => true,
                });
                if todas_chaves_texto && !pares.is_empty() {
                    let mut mapa = serde_json::Map::new();
                    for par in pares {
                        if let Valor::Array(v) = par {
                            if let (Valor::Texto(chave), valor) = (&v[0], &v[1]) {
                                mapa.insert(chave.clone(), valor_para_json(valor));
                            }
                        }
                    }
                    return serde_json::Value::Object(mapa);
                }
                if !pares.is_empty() {
                    return serde_json::Value::Array(
                        pares
                            .iter()
                            .map(|par| match par {
                                Valor::Array(v) if v.len() >= 2 => serde_json::json!([
                                    valor_para_json(&v[0]),
                                    valor_para_json(&v[1])
                                ]),
                                outro => valor_para_json(outro),
                            })
                            .collect(),
                    );
                }
            }
            let mapa: serde_json::Map<String, serde_json::Value> = borrow
                .iter()
                .filter(|(k, _)| !k.starts_with("__"))
                .map(|(k, v)| (k.clone(), valor_para_json(v)))
                .collect();
            serde_json::Value::Object(mapa)
        }
        Valor::Task { .. } => serde_json::Value::Null,
    }
}

fn json_para_valor(j: &serde_json::Value) -> Valor {
    match j {
        serde_json::Value::Null => Valor::Nulo,
        serde_json::Value::Bool(b) => Valor::Booleano(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Valor::Inteiro(i)
            } else {
                Valor::Duplo(n.as_f64().unwrap_or(0.0))
            }
        }
        serde_json::Value::String(s) => Valor::Texto(s.clone()),
        serde_json::Value::Array(items) => {
            Valor::Array(items.iter().map(json_para_valor).collect())
        }
        serde_json::Value::Object(mapa) => {
            let mut campos = HashMap::new();
            for (k, v) in mapa {
                campos.insert(k.clone(), json_para_valor(v));
            }
            Valor::Objeto {
                nome_classe: "Sistema.Texto.JsonObjeto".to_string(),
                campos: Rc::new(RefCell::new(campos)),
                metodos: HashMap::new(),
            }
        }
    }
}

// ===== HTTP mínimo (HTTP/1.1 sobre TCP) =====

/// Extrai (esquema, host, porta, caminho) de uma URL http(s):// simples.
fn parse_url_http(url: &str) -> Result<(String, String, u16, String), String> {
    let (esquema, sem_esquema, porta_padrao) = if let Some(rest) = url.strip_prefix("https://") {
        ("https", rest, 443u16)
    } else if let Some(rest) = url.strip_prefix("http://") {
        ("http", rest, 80u16)
    } else {
        return Err(format!("URL não suportada (apenas http:// ou https://): {}", url));
    };
    let (host_port, caminho) = match sem_esquema.find('/') {
        Some(i) => (&sem_esquema[..i], &sem_esquema[i..]),
        None => (sem_esquema, "/"),
    };
    let (host, porta) = match host_port.rsplit_once(':') {
        Some((h, p)) => {
            let porta: u16 = p
                .parse()
                .map_err(|_| format!("Porta inválida em URL: {}", url))?;
            (h.to_string(), porta)
        }
        None => (host_port.to_string(), porta_padrao),
    };
    if host.is_empty() {
        return Err(format!("Host vazio em URL: {}", url));
    }
    Ok((esquema.to_string(), host, porta, caminho.to_string()))
}

fn montar_requisicao_http(metodo: &str, esquema: &str, host: &str, porta: u16, caminho: &str, corpo: &str) -> String {
    let porta_padrao = if esquema == "https" { 443 } else { 80 };
    let host_header = if porta == porta_padrao {
        host.to_string()
    } else {
        format!("{}:{}", host, porta)
    };
    format!(
        "{} {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: PorDoSol/1.0\r\nAccept: */*\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        metodo,
        caminho,
        host_header,
        corpo.len(),
        corpo
    )
}

/// Extrai o corpo de uma resposta HTTP/1.1 crua (trata Content-Length e chunked básico).
fn extrair_corpo_http(resposta: &str) -> String {
    let (cabecalho, corpo) = match resposta.find("\r\n\r\n") {
        Some(i) => (&resposta[..i], &resposta[i + 4..]),
        None => return resposta.to_string(),
    };
    let lower = cabecalho.to_lowercase();
    if lower.contains("transfer-encoding: chunked") {
        decodificar_chunked(corpo)
    } else {
        // Content-Length: ignora bytes além do declarado, se houver
        if let Some(pos) = lower.find("content-length:") {
            if let Some(fim) = cabecalho[pos..].find('\r') {
                let valor: usize = cabecalho[pos + 15..pos + fim]
                    .trim()
                    .parse()
                    .unwrap_or(corpo.len());
                return corpo.chars().take(valor).collect();
            }
        }
        corpo.to_string()
    }
}

fn decodificar_chunked(corpo: &str) -> String {
    let mut saida = String::new();
    let mut resto = corpo;
    loop {
        let fim_linha = match resto.find("\r\n") {
            Some(i) => i,
            None => break,
        };
        let tamanho_str = resto[..fim_linha].trim();
        // ignora extensões de chunk (ex.: "1a;foo=bar")
        let tamanho_str = tamanho_str.split(';').next().unwrap_or("0");
        let tamanho = match usize::from_str_radix(tamanho_str, 16) {
            Ok(t) => t,
            Err(_) => break,
        };
        if tamanho == 0 {
            break;
        }
        resto = &resto[fim_linha + 2..];
        if resto.len() < tamanho + 2 {
            saida.push_str(&resto[..resto.len().min(tamanho)]);
            break;
        }
        saida.push_str(&resto[..tamanho]);
        resto = &resto[tamanho + 2..];
    }
    saida
}

/// Envia a requisição HTTP e lê a resposta sobre um stream já estabelecido
/// (TCP puro ou TLS). Genérico para servir aos dois transportes.
fn http_trocar<S: Read + Write>(
    stream: &mut S,
    metodo: &str,
    esquema: &str,
    host: &str,
    porta: u16,
    caminho: &str,
    corpo: &str,
) -> Result<String, String> {
    let requisicao = montar_requisicao_http(metodo, esquema, host, porta, caminho, corpo);
    stream
        .write_all(requisicao.as_bytes())
        .map_err(|e| format!("HTTP: falha ao enviar requisição: {}", e))?;
    let mut resposta = String::new();
    stream
        .read_to_string(&mut resposta)
        .map_err(|e| format!("HTTP: falha ao ler resposta: {}", e))?;
    Ok(extrair_corpo_http(&resposta))
}

fn http_request_sync(metodo: &str, url: &str, corpo: &str) -> Result<String, String> {
    let (esquema, host, porta, caminho) = parse_url_http(url)?;
    let addr = format!("{}:{}", host, porta);
    let mut stream = TcpStream::connect(addr.as_str())
        .map_err(|e| format!("HTTP: falha ao conectar em {}: {}", addr, e))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .ok();
    stream
        .set_write_timeout(Some(Duration::from_secs(30)))
        .ok();
    if esquema == "https" {
        let connector = native_tls::TlsConnector::new()
            .map_err(|e| format!("HTTP: falha ao iniciar TLS: {}", e))?;
        let mut tls = connector
            .connect(&host, stream)
            .map_err(|e| format!("HTTP: handshake TLS falhou para {}: {}", host, e))?;
        return http_trocar(&mut tls, metodo, &esquema, &host, porta, &caminho, corpo);
    }
    http_trocar(&mut stream, metodo, &esquema, &host, porta, &caminho, corpo)
}

async fn http_request_async(metodo: &str, url: &str, corpo: &str) -> Result<String, String> {
    // Executa o cliente HTTP síncrono em uma thread bloqueante do tokio,
    // evitando depender da feature "net" do tokio.
    let metodo = metodo.to_string();
    let url = url.to_string();
    let corpo = corpo.to_string();
    tokio::task::spawn_blocking(move || http_request_sync(&metodo, &url, &corpo))
        .await
        .map_err(|e| format!("HTTP: tarefa assíncrona falhou: {}", e))?
}

/// Resolve a URL efetiva combinando BaseUrl do objeto (se houver) com a URL do argumento.
fn resolver_url_com_base(este: &Valor, url: &str) -> String {
    if url.starts_with("http://") || url.starts_with("https://") {
        return url.to_string();
    }
    if let Some(campos) = campos_de(este) {
        if let Some(Valor::Texto(base)) = campos.borrow().get("BaseUrl") {
            let base = base.trim_end_matches('/');
            return format!("{}/{}", base, url.trim_start_matches('/'));
        }
    }
    url.to_string()
}

pub(crate) fn chrono_or_default_date() -> String {
    // Data/hora atual via SystemTime
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    data_hora_de_unix(secs)
}

/// Converte segundos desde 1970-01-01T00:00:00Z em "aaaa-mm-ddTHH:MM:SS".
/// Algoritmo civil de Hinnant, sem deps externas.
fn data_hora_de_unix(secs: u64) -> String {
    let dias = (secs / 86400) as i64;
    let segundos_do_dia = secs % 86400;
    let hora = segundos_do_dia / 3600;
    let minuto = (segundos_do_dia % 3600) / 60;
    let segundo = segundos_do_dia % 60;

    // civil_from_days (Howard Hinnant)
    let z = dias + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let dia = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let mes = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let ano = if mes <= 2 { y + 1 } else { y };

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
        ano, mes, dia, hora, minuto, segundo
    )
}

/// Constrói um Valor::Objeto representando Sistema.Data.Data a partir de campos.
fn objeto_data(
    ano: i64,
    mes: i64,
    dia: i64,
    hora: i64,
    minuto: i64,
    segundo: i64,
) -> Valor {
    let mut campos = HashMap::new();
    campos.insert("Ano".to_string(), Valor::Inteiro(ano));
    campos.insert("Mes".to_string(), Valor::Inteiro(mes));
    campos.insert("Dia".to_string(), Valor::Inteiro(dia));
    campos.insert("Hora".to_string(), Valor::Inteiro(hora));
    campos.insert("Minuto".to_string(), Valor::Inteiro(minuto));
    campos.insert("Segundo".to_string(), Valor::Inteiro(segundo));
    Valor::Objeto {
        nome_classe: "Sistema.Data.Data".to_string(),
        campos: Rc::new(RefCell::new(campos)),
        metodos: HashMap::new(),
    }
}

/// Extrai o horário atual decomposto (ano, mes, dia, hora, minuto, segundo).
fn agora_decomposto() -> (i64, i64, i64, i64, i64, i64) {
    let agora = chrono_or_default_date();
    // Formato: "aaaa-mm-ddTHH:MM:SS"
    let (data, hora_str) = agora.split_once('T').unwrap_or((agora.as_str(), "00:00:00"));
    let mut partes_data = data.split('-');
    let ano: i64 = partes_data.next().and_then(|s| s.parse().ok()).unwrap_or(1970);
    let mes: i64 = partes_data.next().and_then(|s| s.parse().ok()).unwrap_or(1);
    let dia: i64 = partes_data.next().and_then(|s| s.parse().ok()).unwrap_or(1);
    let mut partes_hora = hora_str.split(':');
    let hora: i64 = partes_hora.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let minuto: i64 = partes_hora.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let segundo: i64 = partes_hora.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    (ano, mes, dia, hora, minuto, segundo)
}

pub(crate) async fn despachar_nativo_assincrono(
    chave: &str,
    args: Vec<Valor>,
    este: Option<Valor>,
) -> Result<Valor, String> {
    match chave {
        // ============ Arquivo assíncrono ============
        "LerArquivoAssíncrono" | "LerArquivoAssincrono" | "Arquivo::LerTextoAssíncrono" | "Arquivo::LerTextoAssincrono" => {
            let caminho = args
                .into_iter()
                .next()
                .map(|v| texto_de(&v))
                .unwrap_or_default();
            let conteudo = tokio::fs::read_to_string(&caminho)
                .await
                .map_err(|e| format!("LerArquivoAssíncrono: {}", e))?;
            Ok(Valor::Texto(conteudo))
        }
        "EscreverArquivoAssíncrono" | "EscreverArquivoAssincrono" | "Arquivo::EscreverTextoAssíncrono" | "Arquivo::EscreverTextoAssincrono" => {
            let mut it = args.into_iter();
            let caminho = it.next().map(|v| texto_de(&v)).unwrap_or_default();
            let conteudo = it.next().map(|v| texto_de(&v)).unwrap_or_default();
            tokio::fs::write(&caminho, conteudo.as_bytes())
                .await
                .map_err(|e| format!("EscreverArquivoAssíncrono: {}", e))?;
            Ok(Valor::Nulo)
        }
        "VerificarArquivoAssíncrono" | "VerificarArquivoAssincrono" | "Arquivo::ExisteAssíncrono" | "Arquivo::ExisteAssincrono" => {
            let caminho = args
                .into_iter()
                .next()
                .map(|v| texto_de(&v))
                .unwrap_or_default();
            let existe = tokio::fs::try_exists(&caminho).await.unwrap_or(false);
            Ok(Valor::Booleano(existe))
        }
        "AdicionarTextoAssíncrono" | "AdicionarTextoAssincrono" | "Arquivo::AdicionarTextoAssíncrono" | "Arquivo::AdicionarTextoAssincrono" => {
            use tokio::io::AsyncWriteExt;
            let mut it = args.into_iter();
            let caminho = it.next().map(|v| texto_de(&v)).unwrap_or_default();
            let conteudo = it.next().map(|v| texto_de(&v)).unwrap_or_default();
            let mut file = tokio::fs::OpenOptions::new()
                .append(true)
                .create(true)
                .open(&caminho)
                .await
                .map_err(|e| format!("AdicionarTextoAssíncrono: {}", e))?;
            file.write_all(conteudo.as_bytes())
                .await
                .map_err(|e| format!("AdicionarTextoAssíncrono: {}", e))?;
            Ok(Valor::Nulo)
        }

        // ============ Tarefa ==========
        "Tarefa::Atraso" | "Temporizador::Aguardar" => {
            let ms = args
                .into_iter()
                .next()
                .map(|v| numero_de(&v) as u64)
                .unwrap_or(0);
            tokio::time::sleep(Duration::from_millis(ms)).await;
            Ok(Valor::Nulo)
        }
        "Tarefa::DeResultado" => {
            Ok(args.into_iter().next().unwrap_or(Valor::Nulo))
        }

        // ============ ClienteHttp assíncrono ============
        "ClienteHttp::ObterAssincrono" | "ClienteHttp::GetAsync" | "ClienteHttp::Get" => {
            let url = args
                .into_iter()
                .next()
                .map(|v| texto_de(&v))
                .unwrap_or_default();
            let url = resolver_url_com_base(este.as_ref().unwrap_or(&Valor::Nulo), &url);
            let corpo = http_request_async("GET", &url, "").await?;
            Ok(Valor::Texto(corpo))
        }
        "ClienteHttp::EnviarAssincrono" | "ClienteHttp::Enviar"
        | "ClienteHttp::PostAsync" | "ClienteHttp::Post" => {
            let mut it = args.into_iter();
            let url = it.next().map(|v| texto_de(&v)).unwrap_or_default();
            let corpo_req = it.next().map(|v| texto_de(&v)).unwrap_or_default();
            let url = resolver_url_com_base(este.as_ref().unwrap_or(&Valor::Nulo), &url);
            let corpo = http_request_async("POST", &url, &corpo_req).await?;
            Ok(Valor::Texto(corpo))
        }
        "ClienteHttp::AtualizarAssincrono" | "ClienteHttp::Atualizar"
        | "ClienteHttp::PutAsync" | "ClienteHttp::Put" => {
            let mut it = args.into_iter();
            let url = it.next().map(|v| texto_de(&v)).unwrap_or_default();
            let corpo_req = it.next().map(|v| texto_de(&v)).unwrap_or_default();
            let url = resolver_url_com_base(este.as_ref().unwrap_or(&Valor::Nulo), &url);
            let corpo = http_request_async("PUT", &url, &corpo_req).await?;
            Ok(Valor::Texto(corpo))
        }
        "ClienteHttp::ExcluirAssincrono" | "ClienteHttp::Excluir"
        | "ClienteHttp::DeleteAsync" | "ClienteHttp::Delete" => {
            let url = args
                .into_iter()
                .next()
                .map(|v| texto_de(&v))
                .unwrap_or_default();
            let url = resolver_url_com_base(este.as_ref().unwrap_or(&Valor::Nulo), &url);
            let corpo = http_request_async("DELETE", &url, "").await?;
            Ok(Valor::Texto(corpo))
        }

        // ============ HTTP genérico assíncrono ============
        "HttpGetAsync" | "Rede::HttpGetAsync" => {
            let url = args
                .into_iter()
                .next()
                .map(|v| texto_de(&v))
                .unwrap_or_default();
            let corpo = http_request_async("GET", &url, "").await?;
            Ok(Valor::Texto(corpo))
        }
        "HttpPostAsync" | "Rede::HttpPostAsync" => {
            let mut it = args.into_iter();
            let url = it.next().map(|v| texto_de(&v)).unwrap_or_default();
            let corpo_req = it.next().map(|v| texto_de(&v)).unwrap_or_default();
            let corpo = http_request_async("POST", &url, &corpo_req).await?;
            Ok(Valor::Texto(corpo))
        }

        // Fallback: tenta despachar como nativo síncrono (estático ou de instância)
        outro => {
            if let Some(e) = &este {
                despachar_nativo_instancia(outro, e.clone(), args)
            } else {
                despachar_nativo_estatico(outro, args)
            }
        }
    }
}

/// Gera uma semente u64 para o PRNG; se `semente` for 0, usa tempo+contador.
pub(crate) fn semente_aleatoria(semente: u64) -> u64 {
    if semente != 0 {
        return semente | 1;
    }
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    static CONTADOR: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x9E3779B97F4A7C15);
    let c = CONTADOR.fetch_add(0x9E3779B97F4A7C15, Ordering::Relaxed);
    (nanos ^ c).wrapping_mul(0xBF58476D1CE4E5B9) | 1
}

/// xorshift64* — próximo estado e valor u64.
fn proximo_aleatorio_u64(estado: &mut u64) -> u64 {
    let mut x = *estado;
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    *estado = x;
    x.wrapping_mul(0x2545F4914F6CDD1D)
}

/// Máximo de tarefas paralelas do PoolTarefas (global ao processo).
fn pool_maximo_paralelismo() -> &'static std::sync::atomic::AtomicUsize {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::OnceLock;
    static POOL: OnceLock<AtomicUsize> = OnceLock::new();
    POOL.get_or_init(|| AtomicUsize::new(num_cpus_ou_padrao()))
}

fn num_cpus_ou_padrao() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}

pub(crate) fn despachar_nativo_estatico(chave: &str, args: Vec<Valor>) -> Result<Valor, String> {
    match chave {
        // ============ Sistema.Console ============
        "Console::EscreverLinha" => {
            let msg = args
                .into_iter()
                .next()
                .map(|v| v.to_string())
                .unwrap_or_default();
            println!("{}", msg);
            Ok(Valor::Nulo)
        }
        "Console::Escrever" => {
            let msg = args
                .into_iter()
                .next()
                .map(|v| v.to_string())
                .unwrap_or_default();
            use std::io::Write;
            print!("{}", msg);
            let _ = std::io::stdout().flush();
            Ok(Valor::Nulo)
        }
        "Console::LerLinha" => {
            let mut entrada = String::new();
            std::io::stdin()
                .read_line(&mut entrada)
                .map_err(|e| format!("Erro ao ler entrada: {}", e))?;
            Ok(Valor::Texto(
                entrada.trim_end_matches(['\r', '\n']).to_string(),
            ))
        }

        // ============ Sistema.IO.Arquivo ============
        "Arquivo::LerTexto" => {
            let caminho = args
                .into_iter()
                .next()
                .map(|v| v.to_string())
                .unwrap_or_default();
            let conteudo =
                fs::read_to_string(&caminho).map_err(|e| format!("Arquivo::LerTexto: {}", e))?;
            Ok(Valor::Texto(conteudo))
        }
        "Arquivo::EscreverTexto" => {
            let mut it = args.into_iter();
            let caminho = it.next().map(|v| v.to_string()).unwrap_or_default();
            let conteudo = it.next().map(|v| v.to_string()).unwrap_or_default();
            fs::write(&caminho, &conteudo).map_err(|e| format!("Arquivo::EscreverTexto: {}", e))?;
            Ok(Valor::Nulo)
        }
        "Arquivo::AdicionarTexto" => {
            use std::io::Write;
            let mut it = args.into_iter();
            let caminho = it.next().map(|v| v.to_string()).unwrap_or_default();
            let conteudo = it.next().map(|v| v.to_string()).unwrap_or_default();
            let mut f = std::fs::OpenOptions::new()
                .append(true)
                .create(true)
                .open(&caminho)
                .map_err(|e| format!("Arquivo::AdicionarTexto: {}", e))?;
            f.write_all(conteudo.as_bytes())
                .map_err(|e| format!("Arquivo::AdicionarTexto: {}", e))?;
            Ok(Valor::Nulo)
        }
        "Arquivo::Existe" => {
            let caminho = args
                .into_iter()
                .next()
                .map(|v| v.to_string())
                .unwrap_or_default();
            Ok(Valor::Booleano(std::path::Path::new(&caminho).is_file()))
        }
        "Arquivo::Excluir" => {
            let caminho = args
                .into_iter()
                .next()
                .map(|v| v.to_string())
                .unwrap_or_default();
            fs::remove_file(&caminho).map_err(|e| format!("Arquivo::Excluir: {}", e))?;
            Ok(Valor::Nulo)
        }
        "Arquivo::Copiar" => {
            let mut it = args.into_iter();
            let origem = it.next().map(|v| v.to_string()).unwrap_or_default();
            let destino = it.next().map(|v| v.to_string()).unwrap_or_default();
            fs::copy(&origem, &destino).map_err(|e| format!("Arquivo::Copiar: {}", e))?;
            Ok(Valor::Nulo)
        }
        "Arquivo::Mover" => {
            let mut it = args.into_iter();
            let origem = it.next().map(|v| v.to_string()).unwrap_or_default();
            let destino = it.next().map(|v| v.to_string()).unwrap_or_default();
            fs::rename(&origem, &destino).map_err(|e| format!("Arquivo::Mover: {}", e))?;
            Ok(Valor::Nulo)
        }

        // ============ Sistema.IO.Diretorio ============
        "Diretorio::Existe" => {
            let caminho = args
                .into_iter()
                .next()
                .map(|v| v.to_string())
                .unwrap_or_default();
            Ok(Valor::Booleano(std::path::Path::new(&caminho).is_dir()))
        }
        "Diretorio::Criar" => {
            let caminho = args
                .into_iter()
                .next()
                .map(|v| v.to_string())
                .unwrap_or_default();
            fs::create_dir_all(&caminho).map_err(|e| format!("Diretorio::Criar: {}", e))?;
            Ok(Valor::Nulo)
        }
        "Diretorio::Excluir" => {
            let mut it = args.into_iter();
            let caminho = it.next().map(|v| v.to_string()).unwrap_or_default();
            let recursivo = matches!(it.next(), Some(Valor::Booleano(true)));
            if recursivo {
                fs::remove_dir_all(&caminho).map_err(|e| format!("Diretorio::Excluir: {}", e))?;
            } else {
                fs::remove_dir(&caminho).map_err(|e| format!("Diretorio::Excluir: {}", e))?;
            }
            Ok(Valor::Nulo)
        }
        "Diretorio::ObterAtual" => {
            let cur = std::env::current_dir()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            Ok(Valor::Texto(cur))
        }
        "Diretorio::DefinirAtual" => {
            let caminho = args
                .into_iter()
                .next()
                .map(|v| v.to_string())
                .unwrap_or_default();
            std::env::set_current_dir(&caminho)
                .map_err(|e| format!("Diretorio::DefinirAtual: {}", e))?;
            Ok(Valor::Nulo)
        }
        "Diretorio::ObterArquivos" | "Diretorio::ObterDiretorios" => {
            let caminho = args
                .into_iter()
                .next()
                .map(|v| v.to_string())
                .unwrap_or_default();
            let quer_arquivos = chave == "Diretorio::ObterArquivos";
            let entradas = fs::read_dir(&caminho)
                .map_err(|e| format!("{}: {}", chave, e))?;
            let mut nomes = Vec::new();
            for entrada in entradas.flatten() {
                let meta = match entrada.metadata() {
                    Ok(m) => m,
                    Err(_) => continue,
                };
                if quer_arquivos == meta.is_file() {
                    nomes.push(Valor::Texto(
                        entrada.file_name().to_string_lossy().into_owned(),
                    ));
                }
            }
            nomes.sort_by(|a, b| texto_de(a).cmp(&texto_de(b)));
            Ok(Valor::Array(nomes))
        }

        // ============ Sistema.IO.Arquivo — extras ============
        "Arquivo::Tamanho" => {
            let caminho = args
                .into_iter()
                .next()
                .map(|v| v.to_string())
                .unwrap_or_default();
            let meta = fs::metadata(&caminho).map_err(|e| format!("Arquivo::Tamanho: {}", e))?;
            Ok(Valor::Inteiro(meta.len() as i64))
        }
        "Arquivo::Criar" => {
            let caminho = args
                .into_iter()
                .next()
                .map(|v| v.to_string())
                .unwrap_or_default();
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&caminho)
                .map_err(|e| format!("Arquivo::Criar: {}", e))?;
            Ok(Valor::Nulo)
        }
        "Arquivo::LerLinhas" => {
            let caminho = args
                .into_iter()
                .next()
                .map(|v| v.to_string())
                .unwrap_or_default();
            let conteudo =
                fs::read_to_string(&caminho).map_err(|e| format!("Arquivo::LerLinhas: {}", e))?;
            let linhas = conteudo
                .lines()
                .map(|l| Valor::Texto(l.to_string()))
                .collect();
            Ok(Valor::Array(linhas))
        }

        // ============ Sistema.IO.Caminho ============
        "Caminho::Combinar" => {
            let mut it = args.into_iter();
            let a = it.next().map(|v| v.to_string()).unwrap_or_default();
            let b = it.next().map(|v| v.to_string()).unwrap_or_default();
            let sep = std::path::MAIN_SEPARATOR;
            let combinado = if a.ends_with(sep) || a.ends_with('/') || a.ends_with('\\') {
                format!("{}{}", a, b)
            } else {
                format!("{}{}{}", a, sep, b)
            };
            Ok(Valor::Texto(combinado))
        }
        "Caminho::NomeArquivo" => {
            let caminho = args
                .into_iter()
                .next()
                .map(|v| v.to_string())
                .unwrap_or_default();
            let nome = std::path::Path::new(&caminho)
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or(caminho);
            Ok(Valor::Texto(nome))
        }
        "Caminho::NomeSemExtensao" => {
            let caminho = args
                .into_iter()
                .next()
                .map(|v| v.to_string())
                .unwrap_or_default();
            let nome = std::path::Path::new(&caminho)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            Ok(Valor::Texto(nome))
        }
        "Caminho::DiretorioPai" => {
            let caminho = args
                .into_iter()
                .next()
                .map(|v| v.to_string())
                .unwrap_or_default();
            let pai = std::path::Path::new(&caminho)
                .parent()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            Ok(Valor::Texto(pai))
        }
        "Caminho::Extensao" => {
            let caminho = args
                .into_iter()
                .next()
                .map(|v| v.to_string())
                .unwrap_or_default();
            let ext = std::path::Path::new(&caminho)
                .extension()
                .map(|s| format!(".{}", s.to_string_lossy()))
                .unwrap_or_default();
            Ok(Valor::Texto(ext))
        }
        "Caminho::Absoluto" => {
            let caminho = args
                .into_iter()
                .next()
                .map(|v| v.to_string())
                .unwrap_or_default();
            let abs = if std::path::Path::new(&caminho).is_absolute() {
                caminho
            } else {
                std::env::current_dir()
                    .map(|d| {
                        d.join(&caminho).to_string_lossy().into_owned()
                    })
                    .unwrap_or(caminho)
            };
            Ok(Valor::Texto(abs))
        }
        "Caminho::Separadores" => Ok(Valor::Texto("/\\".to_string())),
        "Caminho::Separador" => Ok(Valor::Texto(
            std::path::MAIN_SEPARATOR.to_string(),
        )),

        // ============ Sistema.IO.Leitor — criação ============
        "Leitor::Abrir" => {
            let caminho = args
                .into_iter()
                .next()
                .map(|v| v.to_string())
                .unwrap_or_default();
            let conteudo =
                fs::read_to_string(&caminho).map_err(|e| format!("Leitor::Abrir: {}", e))?;
            let mut campos = HashMap::new();
            campos.insert("__restante".to_string(), Valor::Texto(conteudo));
            campos.insert("__fechado".to_string(), Valor::Booleano(false));
            Ok(Valor::Objeto {
                nome_classe: "Sistema.IO.Leitor".to_string(),
                campos: Rc::new(RefCell::new(campos)),
                metodos: HashMap::new(),
            })
        }

        // ============ Sistema.IO.InformacaoArquivo — criação ============
        "InformacaoArquivo::Abrir" => {
            let caminho = args
                .into_iter()
                .next()
                .map(|v| v.to_string())
                .unwrap_or_default();
            let caminho_abs = std::path::Path::new(&caminho);
            let meta = fs::metadata(&caminho).ok();
            let existe = meta.is_some();
            let nome = caminho_abs
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let diretorio = caminho_abs
                .parent()
                .map(|s| s.to_string_lossy().into_owned())
                .filter(|s| !s.is_empty())
                .or_else(|| {
                    std::env::current_dir()
                        .ok()
                        .map(|d| d.to_string_lossy().into_owned())
                })
                .unwrap_or_default();
            let tamanho = meta.as_ref().map(|m| m.len() as i64).unwrap_or(0);
            let modificacao = meta
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| data_hora_de_unix(d.as_secs()))
                .unwrap_or_default();
            let mut campos = HashMap::new();
            campos.insert("__caminho".to_string(), Valor::Texto(caminho));
            campos.insert("__existe".to_string(), Valor::Booleano(existe));
            campos.insert("__nome".to_string(), Valor::Texto(nome));
            campos.insert("__diretorio".to_string(), Valor::Texto(diretorio));
            campos.insert("__tamanho".to_string(), Valor::Inteiro(tamanho));
            campos.insert("__modificacao".to_string(), Valor::Texto(modificacao));
            Ok(Valor::Objeto {
                nome_classe: "Sistema.IO.InformacaoArquivo".to_string(),
                campos: Rc::new(RefCell::new(campos)),
                metodos: HashMap::new(),
            })
        }

        // ============ Sistema.Data.Data ============
        "Data::Agora" => {
            let (ano, mes, dia, hora, minuto, segundo) = agora_decomposto();
            Ok(objeto_data(ano, mes, dia, hora, minuto, segundo))
        }
        "Data::Hoje" => {
            let (ano, mes, dia, _, _, _) = agora_decomposto();
            Ok(objeto_data(ano, mes, dia, 0, 0, 0))
        }

        // ============ Sistema.Matematica ============
        "Matematica::Abs" => {
            let v = args.first().map(numero_de).unwrap_or(0.0);
            Ok(Valor::Duplo(v.abs()))
        }
        "Matematica::Max" => {
            let mut it = args.iter();
            let a = it.next().map(numero_de).unwrap_or(0.0);
            let b = it.next().map(numero_de).unwrap_or(0.0);
            Ok(Valor::Duplo(a.max(b)))
        }
        "Matematica::Min" => {
            let mut it = args.iter();
            let a = it.next().map(numero_de).unwrap_or(0.0);
            let b = it.next().map(numero_de).unwrap_or(0.0);
            Ok(Valor::Duplo(a.min(b)))
        }
        "Matematica::Potencia" => {
            let mut it = args.iter();
            let b = it.next().map(numero_de).unwrap_or(0.0);
            let e = it.next().map(numero_de).unwrap_or(0.0);
            Ok(Valor::Duplo(b.powf(e)))
        }
        "Matematica::RaizQuadrada" => {
            let v = args.first().map(numero_de).unwrap_or(0.0);
            if v < 0.0 {
                Ok(Valor::Nulo)
            } else {
                Ok(Valor::Duplo(v.sqrt()))
            }
        }
        "Matematica::Arredondar" => {
            let v = args.first().map(numero_de).unwrap_or(0.0);
            Ok(Valor::Duplo(v.round()))
        }
        "Matematica::Truncar" => {
            let v = args.first().map(numero_de).unwrap_or(0.0);
            Ok(Valor::Duplo(v.trunc()))
        }
        "Matematica::Piso" => {
            let v = args.first().map(numero_de).unwrap_or(0.0);
            Ok(Valor::Duplo(v.floor()))
        }
        "Matematica::Teto" => {
            let v = args.first().map(numero_de).unwrap_or(0.0);
            Ok(Valor::Duplo(v.ceil()))
        }
        "Matematica::Seno" => {
            let v = args.first().map(numero_de).unwrap_or(0.0);
            Ok(Valor::Duplo(v.sin()))
        }
        "Matematica::Cosseno" => {
            let v = args.first().map(numero_de).unwrap_or(0.0);
            Ok(Valor::Duplo(v.cos()))
        }
        "Matematica::Tangente" => {
            let v = args.first().map(numero_de).unwrap_or(0.0);
            Ok(Valor::Duplo(v.tan()))
        }
        "Matematica::LogaritmoNatural" => {
            let v = args.first().map(numero_de).unwrap_or(0.0);
            if v <= 0.0 {
                Ok(Valor::Nulo)
            } else {
                Ok(Valor::Duplo(v.ln()))
            }
        }
        "Matematica::LogaritmoDez" => {
            let v = args.first().map(numero_de).unwrap_or(0.0);
            if v <= 0.0 {
                Ok(Valor::Nulo)
            } else {
                Ok(Valor::Duplo(v.log10()))
            }
        }
        "Matematica::Expoente" => {
            let v = args.first().map(numero_de).unwrap_or(0.0);
            Ok(Valor::Duplo(v.exp()))
        }
        "Matematica::Pi" => Ok(Valor::Duplo(std::f64::consts::PI)),
        "Matematica::E" => Ok(Valor::Duplo(std::f64::consts::E)),

        // ============ Sistema.Threading.Tarefa ============
        "Tarefa::Atraso" | "Temporizador::Aguardar" => {
            let ms = args
                .first()
                .map(numero_de)
                .unwrap_or(0.0)
                .max(0.0) as u64;
            thread::sleep(Duration::from_millis(ms));
            Ok(Valor::Nulo)
        }
        "Tarefa::DeResultado" => {
            Ok(args.into_iter().next().unwrap_or(Valor::Nulo))
        }
        "PoolTarefas::ObterMaximoParalelismo" => {
            use std::sync::atomic::Ordering;
            Ok(Valor::Inteiro(
                pool_maximo_paralelismo().load(Ordering::Relaxed) as i64,
            ))
        }
        "PoolTarefas::DefinirMaximoParalelismo" => {
            use std::sync::atomic::Ordering;
            let maximo = args
                .first()
                .map(numero_de)
                .unwrap_or(0.0)
                .max(1.0) as usize;
            pool_maximo_paralelismo().store(maximo.max(1), Ordering::Relaxed);
            Ok(Valor::Nulo)
        }

        // ============ Sistema.Guid ============
        "Guid::Novo" | "Guid::Vazio" => {
            if chave == "Guid::Vazio" {
                return Ok(Valor::Texto(
                    "00000000-0000-0000-0000-000000000000".to_string(),
                ));
            }
            use std::sync::atomic::{AtomicU64, Ordering};
            static CONTADOR_GUID: AtomicU64 = AtomicU64::new(0);
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0);
            let c = CONTADOR_GUID.fetch_add(1, Ordering::Relaxed);
            let mut estado = semente_aleatoria(nanos ^ c.wrapping_mul(0x9E3779B97F4A7C15));
            let a = proximo_aleatorio_u64(&mut estado);
            let b = proximo_aleatorio_u64(&mut estado);
            let hex = format!("{:016x}{:016x}", a, b);
            let guid = format!(
                "{}-{}-{}-{}-{}",
                &hex[0..8],
                &hex[8..12],
                &hex[12..16],
                &hex[16..20],
                &hex[20..32]
            );
            Ok(Valor::Texto(guid))
        }
        "Guid::EhValido" => {
            let valor = args.first().map(texto_de).unwrap_or_default();
            let bytes = valor.as_bytes();
            let mut valido = bytes.len() == 36;
            if valido {
                for (i, &b) in bytes.iter().enumerate() {
                    let esperado_traco = i == 8 || i == 13 || i == 18 || i == 23;
                    if esperado_traco {
                        if b != b'-' {
                            valido = false;
                            break;
                        }
                    } else if !b.is_ascii_hexdigit() {
                        valido = false;
                        break;
                    }
                }
            }
            Ok(Valor::Booleano(valido))
        }

        // ============ Sistema.Texto.Json ============
        "Json::Serializar" => {
            let valor = args.into_iter().next().unwrap_or(Valor::Nulo);
            match serde_json::to_string(&valor_para_json(&valor)) {
                Ok(s) => Ok(Valor::Texto(s)),
                Err(e) => Err(format!("Json::Serializar: {}", e)),
            }
        }
        "Json::ValidarJson" => {
            let json = args
                .into_iter()
                .next()
                .map(|v| texto_de(&v))
                .unwrap_or_default();
            Ok(Valor::Booleano(
                serde_json::from_str::<serde_json::Value>(&json).is_ok(),
            ))
        }
        "Json::Formatar" => {
            let json = args
                .into_iter()
                .next()
                .map(|v| texto_de(&v))
                .unwrap_or_default();
            match serde_json::from_str::<serde_json::Value>(&json) {
                Ok(v) => Ok(Valor::Texto(
                    serde_json::to_string_pretty(&v).unwrap_or(json),
                )),
                Err(_) => Ok(Valor::Texto(json)),
            }
        }
        "Json::Desserializar" => {
            let json = args
                .into_iter()
                .next()
                .map(|v| texto_de(&v))
                .unwrap_or_default();
            match serde_json::from_str::<serde_json::Value>(&json) {
                Ok(v) => Ok(json_para_valor(&v)),
                Err(e) => Err(format!("Json::Desserializar: JSON inválido: {}", e)),
            }
        }
        "Json::SerializarFormatado" => {
            let valor = args.into_iter().next().unwrap_or(Valor::Nulo);
            match serde_json::to_string_pretty(&valor_para_json(&valor)) {
                Ok(s) => Ok(Valor::Texto(s)),
                Err(e) => Err(format!("Json::SerializarFormatado: {}", e)),
            }
        }
        "Json::TipoDe" => {
            let json = args
                .into_iter()
                .next()
                .map(|v| texto_de(&v))
                .unwrap_or_default();
            let tipo = match serde_json::from_str::<serde_json::Value>(&json) {
                Ok(serde_json::Value::Object(_)) => "objeto",
                Ok(serde_json::Value::Array(_)) => "array",
                Ok(serde_json::Value::String(_)) => "texto",
                Ok(serde_json::Value::Number(_)) => "numero",
                Ok(serde_json::Value::Bool(_)) => "booleano",
                Ok(serde_json::Value::Null) => "nulo",
                Err(_) => "invalido",
            };
            Ok(Valor::Texto(tipo.to_string()))
        }

        // ============ Sistema.Texto.Texto ============
        "Texto::Dividir" => {
            let texto = args.first().map(texto_de).unwrap_or_default();
            let sep = args.get(1).map(texto_de).unwrap_or_default();
            let partes: Vec<Valor> = if sep.is_empty() {
                texto
                    .chars()
                    .map(|c| Valor::Texto(c.to_string()))
                    .collect()
            } else {
                texto
                    .split(sep.as_str())
                    .map(|p| Valor::Texto(p.to_string()))
                    .collect()
            };
            Ok(Valor::Array(partes))
        }
        "Texto::DividirLinhas" => {
            let texto = args.first().map(texto_de).unwrap_or_default();
            let partes: Vec<Valor> = texto
                .lines()
                .map(|p| Valor::Texto(p.to_string()))
                .collect();
            Ok(Valor::Array(partes))
        }
        "Texto::Juntar" => {
            let sep = args.get(1).map(texto_de).unwrap_or_default();
            let texto = args
                .first()
                .map(itens_de_lista)
                .unwrap_or_default()
                .iter()
                .map(texto_de)
                .collect::<Vec<_>>()
                .join(&sep);
            Ok(Valor::Texto(texto))
        }
        "Texto::RemoverEspacos" => {
            let texto = args.first().map(texto_de).unwrap_or_default();
            Ok(Valor::Texto(texto.trim().to_string()))
        }
        "Texto::ParaMaiusculas" => {
            let texto = args.first().map(texto_de).unwrap_or_default();
            Ok(Valor::Texto(texto.to_uppercase()))
        }
        "Texto::ParaMinusculas" => {
            let texto = args.first().map(texto_de).unwrap_or_default();
            Ok(Valor::Texto(texto.to_lowercase()))
        }
        "Texto::Substituir" => {
            let texto = args.first().map(texto_de).unwrap_or_default();
            let antigo = args.get(1).map(texto_de).unwrap_or_default();
            let novo = args.get(2).map(texto_de).unwrap_or_default();
            if antigo.is_empty() {
                return Ok(Valor::Texto(texto));
            }
            Ok(Valor::Texto(texto.replace(&antigo, &novo)))
        }
        "Texto::ComecaCom" => {
            let texto = args.first().map(texto_de).unwrap_or_default();
            let prefixo = args.get(1).map(texto_de).unwrap_or_default();
            Ok(Valor::Booleano(texto.starts_with(&prefixo)))
        }
        "Texto::TerminaCom" => {
            let texto = args.first().map(texto_de).unwrap_or_default();
            let sufixo = args.get(1).map(texto_de).unwrap_or_default();
            Ok(Valor::Booleano(texto.ends_with(&sufixo)))
        }
        "Texto::Contem" => {
            let texto = args.first().map(texto_de).unwrap_or_default();
            let trecho = args.get(1).map(texto_de).unwrap_or_default();
            Ok(Valor::Booleano(texto.contains(&trecho)))
        }
        "Texto::VazioOuNulo" => {
            let vazio = match args.first() {
                None | Some(Valor::Nulo) => true,
                Some(Valor::Texto(s)) => s.is_empty(),
                _ => false,
            };
            Ok(Valor::Booleano(vazio))
        }
        "Texto::Tamanho" => {
            let texto = args.first().map(texto_de).unwrap_or_default();
            Ok(Valor::Inteiro(texto.chars().count() as i64))
        }

        // ============ Sistema.Texto.Codificacao ============
        "Codificacao::ParaBytes" => {
            let texto = args.first().map(texto_de).unwrap_or_default();
            let bytes: Vec<Valor> = texto
                .as_bytes()
                .iter()
                .map(|b| Valor::Inteiro(*b as i64))
                .collect();
            Ok(Valor::Array(bytes))
        }
        "Codificacao::ParaTexto" => {
            let bytes: Vec<u8> = args
                .first()
                .map(itens_de_lista)
                .unwrap_or_default()
                .iter()
                .map(|v| match numero_de(v) {
                    n if n < 0.0 => 0u8,
                    n if n > 255.0 => 255u8,
                    n => n as u8,
                })
                .collect();
            Ok(Valor::Texto(String::from_utf8_lossy(&bytes).into_owned()))
        }
        "Codificacao::TamanhoBytes" => {
            let texto = args.first().map(texto_de).unwrap_or_default();
            Ok(Valor::Inteiro(texto.len() as i64))
        }
        "Codificacao::Base64Codificar" => {
            let texto = args.first().map(texto_de).unwrap_or_default();
            Ok(Valor::Texto(base64_codificar(texto.as_bytes())))
        }
        "Codificacao::Base64Decodificar" => {
            let texto = args.first().map(texto_de).unwrap_or_default();
            let bytes = base64_decodificar(&texto).unwrap_or_default();
            Ok(Valor::Texto(String::from_utf8_lossy(&bytes).into_owned()))
        }
        "Codificacao::ParaHex" => {
            let bytes: Vec<u8> = args
                .first()
                .map(itens_de_lista)
                .unwrap_or_default()
                .iter()
                .map(|v| match numero_de(v) {
                    n if n < 0.0 => 0u8,
                    n if n > 255.0 => 255u8,
                    n => n as u8,
                })
                .collect();
            Ok(Valor::Texto(hex_codificar(&bytes)))
        }
        "Codificacao::DeHex" => {
            let texto = args.first().map(texto_de).unwrap_or_default();
            let bytes = hex_decodificar(&texto).unwrap_or_default();
            let itens: Vec<Valor> = bytes.iter().map(|b| Valor::Inteiro(*b as i64)).collect();
            Ok(Valor::Array(itens))
        }

        // ============ Sistema.Texto.Regex (estáticos) ============
        "Regex::Escape" => {
            let texto = args.first().map(texto_de).unwrap_or_default();
            Ok(Valor::Texto(regex::escape(&texto)))
        }
        "Regex::EhValido" => {
            let padrao = args.first().map(texto_de).unwrap_or_default();
            Ok(Valor::Booleano(regex::Regex::new(&padrao).is_ok()))
        }

        // ============ Sistema.Texto.Texto — utilitários adicionais ============
        "Texto::Subtexto" => {
            let texto = args.first().map(texto_de).unwrap_or_default();
            let inicio = args.get(1).map(numero_de).unwrap_or(0.0) as i64;
            let comprimento = args.get(2).map(numero_de).unwrap_or(-1.0) as i64;
            let chars: Vec<char> = texto.chars().collect();
            let len = chars.len() as i64;
            let ini = inicio.clamp(0, len);
            let fim = if comprimento < 0 {
                len
            } else {
                (ini + comprimento).min(len)
            };
            let resultado: String = chars[ini as usize..fim as usize].iter().collect();
            Ok(Valor::Texto(resultado))
        }
        "Texto::IndiceDe" => {
            let texto = args.first().map(texto_de).unwrap_or_default();
            let trecho = args.get(1).map(texto_de).unwrap_or_default();
            let indice = texto
                .find(&trecho)
                .map(|b| texto[..b].chars().count() as i64)
                .unwrap_or(-1);
            Ok(Valor::Inteiro(indice))
        }
        "Texto::UltimoIndiceDe" => {
            let texto = args.first().map(texto_de).unwrap_or_default();
            let trecho = args.get(1).map(texto_de).unwrap_or_default();
            let indice = texto
                .rfind(&trecho)
                .map(|b| texto[..b].chars().count() as i64)
                .unwrap_or(-1);
            Ok(Valor::Inteiro(indice))
        }
        "Texto::Inverter" => {
            let texto = args.first().map(texto_de).unwrap_or_default();
            Ok(Valor::Texto(texto.chars().rev().collect::<String>()))
        }
        "Texto::PadEsquerda" | "Texto::PadDireita" => {
            let texto = args.first().map(texto_de).unwrap_or_default();
            let largura = args.get(1).map(numero_de).unwrap_or(0.0) as usize;
            let preenchimento = args
                .get(2)
                .map(texto_de)
                .and_then(|p| p.chars().next())
                .unwrap_or(' ');
            let atual = texto.chars().count();
            if atual >= largura {
                return Ok(Valor::Texto(texto));
            }
            let faltam = largura - atual;
            let preenchido: String = std::iter::repeat(preenchimento)
                .take(faltam)
                .collect();
            let resultado = if chave == "Texto::PadEsquerda" {
                format!("{}{}", preenchido, texto)
            } else {
                format!("{}{}", texto, preenchido)
            };
            Ok(Valor::Texto(resultado))
        }
        "Texto::ContarOcorrencias" => {
            let texto = args.first().map(texto_de).unwrap_or_default();
            let trecho = args.get(1).map(texto_de).unwrap_or_default();
            let total = if trecho.is_empty() {
                0
            } else {
                texto.match_indices(&trecho).count() as i64
            };
            Ok(Valor::Inteiro(total))
        }
        "Texto::EstaVazia" => {
            let texto = args.first().map(texto_de).unwrap_or_default();
            Ok(Valor::Booleano(texto.is_empty()))
        }
        "Texto::Repete" => {
            let texto = args.first().map(texto_de).unwrap_or_default();
            let vezes = args.get(1).map(numero_de).unwrap_or(0.0);
            let n = if vezes < 0.0 { 0 } else { vezes as usize };
            Ok(Valor::Texto(texto.repeat(n)))
        }
        "Texto::Comparar" => {
            let a = args.first().map(texto_de).unwrap_or_default();
            let b = args.get(1).map(texto_de).unwrap_or_default();
            let ordem = match a.cmp(&b) {
                std::cmp::Ordering::Less => -1,
                std::cmp::Ordering::Equal => 0,
                std::cmp::Ordering::Greater => 1,
            };
            Ok(Valor::Inteiro(ordem))
        }
        "Texto::IgualIgnoreCase" => {
            let a = args.first().map(texto_de).unwrap_or_default();
            let b = args.get(1).map(texto_de).unwrap_or_default();
            Ok(Valor::Booleano(a.to_lowercase() == b.to_lowercase()))
        }
        "Texto::Remover" => {
            let texto = args.first().map(texto_de).unwrap_or_default();
            let inicio = args.get(1).map(numero_de).unwrap_or(0.0) as i64;
            let comprimento = args.get(2).map(numero_de).unwrap_or(-1.0) as i64;
            let chars: Vec<char> = texto.chars().collect();
            let len = chars.len() as i64;
            let ini = inicio.clamp(0, len);
            let fim = if comprimento < 0 {
                len
            } else {
                (ini + comprimento).min(len)
            };
            let mut resultado = String::new();
            resultado.extend(chars[..ini as usize].iter());
            resultado.extend(chars[fim as usize..].iter());
            Ok(Valor::Texto(resultado))
        }
        "Texto::Inserir" => {
            let texto = args.first().map(texto_de).unwrap_or_default();
            let inicio = args.get(1).map(numero_de).unwrap_or(0.0) as i64;
            let trecho = args.get(2).map(texto_de).unwrap_or_default();
            let chars: Vec<char> = texto.chars().collect();
            let ini = inicio.clamp(0, chars.len() as i64) as usize;
            let mut resultado: String = chars[..ini].iter().collect();
            resultado.push_str(&trecho);
            resultado.extend(chars[ini..].iter());
            Ok(Valor::Texto(resultado))
        }
        "Texto::NormalizarEspacos" => {
            let texto = args.first().map(texto_de).unwrap_or_default();
            let normalizado = texto
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            Ok(Valor::Texto(normalizado))
        }
        "Texto::DividirPalavras" | "Texto::ContarPalavras" => {
            let texto = args.first().map(texto_de).unwrap_or_default();
            let palavras: Vec<&str> = texto.split_whitespace().collect();
            if chave == "Texto::ContarPalavras" {
                Ok(Valor::Inteiro(palavras.len() as i64))
            } else {
                let partes: Vec<Valor> =
                    palavras.into_iter().map(|p| Valor::Texto(p.to_string())).collect();
                Ok(Valor::Array(partes))
            }
        }
        "Texto::LocalizarTodas" => {
            let texto = args.first().map(texto_de).unwrap_or_default();
            let trecho = args.get(1).map(texto_de).unwrap_or_default();
            let mut ocorrencias: Vec<Valor> = Vec::new();
            if !trecho.is_empty() {
                let mut inicio = 0usize;
                while let Some(pos) = texto[inicio..].find(&trecho) {
                    let abs = inicio + pos;
                    ocorrencias.push(Valor::Texto(trecho.clone()));
                    inicio = abs + trecho.len();
                }
            }
            Ok(Valor::Array(ocorrencias))
        }

        // ============ Sistema.Ambiente ============
        "Ambiente::Argumentos" => {
            let args_prog: Vec<Valor> = env::args().skip(1).map(Valor::Texto).collect();
            Ok(Valor::Array(args_prog))
        }
        "Ambiente::VariavelAmbiente" => {
            let nome = args.first().map(texto_de).unwrap_or_default();
            Ok(Valor::Texto(env::var(&nome).unwrap_or_default()))
        }
        "Ambiente::DefinirVariavelAmbiente" => {
            let nome = args.first().map(texto_de).unwrap_or_default();
            let valor = args.get(1).map(texto_de).unwrap_or_default();
            env::set_var(&nome, &valor);
            Ok(Valor::Nulo)
        }
        "Ambiente::NomeComputador" => {
            let nome = env::var("COMPUTERNAME")
                .or_else(|_| env::var("HOSTNAME"))
                .unwrap_or_else(|_| "desconhecido".to_string());
            Ok(Valor::Texto(nome))
        }

        // ============ Sistema.Conversao ============
        "Conversao::ParaInteiro" => {
            let texto = args.first().map(texto_de).unwrap_or_default();
            let t = texto.trim();
            let n = t
                .parse::<i64>()
                .ok()
                .or_else(|| t.parse::<f64>().ok().map(|f| f as i64))
                .unwrap_or(0);
            Ok(Valor::Inteiro(n))
        }
        "Conversao::ParaFlutuante" => {
            let texto = args.first().map(texto_de).unwrap_or_default();
            let n = texto.trim().parse::<f32>().unwrap_or(0.0);
            Ok(Valor::Flutuante(n))
        }
        "Conversao::ParaDuplo" => {
            let texto = args.first().map(texto_de).unwrap_or_default();
            let n = texto.trim().parse::<f64>().unwrap_or(0.0);
            Ok(Valor::Duplo(n))
        }
        "Conversao::ParaDecimal" => {
            let texto = args.first().map(texto_de).unwrap_or_default();
            let d = texto
                .trim()
                .parse::<Decimal>()
                .unwrap_or_else(|_| Decimal::ZERO);
            Ok(Valor::Decimal(d))
        }
        "Conversao::ParaBooleano" => {
            let texto = args.first().map(texto_de).unwrap_or_default();
            let t = texto.trim().to_lowercase();
            let b = match t.as_str() {
                "verdadeiro" | "true" | "1" | "sim" | "yes" => true,
                "falso" | "false" | "0" | "nao" | "não" | "no" => false,
                _ => t.parse::<f64>().map(|n| n != 0.0).unwrap_or(false),
            };
            Ok(Valor::Booleano(b))
        }
        "Conversao::EhNumero" => {
            let texto = args.first().map(texto_de).unwrap_or_default();
            Ok(Valor::Booleano(texto.trim().parse::<f64>().is_ok()))
        }

        // ============ Sistema.Rede.Dns ============
        "Dns::Resolver" => {
            let host = args.first().map(texto_de).unwrap_or_default();
            let ip = (host.as_str(), 0u16)
                .to_socket_addrs()
                .ok()
                .and_then(|mut it| it.next())
                .map(|a| a.ip().to_string())
                .unwrap_or_default();
            Ok(Valor::Texto(ip))
        }
        "Dns::ResolverTodos" => {
            let host = args.first().map(texto_de).unwrap_or_default();
            let ips: Vec<Valor> = (host.as_str(), 0u16)
                .to_socket_addrs()
                .map(|it| {
                    it.map(|a| Valor::Texto(a.ip().to_string()))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            Ok(Valor::Array(ips))
        }

        // ============ Sistema.Rede.SocketTcp / ServidorTcp (estáticos) ============
        "SocketTcp::Conectar" => {
            let host = args.first().map(texto_de).unwrap_or_default();
            let porta = args.get(1).map(numero_de).unwrap_or(0.0) as u16;
            let stream = TcpStream::connect((host.as_str(), porta))
                .map_err(|e| format!("SocketTcp::Conectar: {}", e))?;
            let _ = stream.set_nodelay(true);
            let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
            let id = registrar_socket(stream);
            Ok(objeto_socket_tcp(id))
        }
        "ServidorTcp::Ouvir" => {
            let porta = args.first().map(numero_de).unwrap_or(0.0) as u16;
            let listener = TcpListener::bind(("0.0.0.0", porta))
                .map_err(|e| format!("ServidorTcp::Ouvir: {}", e))?;
            let id = registrar_servidor(listener);
            Ok(objeto_servidor_tcp(id))
        }

        // ============ Sistema.Data.Horario — fábricas estáticas ============
        "Horario::DeSegundos" | "Horario::DeMinutos" | "Horario::DeHoras"
        | "Horario::DeDias" => {
            let n = args.first().map(numero_de).unwrap_or(0.0) as i64;
            let fator = match chave {
                "Horario::DeMinutos" => 60,
                "Horario::DeHoras" => 3600,
                "Horario::DeDias" => 86400,
                _ => 1,
            };
            Ok(objeto_horario(n.saturating_mul(fator)))
        }

        // ============ Fallback ============
        _ => {
            eprintln!(
                "[aviso nativo] Função nativa estática '{}' não implementada; retornando nulo",
                chave
            );
            Ok(Valor::Nulo)
        }
    }
}

/// Registro de métodos nativos de instância.
pub(crate) fn despachar_nativo_instancia(
    chave: &str,
    este: Valor,
    args: Vec<Valor>,
) -> Result<Valor, String> {
    match chave {
        // ============ Sistema.Data.Data — instância ============
        "Data::Formatar" => {
            let formato = args
                .into_iter()
                .next()
                .map(|v| v.to_string())
                .unwrap_or_default();
            if let Valor::Objeto { campos, .. } = &este {
                let c = campos.borrow();
                let dia = c
                    .get("Dia")
                    .and_then(|v| {
                        if let Valor::Inteiro(n) = v {
                            Some(*n)
                        } else {
                            None
                        }
                    })
                    .unwrap_or(1);
                let mes = c
                    .get("Mes")
                    .and_then(|v| {
                        if let Valor::Inteiro(n) = v {
                            Some(*n)
                        } else {
                            None
                        }
                    })
                    .unwrap_or(1);
                let ano = c
                    .get("Ano")
                    .and_then(|v| {
                        if let Valor::Inteiro(n) = v {
                            Some(*n)
                        } else {
                            None
                        }
                    })
                    .unwrap_or(2000);
                let hora = c
                    .get("Hora")
                    .and_then(|v| {
                        if let Valor::Inteiro(n) = v {
                            Some(*n)
                        } else {
                            None
                        }
                    })
                    .unwrap_or(0);
                let min = c
                    .get("Minuto")
                    .and_then(|v| {
                        if let Valor::Inteiro(n) = v {
                            Some(*n)
                        } else {
                            None
                        }
                    })
                    .unwrap_or(0);
                let seg = c
                    .get("Segundo")
                    .and_then(|v| {
                        if let Valor::Inteiro(n) = v {
                            Some(*n)
                        } else {
                            None
                        }
                    })
                    .unwrap_or(0);
                let resultado = formato
                    .replace("dd", &format!("{:02}", dia))
                    .replace("MM", &format!("{:02}", mes))
                    .replace("aaaa", &format!("{:04}", ano))
                    .replace("aa", &format!("{:02}", ano % 100))
                    .replace("HH", &format!("{:02}", hora))
                    .replace("mm", &format!("{:02}", min))
                    .replace("ss", &format!("{:02}", seg));
                Ok(Valor::Texto(resultado))
            } else {
                Ok(Valor::Texto(String::new()))
            }
        }

        // ============ Sistema.Colecoes.Dicionario ============
        "Dicionario::Adicionar" => {
            let mut it = args.into_iter();
            let chave = it.next().unwrap_or(Valor::Nulo);
            let valor = it.next().unwrap_or(Valor::Nulo);
            if let Some(campos) = campos_de(&este) {
                let mut itens = ler_array_interno(&campos, "__itens__");
                dicionario_adicionar(&mut itens, chave, valor);
                escrever_array_interno(&campos, "__itens__", itens);
            }
            Ok(Valor::Nulo)
        }
        "Dicionario::Obter" => {
            let chave = args.into_iter().next().unwrap_or(Valor::Nulo);
            if let Some(campos) = campos_de(&este) {
                let itens = ler_array_interno(&campos, "__itens__");
                return Ok(dicionario_obter(&itens, &chave));
            }
            Ok(Valor::Nulo)
        }
        "Dicionario::TentarObter" => {
            let chave = args.into_iter().next().unwrap_or(Valor::Nulo);
            if let Some(campos) = campos_de(&este) {
                let itens = ler_array_interno(&campos, "__itens__");
                if let Some(i) = dicionario_encontrar(&itens, &chave) {
                    if let Valor::Array(par) = &itens[i] {
                        if par.len() >= 2 {
                            return Ok(par[1].clone());
                        }
                    }
                }
            }
            Ok(Valor::Nulo)
        }
        "Dicionario::ContemChave" => {
            let chave = args.into_iter().next().unwrap_or(Valor::Nulo);
            if let Some(campos) = campos_de(&este) {
                let itens = ler_array_interno(&campos, "__itens__");
                return Ok(Valor::Booleano(dicionario_encontrar(&itens, &chave).is_some()));
            }
            Ok(Valor::Booleano(false))
        }
        "Dicionario::Remover" => {
            let chave = args.into_iter().next().unwrap_or(Valor::Nulo);
            if let Some(campos) = campos_de(&este) {
                let mut itens = ler_array_interno(&campos, "__itens__");
                if let Some(i) = dicionario_encontrar(&itens, &chave) {
                    itens.remove(i);
                    escrever_array_interno(&campos, "__itens__", itens);
                    return Ok(Valor::Booleano(true));
                }
            }
            Ok(Valor::Booleano(false))
        }
        "Dicionario::Limpar" => {
            if let Some(campos) = campos_de(&este) {
                escrever_array_interno(&campos, "__itens__", Vec::new());
            }
            Ok(Valor::Nulo)
        }
        "Dicionario::ContemValor" => {
            let valor = args.into_iter().next().unwrap_or(Valor::Nulo);
            if let Some(campos) = campos_de(&este) {
                let itens = ler_array_interno(&campos, "__itens__");
                let contem = itens.iter().any(|par| match par {
                    Valor::Array(v) if v.len() >= 2 => v[1] == valor,
                    _ => false,
                });
                return Ok(Valor::Booleano(contem));
            }
            Ok(Valor::Booleano(false))
        }
        "Dicionario::Chaves" | "Dicionario::Valores" => {
            let extrair_valor = chave == "Dicionario::Valores";
            if let Some(campos) = campos_de(&este) {
                let itens = ler_array_interno(&campos, "__itens__");
                let valores: Vec<Valor> = itens
                    .iter()
                    .filter_map(|par| match par {
                        Valor::Array(v) if v.len() >= 2 => {
                            Some(if extrair_valor { v[1].clone() } else { v[0].clone() })
                        }
                        _ => None,
                    })
                    .collect();
                // Retorna como instância de Lista para compatibilidade com a API
                let mut campos_lista = HashMap::new();
                campos_lista.insert("__itens__".to_string(), Valor::Array(valores));
                return Ok(Valor::Objeto {
                    nome_classe: "Sistema.Colecoes.Lista".to_string(),
                    campos: Rc::new(RefCell::new(campos_lista)),
                    metodos: HashMap::new(),
                });
            }
            let mut campos_lista = HashMap::new();
            campos_lista.insert("__itens__".to_string(), Valor::Array(Vec::new()));
            Ok(Valor::Objeto {
                nome_classe: "Sistema.Colecoes.Lista".to_string(),
                campos: Rc::new(RefCell::new(campos_lista)),
                metodos: HashMap::new(),
            })
        }

        // ============ Sistema.Texto.ConstrutorTexto ============
        "ConstrutorTexto::Anexar" => {
            let valor = args.into_iter().next().map(|v| texto_de(&v)).unwrap_or_default();
            if let Some(campos) = campos_de(&este) {
                let mut atual = match campos.borrow().get("__texto__") {
                    Some(Valor::Texto(s)) => s.clone(),
                    _ => String::new(),
                };
                atual.push_str(&valor);
                campos
                    .borrow_mut()
                    .insert("__texto__".to_string(), Valor::Texto(atual));
            }
            Ok(Valor::Nulo)
        }
        "ConstrutorTexto::AnexarLinha" => {
            let valor = args.into_iter().next().map(|v| texto_de(&v)).unwrap_or_default();
            if let Some(campos) = campos_de(&este) {
                let mut atual = match campos.borrow().get("__texto__") {
                    Some(Valor::Texto(s)) => s.clone(),
                    _ => String::new(),
                };
                atual.push_str(&valor);
                atual.push_str("\n");
                campos
                    .borrow_mut()
                    .insert("__texto__".to_string(), Valor::Texto(atual));
            }
            Ok(Valor::Nulo)
        }
        "ConstrutorTexto::AnexarValor" => {
            let valor = args.into_iter().next().map(|v| texto_de(&v)).unwrap_or_default();
            if let Some(campos) = campos_de(&este) {
                let mut atual = match campos.borrow().get("__texto__") {
                    Some(Valor::Texto(s)) => s.clone(),
                    _ => String::new(),
                };
                atual.push_str(&valor);
                campos
                    .borrow_mut()
                    .insert("__texto__".to_string(), Valor::Texto(atual));
            }
            Ok(Valor::Nulo)
        }
        "ConstrutorTexto::AnexarNovaLinha" => {
            if let Some(campos) = campos_de(&este) {
                let mut atual = match campos.borrow().get("__texto__") {
                    Some(Valor::Texto(s)) => s.clone(),
                    _ => String::new(),
                };
                atual.push_str("\n");
                campos
                    .borrow_mut()
                    .insert("__texto__".to_string(), Valor::Texto(atual));
            }
            Ok(Valor::Nulo)
        }
        "ConstrutorTexto::Inserir" => {
            let indice = args.first().map(numero_de).unwrap_or(0.0) as i64;
            let valor = args.get(1).map(|v| texto_de(v)).unwrap_or_default();
            if let Some(campos) = campos_de(&este) {
                let atual = match campos.borrow().get("__texto__") {
                    Some(Valor::Texto(s)) => s.clone(),
                    _ => String::new(),
                };
                let chars: Vec<char> = atual.chars().collect();
                let ini = indice.clamp(0, chars.len() as i64) as usize;
                let mut novo: String = chars[..ini].iter().collect();
                novo.push_str(&valor);
                novo.extend(chars[ini..].iter());
                campos
                    .borrow_mut()
                    .insert("__texto__".to_string(), Valor::Texto(novo));
            }
            Ok(Valor::Nulo)
        }
        "ConstrutorTexto::Remover" => {
            let inicio = args.first().map(numero_de).unwrap_or(0.0) as i64;
            let comprimento = args.get(1).map(numero_de).unwrap_or(-1.0) as i64;
            if let Some(campos) = campos_de(&este) {
                let atual = match campos.borrow().get("__texto__") {
                    Some(Valor::Texto(s)) => s.clone(),
                    _ => String::new(),
                };
                let chars: Vec<char> = atual.chars().collect();
                let len = chars.len() as i64;
                let ini = inicio.clamp(0, len);
                let fim = if comprimento < 0 {
                    len
                } else {
                    (ini + comprimento).min(len)
                };
                let mut novo: String = chars[..ini as usize].iter().collect();
                novo.extend(chars[fim as usize..].iter());
                campos
                    .borrow_mut()
                    .insert("__texto__".to_string(), Valor::Texto(novo));
            }
            Ok(Valor::Nulo)
        }
        "ConstrutorTexto::IndiceDe" => {
            let alvo = args.first().map(|v| texto_de(v)).unwrap_or_default();
            let indice = if let Some(campos) = campos_de(&este) {
                match campos.borrow().get("__texto__") {
                    Some(Valor::Texto(s)) => s
                        .find(&alvo)
                        .map(|b| s[..b].chars().count() as i64)
                        .unwrap_or(-1),
                    _ => -1,
                }
            } else {
                -1
            };
            Ok(Valor::Inteiro(indice))
        }
        "ConstrutorTexto::Subtexto" => {
            let inicio = args.first().map(numero_de).unwrap_or(0.0) as i64;
            let comprimento = args.get(1).map(numero_de).unwrap_or(-1.0) as i64;
            let resultado = if let Some(campos) = campos_de(&este) {
                match campos.borrow().get("__texto__") {
                    Some(Valor::Texto(s)) => {
                        let chars: Vec<char> = s.chars().collect();
                        let len = chars.len() as i64;
                        let ini = inicio.clamp(0, len);
                        let fim = if comprimento < 0 {
                            len
                        } else {
                            (ini + comprimento).min(len)
                        };
                        chars[ini as usize..fim as usize].iter().collect::<String>()
                    }
                    _ => String::new(),
                }
            } else {
                String::new()
            };
            Ok(Valor::Texto(resultado))
        }
        "ConstrutorTexto::ParaTexto" => {
            if let Some(campos) = campos_de(&este) {
                if let Some(Valor::Texto(s)) = campos.borrow().get("__texto__") {
                    return Ok(Valor::Texto(s.clone()));
                }
            }
            Ok(Valor::Texto(String::new()))
        }
        "ConstrutorTexto::Limpar" => {
            if let Some(campos) = campos_de(&este) {
                campos
                    .borrow_mut()
                    .insert("__texto__".to_string(), Valor::Texto(String::new()));
            }
            Ok(Valor::Nulo)
        }
        "ConstrutorTexto::Tamanho" | "ConstrutorTexto::Comprimento" => {
            if let Some(campos) = campos_de(&este) {
                if let Some(Valor::Texto(s)) = campos.borrow().get("__texto__") {
                    return Ok(Valor::Inteiro(s.chars().count() as i64));
                }
            }
            Ok(Valor::Inteiro(0))
        }

        // ============ Sistema.Data.Data — métodos com corpo em .pr ============
        "Data::ParaTexto" => {
            if let Valor::Objeto { campos, .. } = &este {
                let c = campos.borrow();
                let get = |k: &str, pad: usize| -> String {
                    let v = c
                        .get(k)
                        .and_then(|v| {
                            if let Valor::Inteiro(n) = v {
                                Some(*n)
                            } else {
                                None
                            }
                        })
                        .unwrap_or(0);
                    format!("{:0width$}", v, width = pad)
                };
                let ano = c
                    .get("Ano")
                    .and_then(|v| {
                        if let Valor::Inteiro(n) = v {
                            Some(*n)
                        } else {
                            None
                        }
                    })
                    .unwrap_or(0);
                return Ok(Valor::Texto(format!(
                    "{:02}/{:02}/{:04} {}:{}:{}",
                    get("Dia", 2),
                    get("Mes", 2),
                    ano,
                    get("Hora", 2),
                    get("Minuto", 2),
                    get("Segundo", 2)
                )));
            }
            Ok(Valor::Texto(String::new()))
        }
        "Data::Igual" => {
            let outro = args.into_iter().next().unwrap_or(Valor::Nulo);
            if let (Valor::Objeto { campos: a, .. }, Valor::Objeto { campos: b, .. }) =
                (&este, &outro)
            {
                let ca = a.borrow();
                let cb = b.borrow();
                let campos_iguais = ["Ano", "Mes", "Dia", "Hora", "Minuto", "Segundo"]
                    .iter()
                    .all(|k| ca.get(*k) == cb.get(*k));
                return Ok(Valor::Booleano(campos_iguais));
            }
            Ok(Valor::Booleano(false))
        }

        // ============ Sistema.Colecoes.Fila (FIFO) ============
        "Fila::Enfileirar" => {
            let item = args.into_iter().next().unwrap_or(Valor::Nulo);
            if let Some(campos) = campos_de(&este) {
                let mut itens = ler_array_interno(&campos, "__itens__");
                itens.push(item);
                escrever_array_interno(&campos, "__itens__", itens);
            }
            Ok(Valor::Nulo)
        }
        "Fila::Desenfileirar" => {
            if let Some(campos) = campos_de(&este) {
                let mut itens = ler_array_interno(&campos, "__itens__");
                if itens.is_empty() {
                    return Ok(Valor::Nulo);
                }
                let item = itens.remove(0);
                escrever_array_interno(&campos, "__itens__", itens);
                return Ok(item);
            }
            Ok(Valor::Nulo)
        }
        "Fila::Espiar" => {
            if let Some(campos) = campos_de(&este) {
                let itens = ler_array_interno(&campos, "__itens__");
                return Ok(itens.first().cloned().unwrap_or(Valor::Nulo));
            }
            Ok(Valor::Nulo)
        }
        "Fila::EstaVazia" => Ok(Valor::Booleano(
            campos_de(&este)
                .map(|c| ler_array_interno(&c, "__itens__").is_empty())
                .unwrap_or(true),
        )),
        "Fila::Limpar" => {
            if let Some(campos) = campos_de(&este) {
                escrever_array_interno(&campos, "__itens__", Vec::new());
            }
            Ok(Valor::Nulo)
        }

        // ============ Sistema.Colecoes.Pilha (LIFO) ============
        "Pilha::Empilhar" => {
            let item = args.into_iter().next().unwrap_or(Valor::Nulo);
            if let Some(campos) = campos_de(&este) {
                let mut itens = ler_array_interno(&campos, "__itens__");
                itens.push(item);
                escrever_array_interno(&campos, "__itens__", itens);
            }
            Ok(Valor::Nulo)
        }
        "Pilha::Desempilhar" => {
            if let Some(campos) = campos_de(&este) {
                let mut itens = ler_array_interno(&campos, "__itens__");
                if itens.is_empty() {
                    return Ok(Valor::Nulo);
                }
                let item = itens.pop().unwrap_or(Valor::Nulo);
                escrever_array_interno(&campos, "__itens__", itens);
                return Ok(item);
            }
            Ok(Valor::Nulo)
        }
        "Pilha::EspiarTopo" => {
            if let Some(campos) = campos_de(&este) {
                let itens = ler_array_interno(&campos, "__itens__");
                return Ok(itens.last().cloned().unwrap_or(Valor::Nulo));
            }
            Ok(Valor::Nulo)
        }
        "Pilha::EstaVazia" => Ok(Valor::Booleano(
            campos_de(&este)
                .map(|c| ler_array_interno(&c, "__itens__").is_empty())
                .unwrap_or(true),
        )),
        "Pilha::Limpar" => {
            if let Some(campos) = campos_de(&este) {
                escrever_array_interno(&campos, "__itens__", Vec::new());
            }
            Ok(Valor::Nulo)
        }

        // ============ Sistema.Colecoes.Conjunto ============
        "Conjunto::Adicionar" => {
            let item = args.into_iter().next().unwrap_or(Valor::Nulo);
            if let Some(campos) = campos_de(&este) {
                let mut itens = ler_array_interno(&campos, "__itens__");
                if itens.contains(&item) {
                    return Ok(Valor::Booleano(false));
                }
                itens.push(item);
                escrever_array_interno(&campos, "__itens__", itens);
                return Ok(Valor::Booleano(true));
            }
            Ok(Valor::Booleano(false))
        }
        "Conjunto::Remover" => {
            let item = args.into_iter().next().unwrap_or(Valor::Nulo);
            if let Some(campos) = campos_de(&este) {
                let mut itens = ler_array_interno(&campos, "__itens__");
                if let Some(i) = itens.iter().position(|v| *v == item) {
                    itens.remove(i);
                    escrever_array_interno(&campos, "__itens__", itens);
                    return Ok(Valor::Booleano(true));
                }
            }
            Ok(Valor::Booleano(false))
        }
        "Conjunto::Contem" => {
            let item = args.into_iter().next().unwrap_or(Valor::Nulo);
            if let Some(campos) = campos_de(&este) {
                let itens = ler_array_interno(&campos, "__itens__");
                return Ok(Valor::Booleano(itens.contains(&item)));
            }
            Ok(Valor::Booleano(false))
        }
        "Conjunto::EstaVazia" => Ok(Valor::Booleano(
            campos_de(&este)
                .map(|c| ler_array_interno(&c, "__itens__").is_empty())
                .unwrap_or(true),
        )),
        "Conjunto::Limpar" => {
            if let Some(campos) = campos_de(&este) {
                escrever_array_interno(&campos, "__itens__", Vec::new());
            }
            Ok(Valor::Nulo)
        }

        // ============ Sistema.Colecoes.Lista — métodos extras ============
        "Lista::Inverter" => {
            if let Some(campos) = campos_de(&este) {
                let mut itens = ler_array_interno(&campos, "__itens__");
                itens.reverse();
                escrever_array_interno(&campos, "__itens__", itens);
            }
            Ok(Valor::Nulo)
        }
        "Lista::Ordenar" => {
            if let Some(campos) = campos_de(&este) {
                let mut itens = ler_array_interno(&campos, "__itens__");
                itens.sort_by(comparar_valores);
                escrever_array_interno(&campos, "__itens__", itens);
            }
            Ok(Valor::Nulo)
        }
        "Lista::ObterPrimeiro" => {
            if let Some(campos) = campos_de(&este) {
                let itens = ler_array_interno(&campos, "__itens__");
                return Ok(itens.first().cloned().unwrap_or(Valor::Nulo));
            }
            Ok(Valor::Nulo)
        }
        "Lista::ObterUltimo" => {
            if let Some(campos) = campos_de(&este) {
                let itens = ler_array_interno(&campos, "__itens__");
                return Ok(itens.last().cloned().unwrap_or(Valor::Nulo));
            }
            Ok(Valor::Nulo)
        }
        "Lista::EstaVazia" => Ok(Valor::Booleano(
            campos_de(&este)
                .map(|c| ler_array_interno(&c, "__itens__").is_empty())
                .unwrap_or(true),
        )),
        "Lista::IndiceDe" => {
            let item = args.into_iter().next().unwrap_or(Valor::Nulo);
            if let Some(campos) = campos_de(&este) {
                let itens = ler_array_interno(&campos, "__itens__");
                let idx = itens.iter().position(|v| *v == item);
                return Ok(Valor::Inteiro(idx.map(|i| i as i64).unwrap_or(-1)));
            }
            Ok(Valor::Inteiro(-1))
        }
        "Lista::Recortar" => {
            let mut it = args.into_iter();
            let inicio = it.next().map(|v| numero_de(&v) as i64).unwrap_or(0);
            let fim = it.next().map(|v| numero_de(&v) as i64).unwrap_or(0);
            if let Some(campos) = campos_de(&este) {
                let itens = ler_array_interno(&campos, "__itens__");
                let len = itens.len() as i64;
                let ini = inicio.max(0);
                let fi = fim.min(len);
                let fatia: Vec<Valor> = if ini < fi {
                    itens[ini as usize..fi as usize].to_vec()
                } else {
                    Vec::new()
                };
                let mut campos_lista = HashMap::new();
                campos_lista.insert("__itens__".to_string(), Valor::Array(fatia));
                return Ok(Valor::Objeto {
                    nome_classe: "Sistema.Colecoes.Lista".to_string(),
                    campos: Rc::new(RefCell::new(campos_lista)),
                    metodos: HashMap::new(),
                });
            }
            let mut campos_lista = HashMap::new();
            campos_lista.insert("__itens__".to_string(), Valor::Array(Vec::new()));
            Ok(Valor::Objeto {
                nome_classe: "Sistema.Colecoes.Lista".to_string(),
                campos: Rc::new(RefCell::new(campos_lista)),
                metodos: HashMap::new(),
            })
        }

        // ============ Sistema.Colecoes.Lista — métodos adicionais ============
        "Lista::Inserir" => {
            let mut it = args.into_iter();
            let indice = it.next().map(|v| numero_de(&v) as i64).unwrap_or(0);
            let item = it.next().unwrap_or(Valor::Nulo);
            if let Some(campos) = campos_de(&este) {
                let mut itens = ler_array_interno(&campos, "__itens__");
                let len = itens.len() as i64;
                let idx = indice.clamp(0, len) as usize;
                itens.insert(idx, item);
                escrever_array_interno(&campos, "__itens__", itens);
            }
            Ok(Valor::Nulo)
        }
        "Lista::RemoverUltimo" => {
            if let Some(campos) = campos_de(&este) {
                let mut itens = ler_array_interno(&campos, "__itens__");
                let item = itens.pop().unwrap_or(Valor::Nulo);
                escrever_array_interno(&campos, "__itens__", itens);
                return Ok(item);
            }
            Ok(Valor::Nulo)
        }
        "Lista::AdicionarSeAusente" => {
            let item = args.into_iter().next().unwrap_or(Valor::Nulo);
            if let Some(campos) = campos_de(&este) {
                let mut itens = ler_array_interno(&campos, "__itens__");
                if itens.contains(&item) {
                    return Ok(Valor::Booleano(false));
                }
                itens.push(item);
                escrever_array_interno(&campos, "__itens__", itens);
                return Ok(Valor::Booleano(true));
            }
            Ok(Valor::Booleano(false))
        }
        "Lista::ParaArray" => {
            if let Some(campos) = campos_de(&este) {
                let itens = ler_array_interno(&campos, "__itens__");
                return Ok(Valor::Array(itens));
            }
            Ok(Valor::Array(Vec::new()))
        }
        "Lista::OrdenarDecrescente" => {
            if let Some(campos) = campos_de(&este) {
                let mut itens = ler_array_interno(&campos, "__itens__");
                itens.sort_by(|a, b| comparar_valores(b, a));
                escrever_array_interno(&campos, "__itens__", itens);
            }
            Ok(Valor::Nulo)
        }

        // ============ Sistema.Colecoes.Fila / Pilha — conversões ============
        "Fila::ParaLista" | "Pilha::ParaLista" => {
            if let Some(campos) = campos_de(&este) {
                let itens = ler_array_interno(&campos, "__itens__");
                return Ok(objeto_com_itens("Sistema.Colecoes.Lista", itens));
            }
            Ok(objeto_com_itens("Sistema.Colecoes.Lista", Vec::new()))
        }
        "Fila::ParaArray" | "Pilha::ParaArray" => {
            if let Some(campos) = campos_de(&este) {
                let itens = ler_array_interno(&campos, "__itens__");
                return Ok(Valor::Array(itens));
            }
            Ok(Valor::Array(Vec::new()))
        }

        // ============ Sistema.Colecoes.Conjunto — operações de conjunto ============
        "Conjunto::Uniao" | "Conjunto::Intersecao" | "Conjunto::Diferenca" => {
            let outro = args.into_iter().next().unwrap_or(Valor::Nulo);
            let itens_outro = match &outro {
                Valor::Objeto { campos: c, .. } => ler_array_interno(c, "__itens__"),
                Valor::Array(v) => v.clone(),
                _ => Vec::new(),
            };
            if let Some(campos) = campos_de(&este) {
                let itens = ler_array_interno(&campos, "__itens__");
                let resultado: Vec<Valor> = match chave {
                    "Conjunto::Uniao" => {
                        let mut r = itens.clone();
                        for item in itens_outro {
                            if !r.contains(&item) {
                                r.push(item);
                            }
                        }
                        r
                    }
                    "Conjunto::Intersecao" => itens
                        .into_iter()
                        .filter(|item| itens_outro.contains(item))
                        .collect(),
                    _ => itens
                        .into_iter()
                        .filter(|item| !itens_outro.contains(item))
                        .collect(),
                };
                return Ok(objeto_com_itens("Sistema.Colecoes.Conjunto", resultado));
            }
            Ok(objeto_com_itens("Sistema.Colecoes.Conjunto", Vec::new()))
        }
        "Conjunto::ParaArray" => {
            if let Some(campos) = campos_de(&este) {
                let itens = ler_array_interno(&campos, "__itens__");
                return Ok(Valor::Array(itens));
            }
            Ok(Valor::Array(Vec::new()))
        }

        // ============ Sistema.Data.Horario — instância ============
        "Horario::TotalSegundos" => Ok(Valor::Inteiro(segundos_de_horario(&este))),
        "Horario::TotalMinutos" => Ok(Valor::Inteiro(segundos_de_horario(&este) / 60)),
        "Horario::TotalHoras" => Ok(Valor::Inteiro(segundos_de_horario(&este) / 3600)),
        "Horario::TotalDias" => Ok(Valor::Inteiro(segundos_de_horario(&este) / 86400)),
        "Horario::ParaTexto" => Ok(Valor::Texto(horario_para_texto(segundos_de_horario(&este)))),
        "Horario::Adicionar" | "Horario::Subtrair" => {
            let outro = args.into_iter().next().unwrap_or(Valor::Nulo);
            let seg = segundos_de_horario(&este);
            let seg_outro = segundos_de_horario(&outro);
            let novo = if chave == "Horario::Adicionar" {
                seg.saturating_add(seg_outro)
            } else {
                seg.saturating_sub(seg_outro)
            };
            Ok(objeto_horario(novo))
        }
        "Horario::Igual" | "Horario::MaiorQue" | "Horario::MenorQue" => {
            let outro = args.into_iter().next().unwrap_or(Valor::Nulo);
            let seg = segundos_de_horario(&este);
            let seg_outro = segundos_de_horario(&outro);
            let resultado = match chave {
                "Horario::Igual" => seg == seg_outro,
                "Horario::MaiorQue" => seg > seg_outro,
                _ => seg < seg_outro,
            };
            Ok(Valor::Booleano(resultado))
        }

        // ============ Sistema.Rede.ClienteHttp — instância (síncrono) ============
        "ClienteHttp::Obter" | "ClienteHttp::Get" => {
            let url = args
                .into_iter()
                .next()
                .map(|v| texto_de(&v))
                .unwrap_or_default();
            let url = resolver_url_com_base(&este, &url);
            match http_request_sync("GET", &url, "") {
                Ok(corpo) => Ok(Valor::Texto(corpo)),
                Err(e) => Err(e),
            }
        }
        "ClienteHttp::Enviar" | "ClienteHttp::Post" => {
            let mut it = args.into_iter();
            let url = it.next().map(|v| texto_de(&v)).unwrap_or_default();
            let corpo_req = it.next().map(|v| texto_de(&v)).unwrap_or_default();
            let url = resolver_url_com_base(&este, &url);
            match http_request_sync("POST", &url, &corpo_req) {
                Ok(corpo) => Ok(Valor::Texto(corpo)),
                Err(e) => Err(e),
            }
        }
        "ClienteHttp::Atualizar" | "ClienteHttp::Put" => {
            let mut it = args.into_iter();
            let url = it.next().map(|v| texto_de(&v)).unwrap_or_default();
            let corpo_req = it.next().map(|v| texto_de(&v)).unwrap_or_default();
            let url = resolver_url_com_base(&este, &url);
            match http_request_sync("PUT", &url, &corpo_req) {
                Ok(corpo) => Ok(Valor::Texto(corpo)),
                Err(e) => Err(e),
            }
        }
        "ClienteHttp::Excluir" | "ClienteHttp::Delete" => {
            let url = args
                .into_iter()
                .next()
                .map(|v| texto_de(&v))
                .unwrap_or_default();
            let url = resolver_url_com_base(&este, &url);
            match http_request_sync("DELETE", &url, "") {
                Ok(corpo) => Ok(Valor::Texto(corpo)),
                Err(e) => Err(e),
            }
        }

        // ============ Sistema.Texto.Regex — instância ============
        "Regex::Corresponde" => {
            let texto = args.first().map(texto_de).unwrap_or_default();
            Ok(Valor::Booleano(
                regex_do_objeto(&este)
                    .map(|r| r.is_match(&texto))
                    .unwrap_or(false),
            ))
        }
        "Regex::Localizar" => {
            let texto = args.first().map(texto_de).unwrap_or_default();
            let pos = regex_do_objeto(&este)
                .and_then(|r| r.find(&texto).map(|m| m.start()));
            match pos {
                Some(byte_idx) => Ok(Valor::Inteiro(
                    texto[..byte_idx].chars().count() as i64
                )),
                None => Ok(Valor::Inteiro(-1)),
            }
        }
        "Regex::Substituir" => {
            let texto = args.first().map(texto_de).unwrap_or_default();
            let substituto = args.get(1).map(texto_de).unwrap_or_default();
            Ok(Valor::Texto(
                regex_do_objeto(&este)
                    .map(|r| r.replace_all(&texto, substituto.as_str()).into_owned())
                    .unwrap_or(texto),
            ))
        }
        "Regex::Dividir" => {
            let texto = args.first().map(texto_de).unwrap_or_default();
            let partes: Vec<Valor> = match regex_do_objeto(&este) {
                Some(r) => r
                    .split(&texto)
                    .map(|p| Valor::Texto(p.to_string()))
                    .collect(),
                None => vec![Valor::Texto(texto)],
            };
            Ok(Valor::Array(partes))
        }
        "Regex::Contar" => {
            let texto = args.first().map(texto_de).unwrap_or_default();
            let n = regex_do_objeto(&este)
                .map(|r| r.find_iter(&texto).count() as i64)
                .unwrap_or(0);
            Ok(Valor::Inteiro(n))
        }
        "Regex::CorrespondeCompleto" => {
            let texto = args.first().map(texto_de).unwrap_or_default();
            let ok = regex_do_objeto(&este)
                .map(|r| {
                    r.find(&texto)
                        .map(|m| m.start() == 0 && m.end() == texto.len())
                        .unwrap_or(false)
                })
                .unwrap_or(false);
            Ok(Valor::Booleano(ok))
        }
        "Regex::LocalizarTodas" => {
            let texto = args.first().map(texto_de).unwrap_or_default();
            let partes: Vec<Valor> = match regex_do_objeto(&este) {
                Some(r) => r
                    .find_iter(&texto)
                    .map(|m| Valor::Texto(m.as_str().to_string()))
                    .collect(),
                None => Vec::new(),
            };
            Ok(Valor::Array(partes))
        }
        "Regex::SubstituirPrimeiro" => {
            let texto = args.first().map(texto_de).unwrap_or_default();
            let substituto = args.get(1).map(texto_de).unwrap_or_default();
            let resultado = match regex_do_objeto(&este) {
                Some(r) => r.replacen(&texto, 1, substituto.as_str()).into_owned(),
                None => texto,
            };
            Ok(Valor::Texto(resultado))
        }

        // ============ Sistema.IO.Leitor — instância ============
        "Leitor::LerLinha" => {
            if let Some(campos) = campos_de(&este) {
                let mut c = campos.borrow_mut();
                if c.get("__fechado") == Some(&Valor::Booleano(true)) {
                    return Err("Leitor::LerLinha: leitor já fechado".to_string());
                }
                let restante = match c.get("__restante") {
                    Some(Valor::Texto(s)) => s.clone(),
                    _ => String::new(),
                };
                if restante.is_empty() {
                    return Ok(Valor::Texto(String::new()));
                }
                let (linha, resto) = match restante.find('\n') {
                    Some(pos) => {
                        let mut linha = restante[..pos].to_string();
                        if linha.ends_with('\r') {
                            linha.pop();
                        }
                        (linha, restante[pos + 1..].to_string())
                    }
                    None => (restante.clone(), String::new()),
                };
                c.insert("__restante".to_string(), Valor::Texto(resto));
                return Ok(Valor::Texto(linha));
            }
            Ok(Valor::Nulo)
        }
        "Leitor::LerResto" => {
            if let Some(campos) = campos_de(&este) {
                let mut c = campos.borrow_mut();
                let resto = match c.get_mut("__restante") {
                    Some(Valor::Texto(s)) => std::mem::take(s),
                    _ => String::new(),
                };
                return Ok(Valor::Texto(resto));
            }
            Ok(Valor::Nulo)
        }
        "Leitor::EstaNoFim" | "Leitor::TemMais" => {
            if let Some(campos) = campos_de(&este) {
                let c = campos.borrow();
                let vazio = match c.get("__restante") {
                    Some(Valor::Texto(s)) => s.is_empty(),
                    _ => true,
                };
                let fechado = c.get("__fechado") == Some(&Valor::Booleano(true));
                let no_fim = vazio || fechado;
                return Ok(Valor::Booleano(if chave == "Leitor::EstaNoFim" {
                    no_fim
                } else {
                    !no_fim
                }));
            }
            Ok(Valor::Booleano(true))
        }
        "Leitor::Fechar" => {
            if let Some(campos) = campos_de(&este) {
                let mut c = campos.borrow_mut();
                c.insert("__fechado".to_string(), Valor::Booleano(true));
                c.insert("__restante".to_string(), Valor::Texto(String::new()));
            }
            Ok(Valor::Nulo)
        }

        // ============ Sistema.IO.InformacaoArquivo — instância ============
        "InformacaoArquivo::Existe" | "InformacaoArquivo::Nome"
        | "InformacaoArquivo::CaminhoCompleto" | "InformacaoArquivo::Tamanho"
        | "InformacaoArquivo::UltimaModificacao" | "InformacaoArquivo::Diretorio" => {
            let chave_campo = match chave {
                "InformacaoArquivo::Existe" => "__existe",
                "InformacaoArquivo::Nome" => "__nome",
                "InformacaoArquivo::CaminhoCompleto" => "__caminho",
                "InformacaoArquivo::Tamanho" => "__tamanho",
                "InformacaoArquivo::UltimaModificacao" => "__modificacao",
                _ => "__diretorio",
            };
            if let Some(campos) = campos_de(&este) {
                let c = campos.borrow();
                return Ok(c.get(chave_campo).cloned().unwrap_or(Valor::Nulo));
            }
            Ok(Valor::Nulo)
        }

        // ============ Sistema.Threading.Mutex — instância ============
        "Mutex::Entrar" => {
            if let Some(campos) = campos_de(&este) {
                let mut c = campos.borrow_mut();
                if c.get("__travado") == Some(&Valor::Booleano(true)) {
                    return Err("Mutex::Entrar: mutex já está travado (possível deadlock)".to_string());
                }
                c.insert("__travado".to_string(), Valor::Booleano(true));
            }
            Ok(Valor::Nulo)
        }
        "Mutex::TentarEntrar" => {
            if let Some(campos) = campos_de(&este) {
                let mut c = campos.borrow_mut();
                if c.get("__travado") == Some(&Valor::Booleano(true)) {
                    return Ok(Valor::Booleano(false));
                }
                c.insert("__travado".to_string(), Valor::Booleano(true));
                return Ok(Valor::Booleano(true));
            }
            Ok(Valor::Booleano(false))
        }
        "Mutex::Sair" | "Mutex::Fechar" => {
            if let Some(campos) = campos_de(&este) {
                let mut c = campos.borrow_mut();
                if chave == "Mutex::Sair" && c.get("__travado") != Some(&Valor::Booleano(true)) {
                    return Err("Mutex::Sair: mutex não está travado".to_string());
                }
                c.insert("__travado".to_string(), Valor::Booleano(false));
            }
            Ok(Valor::Nulo)
        }
        "Mutex::EstaTravado" => {
            if let Some(campos) = campos_de(&este) {
                let c = campos.borrow();
                return Ok(Valor::Booleano(
                    c.get("__travado") == Some(&Valor::Booleano(true)),
                ));
            }
            Ok(Valor::Booleano(false))
        }

        // ============ Sistema.Aleatorio.Aleatorio — instância ============
        "Aleatorio::Proximo" | "Aleatorio::ProximoEntre"
        | "Aleatorio::ProximoFlutuante" | "Aleatorio::ProximoBooleano"
        | "Aleatorio::Reseed" => {
            if let Some(campos) = campos_de(&este) {
                let mut c = campos.borrow_mut();
                let estado = match c.get("__estado__") {
                    Some(Valor::Inteiro(n)) => *n as u64,
                    _ => 0,
                };
                let mut estado = if estado == 0 {
                    let s = semente_aleatoria(0);
                    c.insert("__estado__".to_string(), Valor::Inteiro(s as i64));
                    s
                } else {
                    estado
                };
                match chave {
                    "Aleatorio::Reseed" => {
                        let nova = args.first().map(numero_de).unwrap_or(0.0) as u64;
                        let s = semente_aleatoria(nova);
                        c.insert("__estado__".to_string(), Valor::Inteiro(s as i64));
                        return Ok(Valor::Nulo);
                    }
                    "Aleatorio::ProximoEntre" => {
                        let min = args.first().map(numero_de).unwrap_or(0.0) as i64;
                        let max = args.get(1).map(numero_de).unwrap_or(0.0) as i64;
                        let faixa = (max - min).max(1) as u64;
                        let v = proximo_aleatorio_u64(&mut estado) % faixa;
                        c.insert("__estado__".to_string(), Valor::Inteiro(estado as i64));
                        return Ok(Valor::Inteiro(min + v as i64));
                    }
                    "Aleatorio::ProximoFlutuante" => {
                        let v = proximo_aleatorio_u64(&mut estado);
                        c.insert("__estado__".to_string(), Valor::Inteiro(estado as i64));
                        return Ok(Valor::Duplo((v >> 11) as f64 / (1u64 << 53) as f64));
                    }
                    "Aleatorio::ProximoBooleano" => {
                        let v = proximo_aleatorio_u64(&mut estado) & 1;
                        c.insert("__estado__".to_string(), Valor::Inteiro(estado as i64));
                        return Ok(Valor::Booleano(v == 1));
                    }
                    _ => {
                        let v = proximo_aleatorio_u64(&mut estado) % (i32::MAX as u64);
                        c.insert("__estado__".to_string(), Valor::Inteiro(estado as i64));
                        return Ok(Valor::Inteiro(v as i64));
                    }
                }
            }
            Ok(Valor::Nulo)
        }

        // ============ Sistema.Diagnostics.Cronometro — instância ============
        "Cronometro::Iniciar" | "Cronometro::Reiniciar" | "Cronometro::Parar"
        | "Cronometro::DecorridoSegundos" | "Cronometro::DecorridoMilissegundos"
        | "Cronometro::Decorrido" | "Cronometro::EstaExecutando" => {
            if let Some(campos) = campos_de(&este) {
                let agora = agora_ms() as i64;
                match chave {
                    "Cronometro::Iniciar" => {
                        let mut c = campos.borrow_mut();
                        let rodando = c
                            .get("__rodando__")
                            .map(|v| *v == Valor::Booleano(true))
                            .unwrap_or(false);
                        if !rodando {
                            c.insert("__inicio_ms__".to_string(), Valor::Inteiro(agora));
                            c.insert("__fim_ms__".to_string(), Valor::Inteiro(0));
                            c.insert("__rodando__".to_string(), Valor::Booleano(true));
                        }
                        return Ok(Valor::Nulo);
                    }
                    "Cronometro::Reiniciar" => {
                        let mut c = campos.borrow_mut();
                        c.insert("__inicio_ms__".to_string(), Valor::Inteiro(agora));
                        c.insert("__fim_ms__".to_string(), Valor::Inteiro(0));
                        c.insert("__rodando__".to_string(), Valor::Booleano(true));
                        return Ok(Valor::Nulo);
                    }
                    "Cronometro::Parar" => {
                        let mut c = campos.borrow_mut();
                        let rodando = c
                            .get("__rodando__")
                            .map(|v| *v == Valor::Booleano(true))
                            .unwrap_or(false);
                        if rodando {
                            c.insert("__fim_ms__".to_string(), Valor::Inteiro(agora));
                            c.insert("__rodando__".to_string(), Valor::Booleano(false));
                        }
                        return Ok(Valor::Nulo);
                    }
                    "Cronometro::EstaExecutando" => {
                        let c = campos.borrow();
                        let rodando = c
                            .get("__rodando__")
                            .map(|v| *v == Valor::Booleano(true))
                            .unwrap_or(false);
                        return Ok(Valor::Booleano(rodando));
                    }
                    _ => {
                        // DecorridoSegundos / DecorridoMilissegundos / Decorrido
                        let c = campos.borrow();
                        let inicio = match c.get("__inicio_ms__") {
                            Some(Valor::Inteiro(n)) => *n,
                            _ => return Ok(Valor::Duplo(0.0)),
                        };
                        let fim = match c.get("__fim_ms__") {
                            Some(Valor::Inteiro(n)) if *n > 0 => *n,
                            _ => agora,
                        };
                        let decorrido_ms = (fim - inicio).max(0) as u64;
                        return Ok(if chave == "Cronometro::DecorridoMilissegundos" {
                            Valor::Inteiro(decorrido_ms as i64)
                        } else {
                            Valor::Duplo(decorrido_ms as f64 / 1000.0)
                        });
                    }
                }
            }
            Ok(Valor::Nulo)
        }

        // ============ Sistema.Rede.SocketTcp — instância ============
        "SocketTcp::Enviar" => {
            let dados = args
                .into_iter()
                .next()
                .map(|v| texto_de(&v))
                .unwrap_or_default();
            let sucesso = socket_id_de(&este)
                .and_then(|sid| com_socket(sid, |s| s.write_all(dados.as_bytes()).is_ok()))
                .unwrap_or(false);
            if sucesso {
                Ok(Valor::Nulo)
            } else {
                Err("SocketTcp::Enviar: falha ao enviar dados".to_string())
            }
        }
        "SocketTcp::Receber" => {
            let id = socket_id_de(&este);
            match id {
                Some(sid) => match com_socket(sid, |s| {
                    let mut buf = [0u8; 65536];
                    match s.read(&mut buf) {
                        Ok(0) => None,
                        Ok(n) => Some(Ok(String::from_utf8_lossy(&buf[..n]).into_owned())),
                        Err(e)
                            if e.kind() == io::ErrorKind::WouldBlock
                                || e.kind() == io::ErrorKind::TimedOut =>
                        {
                            Some(Ok(String::new()))
                        }
                        Err(_) => Some(Err(())),
                    }
                }) {
                    Some(Some(Ok(texto))) => Ok(Valor::Texto(texto)),
                    Some(Some(Err(()))) => Err("SocketTcp::Receber: erro de leitura".to_string()),
                    Some(None) => Err("SocketTcp::Receber: conexão encerrada".to_string()),
                    None => Err("SocketTcp::Receber: socket inválido ou fechado".to_string()),
                },
                None => Err("SocketTcp::Receber: socket inválido".to_string()),
            }
        }
        "SocketTcp::Fechar" => {
            if let Some(id) = socket_id_de(&este) {
                remover_socket(id);
            }
            Ok(Valor::Nulo)
        }
        "SocketTcp::Conectado" => {
            match socket_id_de(&este) {
                Some(id) => Ok(Valor::Booleano(
                    com_socket(id, |_s| true).unwrap_or(false),
                )),
                None => Ok(Valor::Booleano(false)),
            }
        }

        // ============ Sistema.Rede.ServidorTcp — instância ============
        "ServidorTcp::Aceitar" => {
            let id = servidor_id_de(&este)
                .ok_or_else(|| "ServidorTcp::Aceitar: servidor inválido".to_string())?;
            let resultado = com_servidor(id, |listener| listener.accept());
            match resultado {
                Some(Ok((stream, _addr))) => {
                    let _ = stream.set_nodelay(true);
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                    let sid = registrar_socket(stream);
                    Ok(objeto_socket_tcp(sid))
                }
                Some(Err(e)) => Err(format!("ServidorTcp::Aceitar: {}", e)),
                None => Err("ServidorTcp::Aceitar: servidor fechado".to_string()),
            }
        }
        "ServidorTcp::Fechar" => {
            if let Some(id) = servidor_id_de(&este) {
                remover_servidor(id);
            }
            Ok(Valor::Nulo)
        }
        "ServidorTcp::EstaAberto" => {
            match servidor_id_de(&este) {
                Some(id) => Ok(Valor::Booleano(
                    com_servidor(id, |_l| true).unwrap_or(false),
                )),
                None => Ok(Valor::Booleano(false)),
            }
        }

        _ => {
            eprintln!(
                "[aviso nativo] Método nativo de instância '{}' não implementado; retornando nulo",
                chave
            );
            Ok(Valor::Nulo)
        }
    }
}

/// Informa se uma chave de nativo de instância está implementada.
pub(crate) fn nativo_instancia_conhecido(chave: &str) -> bool {
    matches!(
        chave,
        "Data::Formatar"
            | "Data::ParaTexto"
            | "Data::Igual"
            | "Dicionario::Adicionar"
            | "Dicionario::Obter"
            | "Dicionario::ContemChave"
            | "Dicionario::ContemValor"
            | "Dicionario::Remover"
            | "Dicionario::Limpar"
            | "Dicionario::Chaves"
            | "Dicionario::Valores"
            | "Dicionario::TentarObter"
            | "ConstrutorTexto::Anexar"
            | "ConstrutorTexto::AnexarLinha"
            | "ConstrutorTexto::AnexarValor"
            | "ConstrutorTexto::AnexarNovaLinha"
            | "ConstrutorTexto::Inserir"
            | "ConstrutorTexto::Remover"
            | "ConstrutorTexto::IndiceDe"
            | "ConstrutorTexto::Subtexto"
            | "ConstrutorTexto::ParaTexto"
            | "ConstrutorTexto::Limpar"
            | "ConstrutorTexto::Tamanho"
            | "ConstrutorTexto::Comprimento"
            | "ClienteHttp::Obter"
            | "ClienteHttp::Enviar"
            | "ClienteHttp::Atualizar"
            | "ClienteHttp::Excluir"
            | "ClienteHttp::Get"
            | "ClienteHttp::Post"
            | "ClienteHttp::Put"
            | "ClienteHttp::Delete"
            | "Fila::Enfileirar"
            | "Fila::Desenfileirar"
            | "Fila::Espiar"
            | "Fila::EstaVazia"
            | "Fila::Limpar"
            | "Pilha::Empilhar"
            | "Pilha::Desempilhar"
            | "Pilha::EspiarTopo"
            | "Pilha::EstaVazia"
            | "Pilha::Limpar"
            | "Conjunto::Adicionar"
            | "Conjunto::Remover"
            | "Conjunto::Contem"
            | "Conjunto::EstaVazia"
            | "Conjunto::Limpar"
            | "Lista::Inverter"
            | "Lista::Ordenar"
            | "Lista::ObterPrimeiro"
            | "Lista::ObterUltimo"
            | "Lista::EstaVazia"
            | "Lista::IndiceDe"
            | "Lista::Recortar"
            | "Lista::Inserir"
            | "Lista::RemoverUltimo"
            | "Lista::AdicionarSeAusente"
            | "Lista::ParaArray"
            | "Lista::OrdenarDecrescente"
            | "Fila::ParaLista"
            | "Fila::ParaArray"
            | "Pilha::ParaLista"
            | "Pilha::ParaArray"
            | "Conjunto::Uniao"
            | "Conjunto::Intersecao"
            | "Conjunto::Diferenca"
            | "Conjunto::ParaArray"
            | "Horario::TotalSegundos"
            | "Horario::TotalMinutos"
            | "Horario::TotalHoras"
            | "Horario::TotalDias"
            | "Horario::ParaTexto"
            | "Horario::Adicionar"
            | "Horario::Subtrair"
            | "Horario::Igual"
            | "Horario::MaiorQue"
            | "Horario::MenorQue"
            | "Leitor::LerLinha"
            | "Leitor::LerResto"
            | "Leitor::EstaNoFim"
            | "Leitor::TemMais"
            | "Leitor::Fechar"
            | "InformacaoArquivo::Existe"
            | "InformacaoArquivo::Nome"
            | "InformacaoArquivo::CaminhoCompleto"
            | "InformacaoArquivo::Tamanho"
            | "InformacaoArquivo::UltimaModificacao"
            | "InformacaoArquivo::Diretorio"
            | "Mutex::Entrar"
            | "Mutex::TentarEntrar"
            | "Mutex::Sair"
            | "Mutex::EstaTravado"
            | "Mutex::Fechar"
            | "Aleatorio::Proximo"
            | "Aleatorio::ProximoEntre"
            | "Aleatorio::ProximoFlutuante"
            | "Aleatorio::ProximoBooleano"
            | "Aleatorio::Reseed"
            | "Regex::Corresponde"
            | "Regex::Localizar"
            | "Regex::Substituir"
            | "Regex::Dividir"
            | "Regex::Contar"
            | "Regex::CorrespondeCompleto"
            | "Regex::LocalizarTodas"
            | "Regex::SubstituirPrimeiro"
            | "Cronometro::Iniciar"
            | "Cronometro::Parar"
            | "Cronometro::Reiniciar"
            | "Cronometro::DecorridoSegundos"
            | "Cronometro::DecorridoMilissegundos"
            | "Cronometro::Decorrido"
            | "Cronometro::EstaExecutando"
            | "SocketTcp::Enviar"
            | "SocketTcp::Receber"
            | "SocketTcp::Fechar"
            | "SocketTcp::Conectado"
            | "ServidorTcp::Aceitar"
            | "ServidorTcp::Fechar"
            | "ServidorTcp::EstaAberto"
    )
}

/// Informa se uma chave de nativo estático está implementada.
pub(crate) fn nativo_estatico_conhecido(chave: &str) -> bool {
    matches!(
        chave,
        "Console::EscreverLinha"
            | "Console::Escrever"
            | "Console::LerLinha"
            | "Arquivo::LerTexto"
            | "Arquivo::EscreverTexto"
            | "Arquivo::AdicionarTexto"
            | "Arquivo::Existe"
            | "Arquivo::Excluir"
            | "Arquivo::Copiar"
            | "Arquivo::Mover"
            | "Diretorio::Existe"
            | "Diretorio::Criar"
            | "Diretorio::Excluir"
            | "Diretorio::ObterAtual"
            | "Diretorio::DefinirAtual"
            | "Diretorio::ObterArquivos"
            | "Diretorio::ObterDiretorios"
            | "Arquivo::Tamanho"
            | "Arquivo::Criar"
            | "Arquivo::LerLinhas"
            | "Leitor::Abrir"
            | "InformacaoArquivo::Abrir"
            | "Caminho::Combinar"
            | "Caminho::NomeArquivo"
            | "Caminho::NomeSemExtensao"
            | "Caminho::DiretorioPai"
            | "Caminho::Extensao"
            | "Caminho::Absoluto"
            | "Caminho::Separadores"
            | "Caminho::Separador"
            | "Data::Agora"
            | "Data::Hoje"
            | "Matematica::Abs"
            | "Matematica::Max"
            | "Matematica::Min"
            | "Matematica::Potencia"
            | "Matematica::RaizQuadrada"
            | "Matematica::Arredondar"
            | "Matematica::Truncar"
            | "Matematica::Piso"
            | "Matematica::Teto"
            | "Matematica::Seno"
            | "Matematica::Cosseno"
            | "Matematica::Tangente"
            | "Matematica::LogaritmoNatural"
            | "Matematica::LogaritmoDez"
            | "Matematica::Expoente"
            | "Matematica::Pi"
            | "Matematica::E"
            | "Tarefa::Atraso"
            | "Json::Serializar"
            | "Json::ValidarJson"
            | "Json::Formatar"
            | "Json::Desserializar"
            | "Json::SerializarFormatado"
            | "Json::TipoDe"
            | "Texto::Dividir"
            | "Texto::DividirLinhas"
            | "Texto::Juntar"
            | "Texto::RemoverEspacos"
            | "Texto::ParaMaiusculas"
            | "Texto::ParaMinusculas"
            | "Texto::Substituir"
            | "Texto::ComecaCom"
            | "Texto::TerminaCom"
            | "Texto::Contem"
            | "Texto::VazioOuNulo"
            | "Texto::Tamanho"
            | "Codificacao::ParaBytes"
            | "Codificacao::ParaTexto"
            | "Codificacao::TamanhoBytes"
            | "Codificacao::Base64Codificar"
            | "Codificacao::Base64Decodificar"
            | "Codificacao::ParaHex"
            | "Codificacao::DeHex"
            | "Regex::Escape"
            | "Regex::EhValido"
            | "Temporizador::Aguardar"
            | "PoolTarefas::ObterMaximoParalelismo"
            | "PoolTarefas::DefinirMaximoParalelismo"
            | "Guid::Novo"
            | "Guid::Vazio"
            | "Guid::EhValido"
            | "Texto::Subtexto"
            | "Texto::IndiceDe"
            | "Texto::UltimoIndiceDe"
            | "Texto::Inverter"
            | "Texto::PadEsquerda"
            | "Texto::PadDireita"
            | "Texto::ContarOcorrencias"
            | "Texto::EstaVazia"
            | "Texto::Repete"
            | "Texto::Comparar"
            | "Texto::IgualIgnoreCase"
            | "Texto::Remover"
            | "Texto::Inserir"
            | "Texto::NormalizarEspacos"
            | "Texto::DividirPalavras"
            | "Texto::ContarPalavras"
            | "Texto::LocalizarTodas"
            | "Ambiente::Argumentos"
            | "Ambiente::VariavelAmbiente"
            | "Ambiente::DefinirVariavelAmbiente"
            | "Ambiente::NomeComputador"
            | "Conversao::ParaInteiro"
            | "Conversao::ParaFlutuante"
            | "Conversao::ParaDuplo"
            | "Conversao::ParaDecimal"
            | "Conversao::ParaBooleano"
            | "Conversao::EhNumero"
            | "Dns::Resolver"
            | "Dns::ResolverTodos"
            | "SocketTcp::Conectar"
            | "ServidorTcp::Ouvir"
            | "Horario::DeSegundos"
            | "Horario::DeMinutos"
            | "Horario::DeHoras"
            | "Horario::DeDias"
    )
}

// Ponto de entrada do programa interpretador.
