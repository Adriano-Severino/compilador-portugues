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
