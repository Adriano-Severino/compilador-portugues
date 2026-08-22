use crate::carregador::*;
use crate::nativos::*;
use crate::objetos::*;
use crate::tipos::VM;
use crate::tipos::*;
use crate::util::*;
use std::collections::HashMap;
use std::collections::HashSet;

use std::io::{self, Write};
pub(crate) fn debug_pause_if_needed(vm: &mut VM, instr: &str) -> Result<(), String> {
    let Some(dbg_rc) = vm.debug.clone() else {
        return Ok(());
    };
    let mut st = dbg_rc.borrow_mut();
    if !st.enabled {
        return Ok(());
    }

    let mut should_pause = matches!(st.step_mode, Some(StepMode::StepInto));

    // Step Over: pausa quando voltar à mesma profundidade
    if !should_pause && matches!(st.step_mode, Some(StepMode::StepOver)) {
        if st.call_depth == 0 {
            should_pause = true;
        }
    }

    // Step Out: pausa quando profundidade diminui
    if !should_pause && matches!(st.step_mode, Some(StepMode::StepOut)) {
        // Será tratado ao sair de função
    }

    if !should_pause {
        if let Some(bps) = st.breakpoints.get(&vm.code_id) {
            // Para instruções não-JUMP, ip já foi incrementado no loop run
            let cur_ip = vm.ip.saturating_sub(1);
            if bps.contains(&cur_ip) {
                should_pause = true;
            }
        }
    }
    if !should_pause {
        return Ok(());
    }

    st.last_break_location = Some((vm.code_id.clone(), vm.ip.saturating_sub(1)));
    drop(st);

    loop {
        println!(
            "\n[depurador] {}@ip={} -> {}\ncomandos: c(continue), s(step into), so(step over), sr(step out), p(pause), p(pilha), vars, v <nome>, dis [n], bp add|del <ip>|list, bp add|del <code_id> <ip>, bp list [code_id], where, help, q(quit)",
            vm.code_id, vm.ip.saturating_sub(1), instr
        );
        print!("dbg> ");
        io::stdout().flush().ok();
        let mut entrada = String::new();
        io::stdin()
            .read_line(&mut entrada)
            .map_err(|e| e.to_string())?;
        let cmd = entrada.trim();
        if cmd.is_empty() || cmd == "c" || cmd == "cont" || cmd == "continue" {
            if let Some(d) = &vm.debug {
                d.borrow_mut().step_mode = None;
            }
            break;
        } else if cmd == "s" || cmd == "step" || cmd == "next" || cmd == "n" {
            if let Some(d) = &vm.debug {
                d.borrow_mut().step_mode = Some(StepMode::StepInto);
            }
            break;
        } else if cmd == "so" || cmd == "stepover" {
            if let Some(d) = &vm.debug {
                d.borrow_mut().step_mode = Some(StepMode::StepOver);
            }
            break;
        } else if cmd == "sr" || cmd == "stepout" {
            if let Some(d) = &vm.debug {
                d.borrow_mut().step_mode = Some(StepMode::StepOut);
            }
            break;
        } else if cmd == "p" || cmd == "pause" {
            // Pausa a execução - para o loop e mantém step_mode para continuar pausado
            if let Some(d) = &vm.debug {
                d.borrow_mut().step_mode = Some(StepMode::StepInto);
            }
            // Não break - continua no loop de debug
        } else if cmd == "q" || cmd == "quit" {
            if let Some(d) = &vm.debug {
                d.borrow_mut().enabled = false;
            }
            return Err("Debugging terminado pelo usuário".into());
        } else if cmd == "p" || cmd == "pilha" {
            println!("pilha ({} itens):", vm.pilha.len());
            for (i, v) in vm.pilha.iter().enumerate() {
                println!("  [{}] {}", i, v);
            }
        } else if cmd == "vars" {
            println!("variaveis ({}):", vm.variaveis.len());
            for (k, v) in &vm.variaveis {
                println!("  {} = {}", k, v);
            }
        } else if cmd == "varsjson" {
            // Mostrar variáveis em formato JSON para DAP adapter
            let vars_json: serde_json::Value = vm
                .variaveis
                .iter()
                .map(|(k, v)| {
                    serde_json::json!({
                        "name": k,
                        "value": v.to_string(),
                        "type": match v {
                            Valor::Inteiro(_) => "inteiro",
                            Valor::Texto(_) => "texto",
                            Valor::Booleano(_) => "booleano",
                            Valor::Flutuante(_) => "flutuante",
                            Valor::Duplo(_) => "duplo",
                            Valor::Decimal(_) => "decimal",
                            Valor::Nulo => "nulo",
                            Valor::Array(_) => "array",
                            Valor::Objeto { .. } => "objeto",
                            _ => "desconhecido"
                        }
                    })
                })
                .collect();
            println!(
                "{}",
                serde_json::to_string(&vars_json).unwrap_or_else(|_| "[]".to_string())
            );
        } else if cmd.starts_with("v ") {
            let nome = cmd.splitn(2, ' ').nth(1).unwrap_or("");
            if let Some(v) = vm.variaveis.get(nome) {
                println!("{} = {}", nome, v);
            } else {
                println!("(sem variável '{}')", nome);
            }
        } else if cmd == "stack" || cmd == "where" {
            // Mostrar call stack em formato JSON para DAP adapter
            let frames: Vec<serde_json::Value> = vm
                .call_stack
                .iter()
                .map(|frame| {
                    serde_json::json!({
                        "code_id": frame.code_id,
                        "ip": frame.ip,
                        "vars": frame.variaveis.len()
                    })
                })
                .collect();

            // Adicionar frame atual
            let current_frame = serde_json::json!({
                "code_id": vm.code_id,
                "ip": vm.ip.saturating_sub(1),
                "vars": vm.variaveis.len()
            });

            let all_frames: Vec<serde_json::Value> = frames
                .into_iter()
                .chain(std::iter::once(current_frame))
                .collect();
            println!(
                "{}",
                serde_json::to_string(&all_frames).unwrap_or_else(|_| "[]".to_string())
            );
        } else if cmd.starts_with("dis") {
            let parts: Vec<&str> = cmd.split_whitespace().collect();
            let n: usize = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(8);
            let start = vm.ip.saturating_sub(1);
            let end = (start + n).min(vm.bytecode.len());
            for i in start..end {
                let mark = if i + 1 == vm.ip { "->" } else { "  " };
                println!("{} {:04}: {}", mark, i, vm.bytecode[i]);
            }
        } else if cmd.starts_with("bp ") {
            let parts: Vec<&str> = cmd.split_whitespace().collect();
            if parts.len() >= 2 {
                match parts[1] {
                    // bp add <ip>
                    "add" if parts.len() == 3 => {
                        if let Ok(ip) = parts[2].parse::<usize>() {
                            if let Some(d) = &vm.debug { let mut s = d.borrow_mut(); let set = s.breakpoints.entry(vm.code_id.clone()).or_insert_with(HashSet::new); set.insert(ip); }
                            println!("Breakpoint adicionado em {}:{}", vm.code_id, ip);
                        } else { println!("ip inválido"); }
                    }
                    // bp add <code_id> <ip>
                    "add" if parts.len() >= 4 => {
                        let code_id = parts[2].to_string();
                        if let Ok(ip) = parts[3].parse::<usize>() {
                            if let Some(d) = &vm.debug { let mut s = d.borrow_mut(); let set = s.breakpoints.entry(code_id.clone()).or_insert_with(HashSet::new); set.insert(ip); }
                            println!("Breakpoint adicionado em {}:{}", code_id, ip);
                        } else { println!("ip inválido"); }
                    }
                    // bp del <ip>
                    "del" if parts.len() == 3 => {
                        if let Ok(ip) = parts[2].parse::<usize>() {
                            if let Some(d) = &vm.debug { let mut s = d.borrow_mut(); if let Some(set) = s.breakpoints.get_mut(&vm.code_id) { set.remove(&ip); } }
                            println!("Breakpoint removido em {}:{}", vm.code_id, ip);
                        } else { println!("ip inválido"); }
                    }
                    // bp del <code_id> <ip>
                    "del" if parts.len() >= 4 => {
                        let code_id = parts[2].to_string();
                        if let Ok(ip) = parts[3].parse::<usize>() {
                            if let Some(d) = &vm.debug { let mut s = d.borrow_mut(); if let Some(set) = s.breakpoints.get_mut(&code_id) { set.remove(&ip); } }
                            println!("Breakpoint removido em {}:{}", code_id, ip);
                        } else { println!("ip inválido"); }
                    }
                    // bp list [code_id]
                    "list" => {
                        let target = if parts.len() >= 3 { parts[2] } else { &vm.code_id };
                        if let Some(d) = &vm.debug { let s = d.borrow(); if let Some(set) = s.breakpoints.get(target) { println!("breakpoints em {}: {:?}", target, set); } else { println!("sem breakpoints em {}", target); } }
                    }
                    _ => println!("uso: bp add <ip> | bp add <code_id> <ip> | bp del <ip> | bp del <code_id> <ip> | bp list [code_id]"),
                }
            } else {
                println!("uso: bp add <ip> | bp add <code_id> <ip> | bp del <ip> | bp del <code_id> <ip> | bp list [code_id]");
            }
        } else if cmd == "where" {
            println!(
                "em {} ip={} -> {}",
                vm.code_id,
                vm.ip.saturating_sub(1),
                instr
            );
        } else if cmd == "help" || cmd == "?" {
            println!("comandos: c, s, p, vars, v <nome>, dis [n], bp add|del <ip>|list, bp add|del <code_id> <ip>, bp list [code_id], where, help, q");
        } else if cmd == "q" || cmd == "quit" || cmd == "exit" {
            return Err("Execução abortada pelo usuário".to_string());
        } else {
            println!("comando desconhecido. digite 'help'.");
        }
    }
    Ok(())
}
