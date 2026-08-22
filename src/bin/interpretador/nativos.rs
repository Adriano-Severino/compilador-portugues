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
use std::thread;
use std::time::Duration;
pub(crate) fn chrono_or_default_date() -> String {
    // Usa SystemTime para obter a data atual sem deps externas
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Cálculo simplificado de data a partir de segundos unix
    let days = secs / 86400;
    let years = 1970 + days / 365;
    let rem_days = days % 365;
    let months = rem_days / 30 + 1;
    let day = rem_days % 30 + 1;
    format!("{:04}-{:02}-{:02}", years, months, day)
}

pub(crate) async fn despachar_nativo_assincrono(
    chave: &str,
    args: Vec<Valor>,
) -> Result<Valor, String> {
    match chave {
        // ============ Arquivo assíncrono ============
        "LerArquivoAssíncrono" | "Arquivo::LerTextoAssíncrono" => {
            let caminho = args
                .into_iter()
                .next()
                .map(|v| v.to_string())
                .unwrap_or_default();
            let conteudo = tokio::fs::read_to_string(&caminho)
                .await
                .map_err(|e| format!("LerArquivoAssíncrono: {}", e))?;
            Ok(Valor::Texto(conteudo))
        }
        "EscreverArquivoAssíncrono" | "Arquivo::EscreverTextoAssíncrono" => {
            let mut it = args.into_iter();
            let caminho = it.next().map(|v| v.to_string()).unwrap_or_default();
            let conteudo = it.next().map(|v| v.to_string()).unwrap_or_default();
            tokio::fs::write(&caminho, conteudo.as_bytes())
                .await
                .map_err(|e| format!("EscreverArquivoAssíncrono: {}", e))?;
            Ok(Valor::Nulo)
        }
        "VerificarArquivoAssíncrono" | "Arquivo::ExisteAssíncrono" => {
            let caminho = args
                .into_iter()
                .next()
                .map(|v| v.to_string())
                .unwrap_or_default();
            let existe = tokio::fs::try_exists(&caminho).await.unwrap_or(false);
            Ok(Valor::Booleano(existe))
        }
        "AdicionarTextoAssíncrono" | "Arquivo::AdicionarTextoAssíncrono" => {
            use tokio::io::AsyncWriteExt;
            let mut it = args.into_iter();
            let caminho = it.next().map(|v| v.to_string()).unwrap_or_default();
            let conteudo = it.next().map(|v| v.to_string()).unwrap_or_default();
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
        // ============ Placeholder para HTTP/rede (futuro reqwest) ============
        "HttpGetAsync" | "Rede::HttpGetAsync" => {
            eprintln!(
                "[aviso] {} não implementado ainda (requer reqwest); retornando vazio",
                chave
            );
            Ok(Valor::Texto(String::new()))
        }
        "HttpPostAsync" | "Rede::HttpPostAsync" => {
            eprintln!(
                "[aviso] {} não implementado ainda (requer reqwest); retornando vazio",
                chave
            );
            Ok(Valor::Texto(String::new()))
        }
        // Fallback: tenta síncrono
        outro => {
            eprintln!(
                "[aviso] Função assíncrona nativa '{}' não implementada; retornando nulo",
                outro
            );
            Ok(Valor::Nulo)
        }
    }
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

        // ============ Sistema.Data.Data ============
        "Data::Agora" | "Data::Hoje" => {
            use std::time::{SystemTime, UNIX_EPOCH};
            let secs = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let days = secs / 86400;
            let year = 1970 + days / 365;
            let rem = days % 365;
            let month = rem / 30 + 1;
            let day = rem % 30 + 1;
            let mut campos = HashMap::new();
            campos.insert("Ano".to_string(), Valor::Inteiro(year as i64));
            campos.insert("Mes".to_string(), Valor::Inteiro(month as i64));
            campos.insert("Dia".to_string(), Valor::Inteiro(day as i64));
            campos.insert("Hora".to_string(), Valor::Inteiro(0));
            campos.insert("Minuto".to_string(), Valor::Inteiro(0));
            campos.insert("Segundo".to_string(), Valor::Inteiro(0));
            Ok(Valor::Objeto {
                nome_classe: "Sistema.Data.Data".to_string(),
                campos: Rc::new(RefCell::new(campos)),
                metodos: HashMap::new(),
            })
        }

        // ============ Sistema.Texto.Json ============
        "Json::Serializar" => {
            // Serialização básica para fins de demonstração
            let obj = args.into_iter().next().unwrap_or(Valor::Nulo);
            Ok(Valor::Texto(format!("{}", obj)))
        }
        "Json::ValidarJson" => {
            let json = args
                .into_iter()
                .next()
                .map(|v| v.to_string())
                .unwrap_or_default();
            let valido = json.trim().starts_with('{') || json.trim().starts_with('[');
            Ok(Valor::Booleano(valido))
        }
        "Json::Formatar" => {
            let json = args
                .into_iter()
                .next()
                .map(|v| v.to_string())
                .unwrap_or_default();
            Ok(Valor::Texto(json)) // Retorna como está por ora
        }

        // ============ Sistema.Rede.ClienteHttp (estáticos não existem — só instância) ============
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

        // ============ Sistema.Rede.ClienteHttp — instância ============
        "ClienteHttp::Get" => {
            let url = args
                .into_iter()
                .next()
                .map(|v| v.to_string())
                .unwrap_or_default();
            eprintln!(
                "[aviso] ClienteHttp::Get('{}') — requer runtime HTTP; retornando vazio",
                url
            );
            Ok(Valor::Texto(String::new()))
        }
        "ClienteHttp::Post" | "ClienteHttp::Put" => {
            let mut it = args.into_iter();
            let url = it.next().map(|v| v.to_string()).unwrap_or_default();
            eprintln!(
                "[aviso] ClienteHttp::Post/Put('{}') — requer runtime HTTP; retornando vazio",
                url
            );
            Ok(Valor::Texto(String::new()))
        }
        "ClienteHttp::Delete" => {
            let url = args
                .into_iter()
                .next()
                .map(|v| v.to_string())
                .unwrap_or_default();
            eprintln!(
                "[aviso] ClienteHttp::Delete('{}') — requer runtime HTTP; retornando vazio",
                url
            );
            Ok(Valor::Texto(String::new()))
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

// Ponto de entrada do programa interpretador.
