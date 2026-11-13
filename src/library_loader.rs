//! src/library_loader.rs
//!
//! Carrega metadados de tipo de arquivos de biblioteca compilada.
//! Suporta o formato `.pbl` — Biblioteca Por do Sol (formato com seções
//! [MANIFESTO] e [BYTECODE]). O formato legado `.pbc` não é mais aceito.
//!
//! O compilador usa apenas o manifesto para verificação de tipos, sem carregar o bytecode
//! completo na memória — equivalente ao mecanismo de Reference Assemblies do .NET.

use std::collections::HashMap;
use std::io;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct LibMetodo {
    pub nome: String,
    pub nome_classe: String,
    pub tipo_retorno: String,
    pub parametros: Vec<(String, String)>, // Vec<(tipo, nome)>
    pub aridade: usize,
    /// Para métodos marcados com [Nativo]: chave de despacho no runtime (ex. "Console::EscreverLinha")
    pub chave_nativa: Option<String>,
    pub eh_estatica: bool,
    /// Método assíncrono (assíncrona) — deve ser aguardado com `aguarde`.
    pub eh_assincrona: bool,
}

#[derive(Debug, Clone)]
pub struct LibPropriedade {
    pub nome: String,
    pub tipo: String,
}

#[derive(Debug, Clone)]
pub struct LibCampo {
    pub nome: String,
    pub tipo: String,
}

#[derive(Debug, Clone)]
pub struct LibClasse {
    pub fqn: String, // Full Qualified Name, e.g., "Sistema.Colecoes.Dicionario"
    pub nome: String,
    pub nome_pai: Option<String>,
    pub metodos: HashMap<String, LibMetodo>,
    pub propriedades: Vec<LibPropriedade>,
    pub campos: Vec<LibCampo>,
    pub eh_estatica: bool,
}

#[derive(Debug, Clone)]
pub struct LibFuncao {
    pub nome: String,
    pub aridade: usize,
}

#[derive(Debug, Clone)]
pub enum LibSimbolo {
    Classe(LibClasse),
    Funcao(LibFuncao),
}

#[derive(Debug, Default, Clone)]
pub struct Biblioteca {
    pub simbolos: HashMap<String, LibSimbolo>,
}

impl Biblioteca {
    pub fn new() -> Self {
        Self::default()
    }
}

// ============================================================================
// Ponto de entrada público
// ============================================================================

/// Carrega uma biblioteca a partir de um arquivo `.pbl`.
/// Retorna os metadados necessários para a verificação de tipos.
pub fn carregar_biblioteca(caminho: &Path) -> io::Result<Biblioteca> {
    let extensao = caminho.extension().and_then(|s| s.to_str()).unwrap_or("");
    if extensao != "pbl" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "Formato de biblioteca não suportado: .{} (esperado .pbl)",
                extensao
            ),
        ));
    }
    carregar_pbl(caminho)
}

// ============================================================================
// Formato .pbl (Biblioteca Por do Sol)
// ============================================================================

fn carregar_pbl(caminho: &Path) -> io::Result<Biblioteca> {
    let conteudo = std::fs::read_to_string(caminho)?;
    let mut biblioteca = Biblioteca::new();
    let mut em_manifesto = false;

    for linha in conteudo.lines() {
        let linha = linha.trim();
        match linha {
            "[MANIFESTO]" => {
                em_manifesto = true;
                continue;
            }
            "[BYTECODE]" | "[PBL]" => {
                em_manifesto = false;
                continue;
            }
            _ => {}
        }
        if linha.is_empty() || linha.starts_with(';') || linha.starts_with('#') {
            continue;
        }
        // Ignorar metadados de cabeçalho (nome=, versao=, …)
        if linha.contains('=')
            && !linha.starts_with("DEFINE")
            && !linha.starts_with("PROPERTY")
            && !linha.starts_with("FIELD")
        {
            continue;
        }
        if em_manifesto {
            processar_linha_manifesto(linha, &mut biblioteca);
        }
    }

    Ok(biblioteca)
}

