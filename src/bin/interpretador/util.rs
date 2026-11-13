use crate::tipos::*;
use std::fs;
use std::io::{Read, Write};

pub(crate) fn ler_bytecode(caminho_arquivo: &str) -> Result<Vec<String>, std::io::Error> {
    let mut arquivo = fs::File::open(caminho_arquivo)?;
    let mut conteudo = String::new();
    arquivo.read_to_string(&mut conteudo)?;
    let bytecode_linhas: Vec<String> = conteudo
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(String::from)
        .collect();
    Ok(bytecode_linhas)
}

/// Extrai o conteúdo de uma instrução `LOAD_CONST_STR "..."` da linha de bytecode
/// e resolve sequências de escape. Preserva espaços internos.
pub(crate) fn parse_load_const_str(instrucao: &str) -> String {
    let resto = instrucao
        .strip_prefix("LOAD_CONST_STR")
        .unwrap_or(instrucao)
        .trim_start();
    // Remove o par de aspas externo, se presente
    let miolo = if resto.starts_with('"') && resto.ends_with('"') && resto.len() >= 2 {
        &resto[1..resto.len() - 1]
    } else {
        resto
    };
    desescapar(miolo)
}

/// Resolve sequências de escape comuns; escapes desconhecidos são preservados.
pub(crate) fn desescapar(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some('0') => out.push('\0'),
            Some('"') => out.push('"'),
            Some('\'') => out.push('\''),
            Some('\\') => out.push('\\'),
            Some(outro) => {
                out.push('\\');
                out.push(outro);
            }
            None => out.push('\\'),
        }
    }
    out
}
