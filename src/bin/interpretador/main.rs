pub mod carregador;
pub mod debug;
pub mod nativos;
pub mod objetos;
pub mod tipos;
pub mod util;
pub mod vm;

use tipos::*;
use util::*;
use vm::*;

use rust_decimal::Decimal;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::env;
use std::fmt;
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[cfg(feature = "jit")]
use compilador_portugues::jit::CraneliftJit;

// Usa tokio::main para inicializar o runtime multi-thread antes de qualquer código async.
#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let usar_jit = args.iter().any(|a| a == "--jit");

    // Quando --jit for passado, faça um autoteste simples do JIT para confirmar que está funcional.
    #[cfg(feature = "jit")]
    if usar_jit {
        if let Ok(mut jit) = CraneliftJit::new() {
            if let Ok(handle) = jit.compilar_soma_i32() {
                let r = unsafe { jit.chamar_soma_i32(&handle, 2, 40) };
                eprintln!("[JIT] autoteste soma_i32(2,40) = {}", r);
            }
        }
    }

    if args.len() < 2 {
        eprintln!(
            "Uso: {} <arquivo.pbc> [--executar-funcao <nome_da_funcao_completo>]",
            args[0]
        );
        return Err("Argumento inválido".into());
    }

    let caminho_arquivo = &args[1];
    let mut function_to_execute: Option<String> = None;
    let mut usar_debug = false;

    let mut i = 2;
    while i < args.len() {
        if args[i] == "--executar-funcao" {
            if i + 1 < args.len() {
                function_to_execute = Some(args[i + 1].clone());
                i += 2;
            } else {
                return Err("Argumento --executar-funcao requer um nome de função".into());
            }
        } else if args[i] == "--debug" {
            usar_debug = true;
            i += 1;
        } else {
            i += 1;
        }
    }
    let bytecode = ler_bytecode(caminho_arquivo)?;
    if bytecode.is_empty() {
        return Err("Arquivo de bytecode vazio".into());
    }

    //Obter o diretório base do arquivo de bytecode.
    let mut path = std::path::PathBuf::from(caminho_arquivo);
    path.pop(); // Remove o nome do arquivo, deixando o diretório.
    let base_dir = if path.as_os_str().is_empty() {
        std::path::PathBuf::from(".")
    } else {
        path
    };

    let mut vm = VM::new(bytecode, base_dir);
    if usar_debug {
        let dbg = DebugState {
            enabled: true,
            breakpoints: HashMap::new(),
            step_mode: Some(StepMode::StepInto),
            last_break_location: None,
            call_depth: 0,
        };
        vm.debug = Some(Rc::new(RefCell::new(dbg)));
    }

    // Carregar definições (classes, funções)
    if let Err(e) = crate::carregador::carregar_definicoes(&mut vm) {
        eprintln!("Erro ao carregar definições: {}", e);
        return Err(e.into());
    }

    // Fase 2: Executar inicializadores de propriedades estáticas
    if let Err(e) = vm.run_apenas_inicializadores() {
        eprintln!("Erro em inicializadores: {}", e);
        return Err(e.into());
    }

    // Fase 3: Executar código global (funções main, etc.)
    if let Err(e) = vm.executar_codigo_global().await {
        eprintln!("Erro ao executar código de inicialização: {}", e);
        return Err(e.into());
    }

    // Fase 4: Encontrar e executar a função especificada ou 'Principal'
    let func_to_run = if let Some(func_name) = function_to_execute {
        Some(func_name)
    } else {
        vm.functions
            .keys()
            .find(|nome| {
                nome.ends_with("Principal")
                    || nome == &&"Principal".to_string()
                    || nome == &&"principal".to_string()
            })
            .cloned()
    };

    if let Some(nome_funcao) = func_to_run {
        let func_info = vm
            .functions
            .get(&nome_funcao)
            .ok_or_else(|| format!("Função \"{}\" não encontrada para execução.", nome_funcao))?
            .clone();

        let mut main_vm = VM {
            pilha: Vec::new(),
            variaveis: HashMap::new(),
            bytecode: func_info.corpo.clone(),
            ip: 0,
            classes: vm.classes.clone(),
            functions: vm.functions.clone(),
            loaded_modules: vm.loaded_modules.clone(),
            base_dir: vm.base_dir.clone(),
            debug: vm.debug.clone(),
            code_id: format!("main:{}", nome_funcao),
            // A VM principal herda o gerenciador de tasks compartilhado
            task_counter: vm.task_counter.clone(),
            tasks: vm.tasks.clone(),
            call_stack: Vec::new(),
        };

        if let Err(e) = main_vm.run().await {
            eprintln!("❌ Erro na execução da função {}: {}", nome_funcao, e);
            return Err(e.into());
        }
    }

    Ok(())
}

//Função auxiliar para ler o bytecode do arquivo.