fn processar_linha_manifesto(linha: &str, biblioteca: &mut Biblioteca) {
    let partes: Vec<&str> = linha.split_whitespace().collect();
    if partes.is_empty() {
        return;
    }

    match partes[0] {
        "DEFINE_CLASS" => {
            if let Some(fqn) = partes.get(1) {
                let nome_pai = partes.get(2).and_then(|&p| {
                    if p == "NULO" {
                        None
                    } else {
                        Some(p.to_string())
                    }
                });
                let nome_simples = fqn.split('.').last().unwrap_or(fqn).to_string();
                let classe = LibClasse {
                    fqn: fqn.to_string(),
                    nome: nome_simples,
                    nome_pai,
                    metodos: HashMap::new(),
                    propriedades: Vec::new(),
                    campos: Vec::new(),
                    eh_estatica: false,
                };
                biblioteca
                    .simbolos
                    .insert(fqn.to_string(), LibSimbolo::Classe(classe));
            }
        }
        "DEFINE_STATIC_CLASS" => {
            if let Some(fqn) = partes.get(1) {
                let nome_simples = fqn.split('.').last().unwrap_or(fqn).to_string();
                let classe = LibClasse {
                    fqn: fqn.to_string(),
                    nome: nome_simples,
                    nome_pai: None,
                    metodos: HashMap::new(),
                    propriedades: Vec::new(),
                    campos: Vec::new(),
                    eh_estatica: true,
                };
                biblioteca
                    .simbolos
                    .insert(fqn.to_string(), LibSimbolo::Classe(classe));
            }
        }
        "PROPERTY" => {
            // PROPERTY <fqn_classe> <nome> <tipo>
            if let (Some(fqn), Some(nome), Some(tipo)) =
                (partes.get(1), partes.get(2), partes.get(3))
            {
                if let Some(LibSimbolo::Classe(cl)) = biblioteca.simbolos.get_mut(*fqn) {
                    cl.propriedades.push(LibPropriedade {
                        nome: nome.to_string(),
                        tipo: tipo.to_string(),
                    });
                }
            }
        }
        "FIELD" => {
            // FIELD <fqn_classe> <nome> <tipo>
            if let (Some(fqn), Some(nome), Some(tipo)) =
                (partes.get(1), partes.get(2), partes.get(3))
            {
                if let Some(LibSimbolo::Classe(cl)) = biblioteca.simbolos.get_mut(*fqn) {
                    cl.campos.push(LibCampo {
                        nome: nome.to_string(),
                        tipo: tipo.to_string(),
                    });
                }
            }
        }
        // Métodos nativos: DEFINE_STATIC_NATIVE_METHOD <fqn> <nome> <ret> <chave> [params...]
        "DEFINE_STATIC_NATIVE_METHOD" => {
            if let (Some(fqn), Some(nome), Some(ret), Some(chave)) =
                (partes.get(1), partes.get(2), partes.get(3), partes.get(4))
            {
                let params = parse_params(&partes[5..]);
                let metodo = LibMetodo {
                    nome: nome.to_string(),
                    nome_classe: fqn.to_string(),
                    tipo_retorno: ret.to_string(),
                    parametros: params.clone(),
                    aridade: params.len(),
                    chave_nativa: Some(chave.to_string()),
                    eh_estatica: true,
                    eh_assincrona: partes.contains(&"ASSINCRONO"),
                };
                if let Some(LibSimbolo::Classe(cl)) = biblioteca.simbolos.get_mut(*fqn) {
                    cl.metodos.insert(nome.to_string(), metodo);
                }
            }
        }
        // DEFINE_NATIVE_METHOD <fqn> <nome> <ret> <chave> [params...]
        "DEFINE_NATIVE_METHOD" => {
            if let (Some(fqn), Some(nome), Some(ret), Some(chave)) =
                (partes.get(1), partes.get(2), partes.get(3), partes.get(4))
            {
                let params = parse_params(&partes[5..]);
                let metodo = LibMetodo {
                    nome: nome.to_string(),
                    nome_classe: fqn.to_string(),
                    tipo_retorno: ret.to_string(),
                    parametros: params.clone(),
                    aridade: params.len(),
                    chave_nativa: Some(chave.to_string()),
                    eh_estatica: false,
                    eh_assincrona: partes.contains(&"ASSINCRONO"),
                };
                if let Some(LibSimbolo::Classe(cl)) = biblioteca.simbolos.get_mut(*fqn) {
                    cl.metodos.insert(nome.to_string(), metodo);
                }
            }
        }
        // Métodos normais: DEFINE_STATIC_METHOD / DEFINE_METHOD <fqn> <nome> <ret> <nparams> [params...]
        "DEFINE_STATIC_METHOD" | "DEFINE_METHOD" => {
            let eh_estatica = partes[0] == "DEFINE_STATIC_METHOD";
            if let (Some(fqn), Some(nome), Some(ret)) =
                (partes.get(1), partes.get(2), partes.get(3))
            {
                let params = parse_params(if partes.len() > 5 { &partes[5..] } else { &[] });
                let metodo = LibMetodo {
                    nome: nome.to_string(),
                    nome_classe: fqn.to_string(),
                    tipo_retorno: ret.to_string(),
                    parametros: params.clone(),
                    aridade: params.len(),
                    chave_nativa: None,
                    eh_estatica,
                    eh_assincrona: false,
                };
                if let Some(LibSimbolo::Classe(cl)) = biblioteca.simbolos.get_mut(*fqn) {
                    // Não sobrescreve um método nativo já registrado.
                    let deve_inserir = cl
                        .metodos
                        .get(*nome)
                        .map_or(true, |m| m.chave_nativa.is_none());
                    if deve_inserir {
                        cl.metodos.insert(nome.to_string(), metodo);
                    }
                }
            }
        }
        _ => {}
    }
}

fn parse_params(partes: &[&str]) -> Vec<(String, String)> {
    partes
        .iter()
        .filter_map(|p| {
            let mut parts = p.splitn(2, ':');
            if let (Some(tipo), Some(nome)) = (parts.next(), parts.next()) {
                Some((tipo.to_string(), nome.to_string()))
            } else {
                None
            }
        })
        .collect()
}
