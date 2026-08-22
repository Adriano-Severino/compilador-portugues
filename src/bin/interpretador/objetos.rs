use crate::carregador::*;
use crate::debug::*;
use crate::nativos::*;
use crate::tipos::VM;
use crate::tipos::*;
use crate::util::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub(crate) fn eh_classe_stdlib(nome_classe: &str) -> bool {
    nome_classe.starts_with("Sistema.") || nome_classe == "ClienteHttp" || nome_classe == "Sistema"
}

pub(crate) async fn criar_objeto(
    vm: &mut VM,
    nome_classe: &str,
    argumentos: Vec<Valor>,
) -> Result<Valor, String> {
    // --- Intercepta instâncias de classes da biblioteca padrão (igual ao CLR do C#) ---
    if eh_classe_stdlib(nome_classe) {
        // Resolve o nome qualificado completo para classes sem namespace explícito
        let fqn = if nome_classe.contains('.') {
            nome_classe.to_string()
        } else {
            // Tenta encontrar por nome curto nos namespaces conhecidos
            let candidatos = [
                format!("Sistema.Rede.{}", nome_classe),
                format!("Sistema.IO.{}", nome_classe),
                format!("Sistema.Colecoes.{}", nome_classe),
                format!("Sistema.Data.{}", nome_classe),
                nome_classe.to_string(),
            ];
            candidatos.into_iter().next().unwrap_or_default()
        };
        // Cria um objeto stub com o estado interno necessário
        let mut campos_map = HashMap::new();
        campos_map.insert("__fqn__".to_string(), Valor::Texto(fqn.clone()));
        // Estado específico por tipo
        match fqn.as_str() {
            "Sistema.Rede.ClienteHttp" | "ClienteHttp" => {
                campos_map.insert(
                    "__tipo__".to_string(),
                    Valor::Texto("ClienteHttp".to_string()),
                );
            }
            "Sistema.Colecoes.Lista" => {
                campos_map.insert("__itens__".to_string(), Valor::Array(Vec::new()));
            }
            "Sistema.Colecoes.Dicionario" => {
                campos_map.insert("__itens__".to_string(), Valor::Array(Vec::new()));
            }
            "Sistema.Data.Data" => {
                let agora = chrono_or_default_date();
                campos_map.insert("__data__".to_string(), Valor::Texto(agora));
            }
            _ => {}
        }
        return Ok(Valor::Objeto {
            nome_classe: fqn,
            campos: Rc::new(RefCell::new(campos_map)),
            metodos: HashMap::new(),
        });
    }

    let classe_info = vm
        .classes
        .get(nome_classe)
        .ok_or_else(|| format!("Classe \"{}\" não encontrada", nome_classe))?
        .clone();

    let mut campos_map = HashMap::new();

    // Adiciona os campos da classe atual, inicializando com Nulo.
    for campo_nome in &classe_info.campos {
        if !campos_map.contains_key(campo_nome) {
            campos_map.insert(campo_nome.clone(), Valor::Nulo);
        }
    }

    let objeto_rc = Rc::new(RefCell::new(campos_map));
    let objeto = Valor::Objeto {
        nome_classe: nome_classe.to_string(),
        campos: objeto_rc.clone(),
        metodos: classe_info.metodos.clone(),
    };

    // Se houver um construtor, executa-o.
    if let Some(constructor_info) = classe_info.metodos.get("construtor").cloned() {
        let mut constructor_vm = VM {
            pilha: Vec::new(),
            variaveis: HashMap::new(), // Começa com escopo limpo
            bytecode: constructor_info.corpo.clone(),
            ip: 0,
            classes: vm.classes.clone(),
            functions: vm.functions.clone(),
            loaded_modules: vm.loaded_modules.clone(),
            base_dir: vm.base_dir.clone(),
            debug: vm.debug.clone(),
            code_id: format!("ctor:{}", nome_classe),
            task_counter: vm.task_counter.clone(),
            tasks: vm.tasks.clone(),
            call_stack: Vec::new(),
        };

        // Adiciona 'este' e os argumentos ao escopo do construtor.
        constructor_vm
            .variaveis
            .insert("este".to_string(), objeto.clone());
        for (i, param_name) in constructor_info.parametros.iter().enumerate() {
            if let Some(arg_val) = argumentos.get(i) {
                constructor_vm
                    .variaveis
                    .insert(param_name.clone(), arg_val.clone());
            }
        }

        Box::pin(constructor_vm.run()).await?;
    }

    Ok(objeto)
}

