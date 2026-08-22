use crate::debug::*;
use crate::nativos::*;
use crate::objetos::*;
use crate::tipos::VM;
use crate::tipos::*;
use crate::util::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub(crate) fn carregar_definicoes(vm: &mut VM) -> Result<(), String> {
    pub(crate) fn limpa_parametro(raw: &str) -> String {
        let mut clean = raw.split(':').last().unwrap_or(raw);
        clean = clean.split('=').next().unwrap_or(clean);
        clean.to_string()
    }

    let mut i = 0;
    while i < vm.bytecode.len() {
        let instrucao = vm.bytecode[i].clone();
        let partes: Vec<&str> = instrucao.split_whitespace().collect();
        let op = partes.get(0).unwrap_or(&"");

        match *op {
            "DEFINE_STATIC_CLASS" => {
                let nome_classe = partes
                    .get(1)
                    .ok_or("DEFINE_STATIC_CLASS requer nome")?
                    .to_string();
                vm.classes.insert(
                    nome_classe.clone(),
                    ClasseInfo {
                        nome: nome_classe.clone(),
                        campos: Vec::new(),
                        metodos: HashMap::new(),
                        campos_estaticos: Rc::new(RefCell::new(HashMap::new())),
                        metodos_estaticos: HashMap::new(),
                        construtor: None,
                        nome_classe_pai: None,
                        construtor_params: Vec::new(),
                        base_construtor_args: Vec::new(),
                        constructor_body: Vec::new(),
                    },
                );
                i += 1;
            }
            "DEFINE_CLASS" => {
                let nome_classe = partes.get(1).ok_or("DEFINE_CLASS requer nome")?.to_string();
                let parent_class = partes.get(2).map(|s| s.to_string());
                let parent_class = if parent_class.as_deref() == Some("NULO") {
                    None
                } else {
                    parent_class
                };
                let props_and_constructor_str = partes
                    .get(3)
                    .ok_or("DEFINE_CLASS requer propriedades e parâmetros do construtor")?;
                let parts: Vec<&str> = props_and_constructor_str.split('|').collect();
                // A partir de agora, o compilador gera listas separadas por vírgula para evitar confusão com split_whitespace.
                let campos: Vec<String> = parts.get(0).map_or(Vec::new(), |s| {
                    s.split(',')
                        .filter(|p| !p.is_empty())
                        .map(String::from)
                        .collect()
                });
                let construtor_params: Vec<String> = parts.get(1).map_or(Vec::new(), |s| {
                    s.split(',')
                        .filter(|p| !p.is_empty())
                        .map(String::from)
                        .collect()
                });
                let base_construtor_args: Vec<String> = parts.get(2).map_or(Vec::new(), |s| {
                    s.split(',')
                        .filter(|p| !p.is_empty())
                        .map(String::from)
                        .collect()
                });
                let constructor_body: Vec<String> = parts.get(3).map_or(Vec::new(), |s| {
                    s.split(';')
                        .filter(|line| !line.trim().is_empty())
                        .map(String::from)
                        .collect()
                });

                let all_campos = if let Some(parent_name) = &parent_class {
                    if let Some(parent_info) = vm.classes.get(parent_name) {
                        let mut inherited_campos = parent_info.campos.clone();
                        inherited_campos.extend(campos);
                        inherited_campos
                    } else {
                        campos
                    }
                } else {
                    campos
                };

                vm.classes.insert(
                    nome_classe.clone(),
                    ClasseInfo {
                        nome: nome_classe.clone(),
                        campos: all_campos,
                        metodos: HashMap::new(),
                        campos_estaticos: Rc::new(RefCell::new(HashMap::new())),
                        metodos_estaticos: HashMap::new(),
                        construtor: None,
                        nome_classe_pai: parent_class,
                        construtor_params,
                        base_construtor_args,
                        constructor_body,
                    },
                );
                i += 1;
            }
            "DEFINE_FUNCTION" => {
                let nome_func = partes
                    .get(1)
                    .ok_or("DEFINE_FUNCTION requer nome")?
                    .to_string();
                let tamanho: usize = partes
                    .get(2)
                    .ok_or("DEFINE_FUNCTION requer tamanho")?
                    .parse()
                    .map_err(|_| "Tamanho inválido")?;
                let parametros: Vec<String> =
                    partes.iter().skip(3).map(|s| limpa_parametro(s)).collect();
                let corpo_inicio = i + 1;
                let corpo_fim = corpo_inicio + tamanho;
                if corpo_fim > vm.bytecode.len() {
                    return Err("Bytecode truncado em DEFINE_FUNCTION".into());
                }
                let corpo = vm.bytecode[corpo_inicio..corpo_fim].to_vec();
                vm.functions.insert(
                    nome_func.clone(),
                    FuncInfo {
                        nome: nome_func,
                        parametros,
                        corpo,
                    },
                );
                i = corpo_fim;
            }
            "DEFINE_METHOD" => {
                let classe_nome = partes
                    .get(1)
                    .ok_or("DEFINE_METHOD requer classe")?
                    .to_string();
                let metodo_nome = partes
                    .get(2)
                    .ok_or("DEFINE_METHOD requer nome")?
                    .to_string();
                let _tipo_retorno = partes.get(3).unwrap_or(&"vazio");
                let tamanho: usize = partes
                    .get(4)
                    .ok_or("DEFINE_METHOD requer tamanho")?
                    .parse()
                    .map_err(|_| "Tamanho inválido")?;
                let parametros: Vec<String> =
                    partes.iter().skip(5).map(|s| limpa_parametro(s)).collect();
                let corpo_inicio = i + 1;
                let corpo_fim = corpo_inicio + tamanho;
                if corpo_fim > vm.bytecode.len() {
                    return Err("Bytecode truncado em DEFINE_METHOD".into());
                }
                let corpo = vm.bytecode[corpo_inicio..corpo_fim].to_vec();
                let metodo_info = FuncInfo {
                    nome: metodo_nome.clone(),
                    parametros,
                    corpo,
                };
                let entry = vm.classes.entry(classe_nome.clone()).or_insert(ClasseInfo {
                    nome: classe_nome.clone(),
                    campos: Vec::new(),
                    metodos: HashMap::new(),
                    campos_estaticos: Rc::new(RefCell::new(HashMap::new())),
                    metodos_estaticos: HashMap::new(),
                    construtor: None,
                    nome_classe_pai: None,
                    construtor_params: Vec::new(),
                    base_construtor_args: Vec::new(), // Added
                    constructor_body: Vec::new(),     // Added
                });
                if metodo_nome == "construtor" {
                    if let Some(existing) = entry.metodos.get("construtor") {
                        if existing.parametros.len() >= metodo_info.parametros.len() {
                            // Mantém o existente (mais completo ou igual)
                        } else {
                            entry.metodos.insert(metodo_nome, metodo_info);
                        }
                    } else {
                        entry.metodos.insert(metodo_nome, metodo_info);
                    }
                } else {
                    entry.metodos.insert(metodo_nome, metodo_info);
                }
                i = corpo_fim;
            }
            "DEFINE_STATIC_METHOD" => {
                let classe_nome = partes
                    .get(1)
                    .ok_or("DEFINE_STATIC_METHOD requer classe")?
                    .to_string();
                let metodo_nome = partes
                    .get(2)
                    .ok_or("DEFINE_STATIC_METHOD requer nome")?
                    .to_string();
                let _tipo_retorno = partes.get(3).unwrap_or(&"vazio");
                let tamanho: usize = partes
                    .get(4)
                    .ok_or("DEFINE_STATIC_METHOD requer tamanho")?
                    .parse()
                    .map_err(|_| "Tamanho inválido")?;
                let parametros: Vec<String> =
                    partes.iter().skip(5).map(|s| limpa_parametro(s)).collect();
                let corpo_inicio = i + 1;
                let corpo_fim = corpo_inicio + tamanho;
                if corpo_fim > vm.bytecode.len() {
                    return Err("Bytecode truncado em DEFINE_STATIC_METHOD".into());
                }
                let corpo = vm.bytecode[corpo_inicio..corpo_fim].to_vec();
                let metodo_info = FuncInfo {
                    nome: metodo_nome.clone(),
                    parametros,
                    corpo,
                };
                let entry = vm.classes.entry(classe_nome.clone()).or_insert(ClasseInfo {
                    nome: classe_nome.clone(),
                    campos: Vec::new(),
                    metodos: HashMap::new(),
                    campos_estaticos: Rc::new(RefCell::new(HashMap::new())),
                    metodos_estaticos: HashMap::new(),
                    construtor: None,
                    nome_classe_pai: None,
                    construtor_params: Vec::new(),
                    base_construtor_args: Vec::new(), // Added
                    constructor_body: Vec::new(),     // Added
                });
                entry.metodos_estaticos.insert(metodo_nome, metodo_info);
                i = corpo_fim;
            }
            _ => {
                i += 1;
            }
        }
    }
    Ok(())
}

pub(crate) fn parse_definicao_funcao(
    vm: &VM,
    start_index: usize,
) -> Result<(FuncInfo, usize), String> {
    let def_line = vm.bytecode[start_index].clone();
    let partes: Vec<&str> = def_line.split_whitespace().collect();
    if partes.len() < 4 {
        return Err(format!(
            "Instrução DEFINE_FUNCTION malformada: {}",
            def_line
        ));
    }
    let nome = partes[1].to_string();
    let parametros: Vec<String> = partes[3]
        .split(',')
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect();

    let mut corpo = Vec::new();
    let mut i = start_index + 1;
    while i < vm.bytecode.len() && !vm.bytecode[i].starts_with("END_FUNCTION") {
        corpo.push(vm.bytecode[i].clone());
        i += 1;
    }

    let func_info = FuncInfo {
        nome,
        parametros,
        corpo,
    };
    Ok((func_info, i - start_index))
}