pub(crate) async fn chamar_metodo(
    vm: &mut VM,
    objeto: &mut Valor,
    nome_metodo: &str,
    argumentos: Vec<Valor>,
) -> Result<Valor, String> {
    if let Valor::Texto(s) = objeto {
        match nome_metodo {
            "comprimento" => return Ok(Valor::Inteiro(s.len() as i64)),
            "ParaMaiusculo" => return Ok(Valor::Texto(s.to_uppercase())),
            "ParaMinusculo" => return Ok(Valor::Texto(s.to_lowercase())),
            "Aparar" => return Ok(Valor::Texto(s.trim().to_string())),
            "Contem" => {
                let busca = argumentos
                    .first()
                    .map(|v| v.to_string())
                    .unwrap_or_default();
                return Ok(Valor::Booleano(s.contains(busca.as_str())));
            }
            "Substituir" => {
                let de = argumentos
                    .first()
                    .map(|v| v.to_string())
                    .unwrap_or_default();
                let para = argumentos.get(1).map(|v| v.to_string()).unwrap_or_default();
                return Ok(Valor::Texto(s.replace(de.as_str(), para.as_str())));
            }
            _ => {}
        }
    }

    // --- Intercepta métodos de instâncias da biblioteca padrão ---
    if let Valor::Objeto {
        ref nome_classe,
        ref campos,
        ..
    } = objeto.clone()
    {
        if eh_classe_stdlib(nome_classe) {
            let fqn = nome_classe.clone();
            match fqn.as_str() {
                // Lista nativa (Sistema.Colecoes.Lista)
                "Sistema.Colecoes.Lista" => {
                    let mut campos_ref = campos.borrow_mut();
                    match nome_metodo {
                        "Adicionar" => {
                            let item = argumentos.into_iter().next().unwrap_or(Valor::Nulo);
                            if let Some(Valor::Array(ref mut v)) = campos_ref.get_mut("__itens__") {
                                v.push(item);
                            }
                            return Ok(Valor::Nulo);
                        }
                        "Obter" => {
                            let idx = argumentos
                                .first()
                                .and_then(|v| {
                                    if let Valor::Inteiro(i) = v {
                                        Some(*i as usize)
                                    } else {
                                        None
                                    }
                                })
                                .unwrap_or(0);
                            if let Some(Valor::Array(ref v)) = campos_ref.get("__itens__") {
                                return Ok(v.get(idx).cloned().unwrap_or(Valor::Nulo));
                            }
                            return Ok(Valor::Nulo);
                        }
                        "Tamanho" | "Contagem" => {
                            if let Some(Valor::Array(ref v)) = campos_ref.get("__itens__") {
                                return Ok(Valor::Inteiro(v.len() as i64));
                            }
                            return Ok(Valor::Inteiro(0));
                        }
                        "Remover" | "RemoverEm" => {
                            let idx = argumentos
                                .first()
                                .and_then(|v| {
                                    if let Valor::Inteiro(i) = v {
                                        Some(*i as usize)
                                    } else {
                                        None
                                    }
                                })
                                .unwrap_or(0);
                            if let Some(Valor::Array(ref mut v)) = campos_ref.get_mut("__itens__") {
                                if idx < v.len() {
                                    v.remove(idx);
                                }
                            }
                            return Ok(Valor::Nulo);
                        }
                        "Limpar" => {
                            if let Some(Valor::Array(ref mut v)) = campos_ref.get_mut("__itens__") {
                                v.clear();
                            }
                            return Ok(Valor::Nulo);
                        }
                        "Contem" => {
                            let item = argumentos.first().cloned().unwrap_or(Valor::Nulo);
                            if let Some(Valor::Array(ref v)) = campos_ref.get("__itens__") {
                                return Ok(Valor::Booleano(v.contains(&item)));
                            }
                            return Ok(Valor::Booleano(false));
                        }
                        _ => return Ok(Valor::Nulo),
                    }
                }
                // ClienteHttp nativa (stub assíncrono)
                "Sistema.Rede.ClienteHttp" | "ClienteHttp" => match nome_metodo {
                    "ObterAsync" | "Obter" => {
                        eprintln!(
                            "[aviso] ClienteHttp.{} não implementado neste ambiente",
                            nome_metodo
                        );
                        return Ok(Valor::Texto(String::new()));
                    }
                    "EnviarAsync" | "Enviar" => {
                        eprintln!(
                            "[aviso] ClienteHttp.{} não implementado neste ambiente",
                            nome_metodo
                        );
                        return Ok(Valor::Nulo);
                    }
                    _ => return Ok(Valor::Nulo),
                },
                // Fallback para classes stdlib sem handler específico
                _ => {
                    eprintln!(
                        "[aviso stdlib] Método '{}' em '{}' não implementado; retornando nulo",
                        nome_metodo, fqn
                    );
                    return Ok(Valor::Nulo);
                }
            }
        }
    }

    if let Valor::Objeto {
        ref nome_classe, ..
    } = objeto
    {
        // Tenta encontrar o método na classe atual ou em suas classes pai
        let mut current_class_name = Some(nome_classe.clone());
        let mut metodo_info: Option<FuncInfo> = None;

        while let Some(c_name) = current_class_name.clone() {
            if let Some(class_info) = vm.classes.get(&c_name) {
                if let Some(m_info) = class_info.metodos.get(nome_metodo) {
                    metodo_info = Some(m_info.clone());
                    break;
                }
                current_class_name = class_info.nome_classe_pai.clone();
            } else {
                break;
            }
        }

        if let Some(metodo_info) = metodo_info {
            // --- Prepara o ambiente do método ---
            let mut vars = HashMap::new();

            // 1. Adiciona "este" ao escopo local, compartilhando o Rc para os campos.
            vars.insert("este".to_string(), objeto.clone());

            // 2. Adiciona os argumentos do método ao escopo local.
            for (i, param_nome) in metodo_info.parametros.iter().enumerate() {
                let valor_arg = argumentos.get(i).cloned().unwrap_or(Valor::Nulo);
                vars.insert(param_nome.clone(), valor_arg);
            }

            // --- Executa o método ---
            let mut vm_metodo = VM {
                pilha: Vec::new(),
                variaveis: vars,
                bytecode: metodo_info.corpo.clone(),
                ip: 0,
                classes: vm.classes.clone(),
                functions: vm.functions.clone(),
                loaded_modules: vm.loaded_modules.clone(),
                base_dir: vm.base_dir.clone(),
                debug: vm.debug.clone(),
                code_id: format!("method:{}::{}", nome_classe, nome_metodo),
                task_counter: vm.task_counter.clone(),
                tasks: vm.tasks.clone(),
                call_stack: Vec::new(),
            };

            Box::pin(vm_metodo.run()).await?;

            // Pega o valor de retorno da pilha da VM do método
            let valor_retorno = vm_metodo.pilha.pop().unwrap_or(Valor::Nulo);
            Ok(valor_retorno)
        } else {
            Err(format!(
                "Método \"'{}.{}'\" não encontrado",
                nome_classe, nome_metodo
            ))
        }
    } else {
        Err("Tentativa de chamar método em não-objeto".to_string())
    }
}

pub(crate) async fn chamar_metodo_estatico(
    vm: &mut VM,
    nome_classe: &str,
    nome_metodo: &str,
    argumentos: Vec<Valor>,
) -> Result<Valor, String> {
    // --- Intercepta chamadas para a biblioteca padrão ---
    if nome_classe == "Sistema.Console" {
        match nome_metodo {
            "EscreverLinha" => {
                let mut texto = String::new();
                for arg in argumentos {
                    if let Valor::Texto(s) = arg {
                        texto.push_str(&s);
                    } else {
                        texto.push_str(&arg.to_string());
                    }
                }
                println!("{}", texto);
                return Ok(Valor::Nulo);
            }
            "Escrever" => {
                let mut texto = String::new();
                for arg in argumentos {
                    if let Valor::Texto(s) = arg {
                        texto.push_str(&s);
                    } else {
                        texto.push_str(&arg.to_string());
                    }
                }
                use std::io::Write;
                print!("{}", texto);
                let _ = std::io::stdout().flush();
                return Ok(Valor::Nulo);
            }
            "LerLinha" => {
                let mut entrada = String::new();
                std::io::stdin()
                    .read_line(&mut entrada)
                    .map_err(|e| format!("Erro ao ler entrada: {}", e))?;
                return Ok(Valor::Texto(
                    entrada.trim_end_matches(['\r', '\n']).to_string(),
                ));
            }
            _ => {}
        }
    }
    // --- Fallback para bytecode do método estático (classes do usuário) ---
    if let Some(classe_info) = vm.classes.get(nome_classe) {
        if let Some(metodo_info) = classe_info.metodos_estaticos.get(nome_metodo) {
            let mut vars = HashMap::new();
            for (i, param_nome) in metodo_info.parametros.iter().enumerate() {
                let valor_arg = argumentos.get(i).cloned().unwrap_or(Valor::Nulo);
                vars.insert(param_nome.clone(), valor_arg);
            }

            let mut vm_metodo = VM {
                pilha: Vec::new(),
                variaveis: vars,
                bytecode: metodo_info.corpo.clone(),
                ip: 0,
                classes: vm.classes.clone(),
                functions: vm.functions.clone(),
                loaded_modules: vm.loaded_modules.clone(),
                base_dir: vm.base_dir.clone(),
                debug: vm.debug.clone(),
                code_id: format!("static:{}::{}", nome_classe, nome_metodo),
                task_counter: vm.task_counter.clone(),
                tasks: vm.tasks.clone(),
                call_stack: Vec::new(),
            };

            Box::pin(vm_metodo.run()).await?;
            return Ok(vm_metodo.pilha.pop().unwrap_or(Valor::Nulo));
        } else if eh_classe_stdlib(nome_classe) {
            // Método estático stdlib sem handler específico: retorna nulo com aviso
            eprintln!(
                "[aviso stdlib] Método estático '{}.{}' não implementado; retornando nulo",
                nome_classe, nome_metodo
            );
            return Ok(Valor::Nulo);
        } else {
            return Err(format!(
                "Método estático \"'{}.{}'\" não encontrado",
                nome_classe, nome_metodo
            ));
        }
    } else if eh_classe_stdlib(nome_classe) {
        // Classe stdlib não registrada no bytecode — handler nativo direto
        eprintln!("[aviso stdlib] Método estático '{}.{}' chamado sem definição de classe; retornando nulo", nome_classe, nome_metodo);
        return Ok(Valor::Nulo);
    } else {
        return Err(format!("Classe \"{}\" não encontrada", nome_classe));
    }
}
