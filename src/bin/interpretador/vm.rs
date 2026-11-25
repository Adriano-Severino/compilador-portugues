use crate::carregador::*;
use crate::debug::*;
use crate::nativos::*;
use crate::objetos::*;
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

use crate::tipos::*;
use crate::util::*;

impl VM {
    pub(crate) async fn executar_funcao(
        &mut self,
        func: &FuncInfo,
        args: Vec<Valor>,
        este: Option<Valor>,
    ) -> Result<Option<Valor>, String> {
        // Salvar frame atual no call stack
        let current_frame = StackFrame {
            code_id: self.code_id.clone(),
            ip: self.ip,
            variaveis: self.variaveis.clone(),
        };
        if let Some(d) = &self.debug {
            self.call_stack.push(current_frame);
        }

        // Incrementar profundidade para step over/step out
        if let Some(d) = &self.debug {
            d.borrow_mut().call_depth += 1;
        }

        let mut child = VM {
            pilha: Vec::new(),
            variaveis: HashMap::new(),
            bytecode: func.corpo.clone(),
            ip: 0,
            classes: self.classes.clone(),
            functions: self.functions.clone(),
            loaded_modules: self.loaded_modules.clone(),
            base_dir: self.base_dir.clone(),
            debug: self.debug.clone(),
            code_id: format!("func:{}", func.nome),
            task_counter: self.task_counter.clone(),
            tasks: self.tasks.clone(),
            call_stack: self.call_stack.clone(),
        };

        // Mapear parâmetros
        for (idx, param_name) in func.parametros.iter().enumerate() {
            if let Some(val) = args.get(idx) {
                child.variaveis.insert(param_name.clone(), val.clone());
            }
        }
        if let Some(obj) = este {
            child.variaveis.insert("este".to_string(), obj);
        }

        let result = Box::pin(child.run()).await;

        // Decrementar profundidade ao sair da função
        if let Some(d) = &self.debug {
            d.borrow_mut().call_depth = d.borrow_mut().call_depth.saturating_sub(1);
        }

        // Restaurar frame do call stack
        if let Some(d) = &self.debug {
            if let Some(frame) = self.call_stack.pop() {
                self.code_id = frame.code_id;
                self.ip = frame.ip;
                self.variaveis = frame.variaveis;
            }
        }

        result?;
        Ok(child.pilha.pop())
    }

    // Cria uma nova instância da VM com o bytecode fornecido.
    pub(crate) fn new(bytecode: Vec<String>, base_dir: std::path::PathBuf) -> Self {
        Self {
            pilha: Vec::new(),
            variaveis: HashMap::new(),
            bytecode,
            ip: 0,
            classes: HashMap::new(),
            functions: HashMap::new(),
            loaded_modules: std::collections::HashSet::new(),
            base_dir,
            debug: None,
            code_id: "global".to_string(),
            // Inicializa o gerenciador de tasks compartilhado
            task_counter: Arc::new(Mutex::new(0)),
            tasks: Arc::new(Mutex::new(HashMap::new())),
            call_stack: Vec::new(),
        }
    }

    // Analisa uma definição de função a partir do bytecode.

    // O laço principal de execução da VM.
    pub(crate) async fn run(&mut self) -> Result<(), String> {
        while self.ip < self.bytecode.len() {
            let instrucao_str = self.bytecode[self.ip].clone();
            // Divide a instrução em partes (ex: "LOAD_CONST_INT", "42")
            let partes: Vec<&str> = instrucao_str.split_whitespace().collect();
            let op = partes.get(0).ok_or("Instrução vazia encontrada")?;

            // Ponto de parada para debug antes de executar a instrução
            crate::debug::debug_pause_if_needed(self, &instrucao_str)?;

            // Avança o ponteiro de instrução ANTES de executar, para evitar laços infinitos.
            // Apenas para JUMP e JUMP_IF_FALSE o IP é ajustado explicitamente.
            if !matches!(*op, "JUMP" | "JUMP_IF_FALSE") {
                self.ip += 1;
            }

            match *op {
                // ... (instruções LOAD_CONST_INT, LOAD_CONST_STR, LOAD_VAR, STORE_VAR, PRINT, CONCAT, HALT)
                "LOAD_CONST_INT" => {
                    let valor = partes
                        .get(1)
                        .ok_or("LOAD_CONST_INT requer um argumento")?
                        .parse::<i64>()
                        .map_err(|e| format!("Valor inválido para LOAD_CONST_INT: {}", e))?;
                    self.pilha.push(Valor::Inteiro(valor));
                }
                "LOAD_CONST_FLOAT" => {
                    let valor = partes
                        .get(1)
                        .ok_or("LOAD_CONST_FLOAT requer um argumento")?
                        .parse::<f32>()
                        .map_err(|e| format!("Valor inválido para LOAD_CONST_FLOAT: {}", e))?;
                    self.pilha.push(Valor::Flutuante(valor));
                }
                "LOAD_CONST_DOUBLE" => {
                    let valor = partes
                        .get(1)
                        .ok_or("LOAD_CONST_DOUBLE requer um argumento")?
                        .parse::<f64>()
                        .map_err(|e| format!("Valor inválido para LOAD_CONST_DOUBLE: {}", e))?;
                    self.pilha.push(Valor::Duplo(valor));
                }
                "LOAD_CONST_STR" => {
                    let valor = crate::util::parse_load_const_str(&instrucao_str);
                    self.pilha.push(Valor::Texto(valor));
                }
                "LOAD_VAR" => {
                    let nome_var = partes.get(1).ok_or("LOAD_VAR requer um nome de variável")?;
                    let valor = self
                        .variaveis
                        .get(*nome_var)
                        .cloned()
                        // Se não encontrar na pilha local, tenta nos campos de 'este'
                        .or_else(|| {
                            if let Some(Valor::Objeto { campos, .. }) = self.variaveis.get("este") {
                                campos.borrow().get(*nome_var).cloned()
                            } else {
                                None
                            }
                        })
                        // Se ainda não encontrou, verifica se é uma classe conhecida para acesso a estático
                        .or_else(|| {
                            if self.classes.contains_key(*nome_var) {
                                Some(Valor::Texto((*nome_var).to_string()))
                            } else {
                                None
                            }
                        })
                        .unwrap_or(Valor::Nulo);
                    self.pilha.push(valor);
                }
                "STORE_VAR" => {
                    let nome_var = partes
                        .get(1)
                        .ok_or("STORE_VAR requer um nome de variável")?;
                    let valor = self.pilha.pop().ok_or("Pilha vazia em STORE_VAR")?;

                    // Tenta atualizar o campo de um objeto se 'este' existir e tiver o campo.
                    if let Some(Valor::Objeto { campos, .. }) = self.variaveis.get("este") {
                        if campos.borrow().contains_key(*nome_var) {
                            campos.borrow_mut().insert(nome_var.to_string(), valor);
                            continue;
                        }
                    }

                    self.variaveis.insert(nome_var.to_string(), valor);
                }
                "PRINT" => {
                    // Ajuste: evitar falha caso a pilha esteja vazia por algum problema de salto no bytecode.
                    // Em vez de abortar, imprime linha em branco para manter execução.
                    if let Some(valor) = self.pilha.pop() {
                        println!("{}", valor);
                    } else {
                        println!("");
                    }
                }
                "CONCAT" => {
                    let num_operandos = partes
                        .get(1)
                        .ok_or("CONCAT requer um número de operandos")?
                        .parse::<usize>()
                        .map_err(|e| format!("Argumento inválido para CONCAT: {}", e))?;

                    if self.pilha.len() < num_operandos {
                        return Err(format!("Pilha insuficiente para CONCAT {}", num_operandos));
                    }

                    let mut resultado = String::new();
                    // Pega os operandos do topo da pilha.
                    let operandos = self.pilha.split_off(self.pilha.len() - num_operandos);
                    for valor in operandos {
                        resultado.push_str(&valor.to_string());
                    }
                    self.pilha.push(Valor::Texto(resultado));
                }
                "HALT" => {
                    // Para a execução da VM.
                    break;
                }

                "NEW_ARRAY" => {
                    let n = partes
                        .get(1)
                        .ok_or("NEW_ARRAY requer tamanho")?
                        .parse::<usize>()
                        .map_err(|e| format!("Tamanho inválido: {}", e))?;
                    if self.pilha.len() < n {
                        return Err("Pilha insuficiente para NEW_ARRAY".into());
                    }
                    let elems = self.pilha.split_off(self.pilha.len() - n);
                    self.pilha.push(Valor::Array(elems));
                }
                "NEW_ARRAY_OF_TYPE" => {
                    // Aloca array de tamanho N com elementos Nulo.
                    // O operando de tipo é ignorado: o runtime é dinamicamente tipado.
                    let tamanho = self
                        .pilha
                        .pop()
                        .ok_or("Pilha vazia para NEW_ARRAY_OF_TYPE")?;
                    let n = match tamanho {
                        Valor::Inteiro(i) if i >= 0 => i as usize,
                        Valor::Inteiro(i) => {
                            return Err(format!("Tamanho de array negativo: {}", i))
                        }
                        _ => return Err("NEW_ARRAY_OF_TYPE requer tamanho inteiro".into()),
                    };
                    // Limite defensivo contra .pbc malformado solicitando
                    // alocação gigante (DoS por memória).
                    const MAX_ARRAY_SIZE: usize = 1 << 28; // 268M elementos
                    if n > MAX_ARRAY_SIZE {
                        return Err(format!(
                            "Tamanho de array excede o limite máximo: {} > {}",
                            n, MAX_ARRAY_SIZE
                        ));
                    }
                    self.pilha
                        .push(Valor::Array(vec![Valor::Nulo; n]));
                }
                "GET_INDEX" => {
                    let idx = self.pilha.pop().ok_or("Pilha vazia para GET_INDEX idx")?;
                    let arr = self.pilha.pop().ok_or("Pilha vazia para GET_INDEX arr")?;
                    match (arr, idx) {
                        (Valor::Array(v), Valor::Inteiro(i)) => {
                            let i = if i < 0 {
                                return Err("Índice negativo".into());
                            } else {
                                i as usize
                            };
                            let val = v.get(i).cloned().ok_or("Índice fora do intervalo")?;
                            self.pilha.push(val);
                        }
                        _ => return Err("GET_INDEX requer array e inteiro".into()),
                    }
                }
                "SET_INDEX" => {
                    let val = self.pilha.pop().ok_or("Pilha vazia para SET_INDEX val")?;
                    let idx = self.pilha.pop().ok_or("Pilha vazia para SET_INDEX idx")?;
                    let arr = self.pilha.pop().ok_or("Pilha vazia para SET_INDEX arr")?;
                    match (arr, idx) {
                        (Valor::Array(mut v), Valor::Inteiro(i)) => {
                            let i = if i < 0 {
                                return Err("Índice negativo".into());
                            } else {
                                i as usize
                            };
                            if i >= v.len() {
                                return Err("Índice fora do intervalo".into());
                            }
                            v[i] = val;
                            self.pilha.push(Valor::Array(v));
                        }
                        _ => return Err("SET_INDEX requer array e inteiro".into()),
                    }
                }
                "GET_LENGTH" => {
                    let arr = self.pilha.pop().ok_or("Pilha vazia para GET_LENGTH")?;
                    match arr {
                        Valor::Array(v) => self.pilha.push(Valor::Inteiro(v.len() as i64)),
                        Valor::Texto(s) => self.pilha.push(Valor::Inteiro(s.len() as i64)),
                        _ => return Err("GET_LENGTH requer array ou texto".into()),
                    }
                }

                "LOAD_CONST_BOOL" => {
                    let valor = partes
                        .get(1)
                        .ok_or("LOAD_CONST_BOOL requer um argumento")?
                        .parse::<bool>()
                        .map_err(|e| format!("Valor inválido para LOAD_CONST_BOOL: {}", e))?;
                    self.pilha.push(Valor::Booleano(valor));
                }
                "LOAD_CONST_DECIMAL" => {
                    let literal = partes
                        .get(1)
                        .ok_or("LOAD_CONST_DECIMAL requer um argumento")?;
                    let dec = literal
                        .parse::<rust_decimal::Decimal>()
                        .map_err(|e| format!("Decimal inválido: {}", e))?;
                    self.pilha.push(Valor::Decimal(dec));
                }
                "LOAD_CONST_NULL" => {
                    self.pilha.push(Valor::Nulo);
                }

                "ADD" => {
                    let dir = self.pilha.pop().ok_or("Pilha vazia para ADD")?;
                    let esq = self.pilha.pop().ok_or("Pilha vazia para ADD")?;
                    match (esq, dir) {
                        (Valor::Inteiro(a), Valor::Inteiro(b)) => {
                            let v = a
                                .checked_add(b)
                                .ok_or("Overflow em ADD de inteiros")?;
                            self.pilha.push(Valor::Inteiro(v))
                        }
                        (Valor::Decimal(a), Valor::Decimal(b)) => {
                            self.pilha.push(Valor::Decimal(a + b))
                        }
                        (Valor::Flutuante(a), Valor::Flutuante(b)) => {
                            self.pilha.push(Valor::Flutuante(a + b))
                        }
                        (Valor::Duplo(a), Valor::Duplo(b)) => self.pilha.push(Valor::Duplo(a + b)),
                        // promoções
                        (Valor::Inteiro(a), Valor::Flutuante(b)) => {
                            self.pilha.push(Valor::Flutuante(a as f32 + b))
                        }
                        (Valor::Flutuante(a), Valor::Inteiro(b)) => {
                            self.pilha.push(Valor::Flutuante(a + b as f32))
                        }
                        (Valor::Inteiro(a), Valor::Duplo(b)) => {
                            self.pilha.push(Valor::Duplo(a as f64 + b))
                        }
                        (Valor::Duplo(a), Valor::Inteiro(b)) => {
                            self.pilha.push(Valor::Duplo(a + b as f64))
                        }
                        (Valor::Flutuante(a), Valor::Duplo(b)) => {
                            self.pilha.push(Valor::Duplo(a as f64 + b))
                        }
                        (Valor::Duplo(a), Valor::Flutuante(b)) => {
                            self.pilha.push(Valor::Duplo(a + b as f64))
                        }
                        (Valor::Texto(a), Valor::Texto(b)) => {
                            self.pilha.push(Valor::Texto(format!("{}{}", a, b)))
                        }
                        (Valor::Texto(a), v) => {
                            self.pilha.push(Valor::Texto(format!("{}{}", a, v)))
                        }
                        (v, Valor::Texto(b)) => {
                            self.pilha.push(Valor::Texto(format!("{}{}", v, b)))
                        }
                        (esq, dir) => {
                            return Err(format!(
                                "Tipos incompatíveis para ADD: {:?} e {:?}",
                                esq, dir
                            ))
                        }
                    }
                }
                "SUB" => {
                    let dir = self.pilha.pop().ok_or("Pilha vazia para SUB")?;
                    let esq = self.pilha.pop().ok_or("Pilha vazia para SUB")?;
                    match (esq, dir) {
                        (Valor::Inteiro(a), Valor::Inteiro(b)) => {
                            let v = a
                                .checked_sub(b)
                                .ok_or("Overflow em SUB de inteiros")?;
                            self.pilha.push(Valor::Inteiro(v))
                        }
                        (Valor::Decimal(a), Valor::Decimal(b)) => {
                            self.pilha.push(Valor::Decimal(a - b))
                        }
                        (Valor::Flutuante(a), Valor::Flutuante(b)) => {
                            self.pilha.push(Valor::Flutuante(a - b))
                        }
                        (Valor::Duplo(a), Valor::Duplo(b)) => self.pilha.push(Valor::Duplo(a - b)),
                        (Valor::Inteiro(a), Valor::Flutuante(b)) => {
                            self.pilha.push(Valor::Flutuante(a as f32 - b))
                        }
                        (Valor::Flutuante(a), Valor::Inteiro(b)) => {
                            self.pilha.push(Valor::Flutuante(a - b as f32))
                        }
                        (Valor::Inteiro(a), Valor::Duplo(b)) => {
                            self.pilha.push(Valor::Duplo(a as f64 - b))
                        }
                        (Valor::Duplo(a), Valor::Inteiro(b)) => {
                            self.pilha.push(Valor::Duplo(a - b as f64))
                        }
                        (Valor::Flutuante(a), Valor::Duplo(b)) => {
                            self.pilha.push(Valor::Duplo(a as f64 - b))
                        }
                        (Valor::Duplo(a), Valor::Flutuante(b)) => {
                            self.pilha.push(Valor::Duplo(a - b as f64))
                        }
                        _ => return Err("Tipos incompatíveis para SUB".to_string()),
                    }
                }
                "MUL" => {
                    let dir = self.pilha.pop().ok_or("Pilha vazia para MUL")?;
                    let esq = self.pilha.pop().ok_or("Pilha vazia para MUL")?;
                    match (esq, dir) {
                        (Valor::Inteiro(a), Valor::Inteiro(b)) => {
                            let v = a
                                .checked_mul(b)
                                .ok_or("Overflow em MUL de inteiros")?;
                            self.pilha.push(Valor::Inteiro(v))
                        }
                        (Valor::Decimal(a), Valor::Decimal(b)) => {
                            self.pilha.push(Valor::Decimal(a * b))
                        }
                        (Valor::Flutuante(a), Valor::Flutuante(b)) => {
                            self.pilha.push(Valor::Flutuante(a * b))
                        }
                        (Valor::Duplo(a), Valor::Duplo(b)) => self.pilha.push(Valor::Duplo(a * b)),
                        (Valor::Inteiro(a), Valor::Flutuante(b)) => {
                            self.pilha.push(Valor::Flutuante(a as f32 * b))
                        }
                        (Valor::Flutuante(a), Valor::Inteiro(b)) => {
                            self.pilha.push(Valor::Flutuante(a * b as f32))
                        }
                        (Valor::Inteiro(a), Valor::Duplo(b)) => {
                            self.pilha.push(Valor::Duplo(a as f64 * b))
                        }
                        (Valor::Duplo(a), Valor::Inteiro(b)) => {
                            self.pilha.push(Valor::Duplo(a * b as f64))
                        }
                        (Valor::Flutuante(a), Valor::Duplo(b)) => {
                            self.pilha.push(Valor::Duplo(a as f64 * b))
                        }
                        (Valor::Duplo(a), Valor::Flutuante(b)) => {
                            self.pilha.push(Valor::Duplo(a * b as f64))
                        }
                        _ => return Err("Tipos incompatíveis para MUL".to_string()),
                    }
                }
                "DIV" => {
                    let dir = self.pilha.pop().ok_or("Pilha vazia para DIV")?;
                    let esq = self.pilha.pop().ok_or("Pilha vazia para DIV")?;
                    match (esq, dir) {
                        (Valor::Inteiro(a), Valor::Inteiro(b)) => {
                            if b == 0 {
                                return Err("Divisão por zero".to_string());
                            }
                            let v = a
                                .checked_div(b)
                                .ok_or("Overflow em DIV de inteiros")?;
                            self.pilha.push(Valor::Inteiro(v));
                        }
                        (Valor::Decimal(a), Valor::Decimal(b)) => {
                            if b.is_zero() {
                                return Err("Divisão por zero".to_string());
                            }
                            self.pilha.push(Valor::Decimal(a / b));
                        }
                        (Valor::Flutuante(a), Valor::Flutuante(b)) => {
                            if b == 0.0 {
                                return Err("Divisão por zero".to_string());
                            }
                            self.pilha.push(Valor::Flutuante(a / b));
                        }
                        (Valor::Duplo(a), Valor::Duplo(b)) => {
                            if b == 0.0 {
                                return Err("Divisão por zero".to_string());
                            }
                            self.pilha.push(Valor::Duplo(a / b));
                        }
                        (Valor::Inteiro(a), Valor::Flutuante(b)) => {
                            if b == 0.0 {
                                return Err("Divisão por zero".to_string());
                            }
                            self.pilha.push(Valor::Flutuante(a as f32 / b));
                        }
                        (Valor::Flutuante(a), Valor::Inteiro(b)) => {
                            if b == 0 {
                                return Err("Divisão por zero".to_string());
                            }
                            self.pilha.push(Valor::Flutuante(a / b as f32));
                        }
                        (Valor::Inteiro(a), Valor::Duplo(b)) => {
                            if b == 0.0 {
                                return Err("Divisão por zero".to_string());
                            }
                            self.pilha.push(Valor::Duplo(a as f64 / b));
                        }
                        (Valor::Duplo(a), Valor::Inteiro(b)) => {
                            if b == 0 {
                                return Err("Divisão por zero".to_string());
                            }
                            self.pilha.push(Valor::Duplo(a / b as f64));
                        }
                        (Valor::Flutuante(a), Valor::Duplo(b)) => {
                            if b == 0.0 {
                                return Err("Divisão por zero".to_string());
                            }
                            self.pilha.push(Valor::Duplo(a as f64 / b));
                        }
                        (Valor::Duplo(a), Valor::Flutuante(b)) => {
                            if b == 0.0 {
                                return Err("Divisão por zero".to_string());
                            }
                            self.pilha.push(Valor::Duplo(a / b as f64));
                        }
                        _ => return Err("Tipos incompatíveis para DIV".to_string()),
                    }
                }
                "MOD" => {
                    let dir = self.pilha.pop().ok_or("Pilha vazia para MOD")?;
                    let esq = self.pilha.pop().ok_or("Pilha vazia para MOD")?;
                    match (esq, dir) {
                        (Valor::Inteiro(a), Valor::Inteiro(b)) => {
                            if b == 0 {
                                return Err("Módulo por zero".to_string());
                            }
                            let v = a
                                .checked_rem(b)
                                .ok_or("Overflow em MOD de inteiros")?;
                            self.pilha.push(Valor::Inteiro(v));
                        }
                        _ => return Err("Tipos incompatíveis para MOD".to_string()),
                    }
                }
                "NEGATE_INT" => {
                    //Negação numérica
                    let val = self.pilha.pop().ok_or("Pilha vazia para NEGATE_INT")?;
                    match val {
                        Valor::Inteiro(n) => {
                            let v = n
                                .checked_neg()
                                .ok_or("Overflow em NEGATE_INT")?;
                            self.pilha.push(Valor::Inteiro(v))
                        }
                        Valor::Decimal(d) => self.pilha.push(Valor::Decimal(-d)),
                        Valor::Flutuante(x) => self.pilha.push(Valor::Flutuante(-x)),
                        Valor::Duplo(x) => self.pilha.push(Valor::Duplo(-x)),
                        _ => return Err("Tipo incompatível para NEGATE_INT".to_string()),
                    }
                }
                "NEGATE_BOOL" => {
                    // Negação lógica
                    let val = self.pilha.pop().ok_or("Pilha vazia para NEGATE_BOOL")?;
                    match val {
                        Valor::Booleano(b) => self.pilha.push(Valor::Booleano(!b)),
                        _ => return Err("Tipo incompatível para NEGATE_BOOL".to_string()),
                    }
                }

                // Instruções de Comparação (para inteiros e booleanos)
                "COMPARE_EQ" => {
                    let dir = self.pilha.pop().ok_or("Pilha vazia para COMPARE_EQ")?;
                    let esq = self.pilha.pop().ok_or("Pilha vazia para COMPARE_EQ")?;
                    self.pilha.push(Valor::Booleano(esq == dir));
                }
                "COMPARE_NE" => {
                    let dir = self.pilha.pop().ok_or("Pilha vazia para COMPARE_NE")?;
                    let esq = self.pilha.pop().ok_or("Pilha vazia para COMPARE_NE")?;
                    self.pilha.push(Valor::Booleano(esq != dir));
                }
                "COMPARE_LT" => {
                    let dir = self.pilha.pop().ok_or("Pilha vazia para COMPARE_LT")?;
                    let esq = self.pilha.pop().ok_or("Pilha vazia para COMPARE_LT")?;
                    match (esq, dir) {
                        (Valor::Inteiro(a), Valor::Inteiro(b)) => {
                            self.pilha.push(Valor::Booleano(a < b))
                        }
                        (Valor::Decimal(a), Valor::Decimal(b)) => {
                            self.pilha.push(Valor::Booleano(a < b))
                        }
                        (Valor::Flutuante(a), Valor::Flutuante(b)) => {
                            self.pilha.push(Valor::Booleano(a < b))
                        }
                        (Valor::Duplo(a), Valor::Duplo(b)) => {
                            self.pilha.push(Valor::Booleano(a < b))
                        }
                        (Valor::Inteiro(a), Valor::Flutuante(b)) => {
                            self.pilha.push(Valor::Booleano((a as f32) < b))
                        }
                        (Valor::Flutuante(a), Valor::Inteiro(b)) => {
                            self.pilha.push(Valor::Booleano(a < (b as f32)))
                        }
                        (Valor::Inteiro(a), Valor::Duplo(b)) => {
                            self.pilha.push(Valor::Booleano((a as f64) < b))
                        }
                        (Valor::Duplo(a), Valor::Inteiro(b)) => {
                            self.pilha.push(Valor::Booleano(a < (b as f64)))
                        }
                        (Valor::Flutuante(a), Valor::Duplo(b)) => {
                            self.pilha.push(Valor::Booleano((a as f64) < b))
                        }
                        (Valor::Duplo(a), Valor::Flutuante(b)) => {
                            self.pilha.push(Valor::Booleano(a < (b as f64)))
                        }
                        _ => return Err("Tipos incompatíveis para COMPARE_LT".to_string()),
                    }
                }
                "COMPARE_GT" => {
                    let dir = self.pilha.pop().ok_or("Pilha vazia para COMPARE_GT")?;
                    let esq = self.pilha.pop().ok_or("Pilha vazia para COMPARE_GT")?;
                    match (esq, dir) {
                        (Valor::Inteiro(a), Valor::Inteiro(b)) => {
                            self.pilha.push(Valor::Booleano(a > b))
                        }
                        (Valor::Decimal(a), Valor::Decimal(b)) => {
                            self.pilha.push(Valor::Booleano(a > b))
                        }
                        (Valor::Flutuante(a), Valor::Flutuante(b)) => {
                            self.pilha.push(Valor::Booleano(a > b))
                        }
                        (Valor::Duplo(a), Valor::Duplo(b)) => {
                            self.pilha.push(Valor::Booleano(a > b))
                        }
                        (Valor::Inteiro(a), Valor::Flutuante(b)) => {
                            self.pilha.push(Valor::Booleano((a as f32) > b))
                        }
                        (Valor::Flutuante(a), Valor::Inteiro(b)) => {
                            self.pilha.push(Valor::Booleano(a > (b as f32)))
                        }
                        (Valor::Inteiro(a), Valor::Duplo(b)) => {
                            self.pilha.push(Valor::Booleano((a as f64) > b))
                        }
                        (Valor::Duplo(a), Valor::Inteiro(b)) => {
                            self.pilha.push(Valor::Booleano(a > (b as f64)))
                        }
                        (Valor::Flutuante(a), Valor::Duplo(b)) => {
                            self.pilha.push(Valor::Booleano((a as f64) > b))
                        }
                        (Valor::Duplo(a), Valor::Flutuante(b)) => {
                            self.pilha.push(Valor::Booleano(a > (b as f64)))
                        }
                        _ => return Err("Tipos incompatíveis para COMPARE_GT".to_string()),
                    }
                }
                "COMPARE_LE" => {
                    let dir = self.pilha.pop().ok_or("Pilha vazia para COMPARE_LE")?;
                    let esq = self.pilha.pop().ok_or("Pilha vazia para COMPARE_LE")?;
                    match (esq, dir) {
                        (Valor::Inteiro(a), Valor::Inteiro(b)) => {
                            self.pilha.push(Valor::Booleano(a <= b))
                        }
                        (Valor::Decimal(a), Valor::Decimal(b)) => {
                            self.pilha.push(Valor::Booleano(a <= b))
                        }
                        (Valor::Flutuante(a), Valor::Flutuante(b)) => {
                            self.pilha.push(Valor::Booleano(a <= b))
                        }
                        (Valor::Duplo(a), Valor::Duplo(b)) => {
                            self.pilha.push(Valor::Booleano(a <= b))
                        }
                        (Valor::Inteiro(a), Valor::Flutuante(b)) => {
                            self.pilha.push(Valor::Booleano((a as f32) <= b))
                        }
                        (Valor::Flutuante(a), Valor::Inteiro(b)) => {
                            self.pilha.push(Valor::Booleano(a <= (b as f32)))
                        }
                        (Valor::Inteiro(a), Valor::Duplo(b)) => {
                            self.pilha.push(Valor::Booleano((a as f64) <= b))
                        }
                        (Valor::Duplo(a), Valor::Inteiro(b)) => {
                            self.pilha.push(Valor::Booleano(a <= (b as f64)))
                        }
                        (Valor::Flutuante(a), Valor::Duplo(b)) => {
                            self.pilha.push(Valor::Booleano((a as f64) <= b))
                        }
                        (Valor::Duplo(a), Valor::Flutuante(b)) => {
                            self.pilha.push(Valor::Booleano(a <= (b as f64)))
                        }
                        _ => return Err("Tipos incompatíveis para COMPARE_LE".to_string()),
                    }
                }

                "COMPARE_GE" => {
                    let dir = self.pilha.pop().ok_or("Pilha vazia para COMPARE_GE")?;
                    let esq = self.pilha.pop().ok_or("Pilha vazia para COMPARE_GE")?;
                    match (esq, dir) {
                        (Valor::Inteiro(a), Valor::Inteiro(b)) => {
                            self.pilha.push(Valor::Booleano(a >= b))
                        }
                        (Valor::Decimal(a), Valor::Decimal(b)) => {
                            self.pilha.push(Valor::Booleano(a >= b))
                        }
                        (Valor::Flutuante(a), Valor::Flutuante(b)) => {
                            self.pilha.push(Valor::Booleano(a >= b))
                        }
                        (Valor::Duplo(a), Valor::Duplo(b)) => {
                            self.pilha.push(Valor::Booleano(a >= b))
                        }
                        (Valor::Inteiro(a), Valor::Flutuante(b)) => {
                            self.pilha.push(Valor::Booleano((a as f32) >= b))
                        }
                        (Valor::Flutuante(a), Valor::Inteiro(b)) => {
                            self.pilha.push(Valor::Booleano(a >= (b as f32)))
                        }
                        (Valor::Inteiro(a), Valor::Duplo(b)) => {
                            self.pilha.push(Valor::Booleano((a as f64) >= b))
                        }
                        (Valor::Duplo(a), Valor::Inteiro(b)) => {
                            self.pilha.push(Valor::Booleano(a >= (b as f64)))
                        }
                        (Valor::Flutuante(a), Valor::Duplo(b)) => {
                            self.pilha.push(Valor::Booleano((a as f64) >= b))
                        }
                        (Valor::Duplo(a), Valor::Flutuante(b)) => {
                            self.pilha.push(Valor::Booleano(a >= (b as f64)))
                        }
                        _ => return Err("Tipos incompatíveis para COMPARE_GE".to_string()),
                    }
                }
                // Instruções de Salto
                "JUMP" => {
                    // Salto incondicional
                    let target_ip: usize = partes
                        .get(1)
                        .ok_or("JUMP requer um endereço de destino")?
                        .parse()
                        .map_err(|e| format!("Endereço inválido para JUMP: {}", e))?;
                    if target_ip > self.bytecode.len() {
                        return Err(format!(
                            "Salto fora dos limites: {} >= {}",
                            target_ip,
                            self.bytecode.len()
                        ));
                    }
                    self.ip = target_ip;
                }
                "JUMP_IF_FALSE" => {
                    // Salto condicional
                    let target_ip: usize = partes
                        .get(1)
                        .ok_or("JUMP_IF_FALSE requer um endereço de destino")?
                        .parse()
                        .map_err(|e| format!("Endereço inválido para JUMP_IF_FALSE: {}", e))?;
                    if target_ip > self.bytecode.len() {
                        return Err(format!(
                            "Salto fora dos limites: {} >= {}",
                            target_ip,
                            self.bytecode.len()
                        ));
                    }
                    let condicao = self.pilha.pop().ok_or("Pilha vazia para JUMP_IF_FALSE")?;
                    match condicao {
                        Valor::Booleano(b) => {
                            if !b {
                                self.ip = target_ip;
                            } else {
                                self.ip += 1; // Se a condição for verdadeira, avança normalmente
                            }
                        }
                        _ => return Err("JUMP_IF_FALSE requer um valor booleano".to_string()),
                    }
                }
                // Instruções para classes
                "NEW_OBJECT" => {
                    let nome_classe = partes.get(1).ok_or("NEW_OBJECT requer nome da classe")?;
                    let num_args = partes
                        .get(2)
                        .ok_or("NEW_OBJECT requer número de argumentos")?
                        .parse::<usize>()
                        .map_err(|e| format!("Número inválido de argumentos: {}", e))?;

                    // Pegar argumentos da pilha
                    if self.pilha.len() < num_args {
                        return Err(format!("Pilha insuficiente para NEW_OBJECT"));
                    }
                    let argumentos = self.pilha.split_off(self.pilha.len() - num_args);

                    // Criar objeto
                    let objeto =
                        crate::objetos::criar_objeto(self, nome_classe, argumentos).await?;
                    self.pilha.push(objeto);
                }

                "GET_PROPERTY" => {
                    let nome_propriedade = partes
                        .get(1)
                        .ok_or("GET_PROPERTY requer nome da propriedade")?;
                    let objeto = self.pilha.pop().ok_or("Pilha vazia para GET_PROPERTY")?;

                    match objeto {
                        Valor::Objeto { campos, .. } => {
                            // Propriedades calculadas da stdlib (ex.: Tamanho de Lista/Dicionario)
                            if (*nome_propriedade == "Tamanho" || *nome_propriedade == "Contagem")
                                && campos.borrow().contains_key("__itens__")
                            {
                                let len = match campos.borrow().get("__itens__") {
                                    Some(Valor::Array(v)) => v.len() as i64,
                                    _ => 0,
                                };
                                self.pilha.push(Valor::Inteiro(len));
                                continue;
                            }
                            let valor = campos
                                .borrow()
                                .get(*nome_propriedade)
                                .cloned()
                                .unwrap_or(Valor::Nulo);
                            self.pilha.push(valor);
                        }
                        Valor::Array(ref itens) => {
                            if *nome_propriedade == "Tamanho"
                                || *nome_propriedade == "Comprimento"
                                || *nome_propriedade == "Length"
                            {
                                self.pilha.push(Valor::Inteiro(itens.len() as i64));
                            } else {
                                return Err(format!(
                                    "GET_PROPERTY '{}' não existe em array",
                                    nome_propriedade
                                ));
                            }
                        }
                        _ => {
                            eprintln!("DEBUG: GET_PROPERTY {} falhou no ip = {}, codigo = {}, objeto era: {:?}", nome_propriedade, self.ip, self.code_id, objeto);
                            return Err("GET_PROPERTY requer um objeto".to_string());
                        }
                    }
                }

                "SET_PROPERTY" => {
                    let prop = partes.get(1).ok_or("SET_PROPERTY requer nome")?.to_string();
                    let valor = self
                        .pilha
                        .pop()
                        .ok_or("Pilha vazia para SET_PROPERTY valor")?;
                    let alvo = self
                        .pilha
                        .pop()
                        .ok_or("Pilha vazia para SET_PROPERTY alvo")?;
                    match alvo {
                        Valor::Objeto { campos, .. } => {
                            campos.borrow_mut().insert(prop, valor);
                            self.pilha.push(Valor::Nulo);
                        }
                        Valor::Texto(nome_classe) => {
                            if let Some(cls) = self.classes.get(&nome_classe) {
                                cls.campos_estaticos.borrow_mut().insert(prop, valor);
                                self.pilha.push(Valor::Nulo);
                            } else {
                                return Err("Classe não encontrada para SET_PROPERTY".into());
                            }
                        }
                        _ => return Err("SET_PROPERTY em tipo inválido".into()),
                    }
                }

                "GET_STATIC_PROPERTY" => {
                    let nome_classe = partes
                        .get(1)
                        .ok_or("GET_STATIC_PROPERTY requer nome da classe")?;
                    let nome_prop = partes
                        .get(2)
                        .ok_or("GET_STATIC_PROPERTY requer nome da propriedade")?;
                    let classe = self
                        .classes
                        .get(*nome_classe)
                        .ok_or_else(|| format!("Classe \"{}\" não encontrada", nome_classe))?;
                    let valor = classe
                        .campos_estaticos
                        .borrow()
                        .get(*nome_prop)
                        .cloned()
                        .unwrap_or(Valor::Nulo);
                    self.pilha.push(valor);
                }

                "SET_STATIC_PROPERTY" => {
                    let nome_classe = partes
                        .get(1)
                        .ok_or("SET_STATIC_PROPERTY requer nome da classe")?;
                    let nome_prop = partes
                        .get(2)
                        .ok_or("SET_STATIC_PROPERTY requer nome da propriedade")?;
                    let valor = self
                        .pilha
                        .pop()
                        .ok_or("Pilha vazia em SET_STATIC_PROPERTY")?;
                    let classe = self
                        .classes
                        .get_mut(*nome_classe)
                        .ok_or_else(|| format!("Classe \"{}\" não encontrada", nome_classe))?;
                    classe
                        .campos_estaticos
                        .borrow_mut()
                        .insert(nome_prop.to_string(), valor);
                }

                "CALL_METHOD" => {
                    let nome_metodo = partes.get(1).ok_or("CALL_METHOD requer nome do método")?;
                    let num_args = partes
                        .get(2)
                        .ok_or("CALL_METHOD requer número de argumentos")?
                        .parse::<usize>()
                        .map_err(|e| format!("Número inválido de argumentos: {}", e))?;

                    // Pegar argumentos da pilha
                    if self.pilha.len() < num_args + 1 {
                        // +1 para o objeto
                        return Err(format!("Pilha insuficiente para CALL_METHOD"));
                    }

                    let argumentos = if num_args > 0 {
                        self.pilha.split_off(self.pilha.len() - num_args)
                    } else {
                        Vec::new()
                    };

                    let mut objeto = self
                        .pilha
                        .pop()
                        .ok_or("Pilha vazia para objeto em CALL_METHOD")?;
                    let valor_retorno =
                        crate::objetos::chamar_metodo(self, &mut objeto, nome_metodo, argumentos)
                            .await?;
                    self.pilha.push(valor_retorno);
                }

                "CALL_STATIC_METHOD" => {
                    let nome_classe = partes
                        .get(1)
                        .ok_or("CALL_STATIC_METHOD requer nome da classe")?;
                    let nome_metodo = partes
                        .get(2)
                        .ok_or("CALL_STATIC_METHOD requer nome do método")?;
                    let num_args = partes
                        .get(3)
                        .ok_or("CALL_STATIC_METHOD requer número de argumentos")?
                        .parse::<usize>()
                        .map_err(|e| format!("Número inválido de argumentos: {}", e))?;

                    if self.pilha.len() < num_args {
                        return Err(format!("Pilha insuficiente para CALL_STATIC_METHOD"));
                    }

                    let argumentos = if num_args > 0 {
                        self.pilha.split_off(self.pilha.len() - num_args)
                    } else {
                        Vec::new()
                    };

                    let resultado = crate::objetos::chamar_metodo_estatico(
                        self,
                        nome_classe,
                        nome_metodo,
                        argumentos,
                    )
                    .await?;
                    self.pilha.push(resultado);
                }

                "SET_DEFAULT" => {
                    let nome_var = partes
                        .get(1)
                        .ok_or("SET_DEFAULT requer um nome de variável")?;
                    let has_value = match self.variaveis.get(*nome_var) {
                        Some(Valor::Nulo) | None => false,
                        _ => true,
                    };
                    if !has_value {
                        let default_expr_bytecode_str = partes[2..].join(" ");
                        let mut temp_vm =
                            VM::new(vec![default_expr_bytecode_str], self.base_dir.clone());
                        temp_vm.debug = self.debug.clone();
                        temp_vm.code_id = format!("expr-default:{}", nome_var);
                        Box::pin(temp_vm.run()).await?;
                        let valor = temp_vm.pilha.pop().unwrap_or(Valor::Nulo);
                        self.variaveis.insert(nome_var.to_string(), valor);
                    }
                }
                "POP" => {
                    self.pilha.pop().ok_or("Pilha vazia em POP")?;
                }

                "CALL_BASE_CONSTRUCTOR" => {
                    let num_args = partes
                        .get(1)
                        .ok_or("CALL_BASE_CONSTRUCTOR requer número de argumentos")?
                        .parse::<usize>()
                        .map_err(|e| format!("Número inválido de argumentos: {}", e))?;
                    if self.pilha.len() < num_args {
                        return Err(format!("Pilha insuficiente para CALL_BASE_CONSTRUCTOR"));
                    }
                    let argumentos = self.pilha.split_off(self.pilha.len() - num_args);
                    let este_obj = self
                        .variaveis
                        .get("este")
                        .cloned()
                        .ok_or("CALL_BASE_CONSTRUCTOR requer 'este' no escopo")?;
                    if let Valor::Objeto { nome_classe, .. } = &este_obj {
                        if let Some(classe_info) = self.classes.get(nome_classe).cloned() {
                            if let Some(parent_name) = &classe_info.nome_classe_pai {
                                if let Some(parent_info) = self.classes.get(parent_name).cloned() {
                                    if let Some(constructor_info) =
                                        parent_info.metodos.get("construtor").cloned()
                                    {
                                        let mut constructor_vm = VM {
                                            pilha: Vec::new(),
                                            variaveis: HashMap::new(),
                                            bytecode: constructor_info.corpo.clone(),
                                            ip: 0,
                                            classes: self.classes.clone(),
                                            functions: self.functions.clone(),
                                            loaded_modules: self.loaded_modules.clone(),
                                            base_dir: self.base_dir.clone(),
                                            debug: self.debug.clone(),
                                            code_id: format!("base_ctor:{}", parent_name),
                                            task_counter: self.task_counter.clone(),
                                            tasks: self.tasks.clone(),
                                            call_stack: Vec::new(),
                                        };
                                        constructor_vm
                                            .variaveis
                                            .insert("este".to_string(), este_obj.clone());
                                        for (i, param_name) in
                                            constructor_info.parametros.iter().enumerate()
                                        {
                                            if let Some(arg_val) = argumentos.get(i) {
                                                constructor_vm
                                                    .variaveis
                                                    .insert(param_name.clone(), arg_val.clone());
                                            }
                                        }
                                        Box::pin(constructor_vm.run()).await?;
                                    }
                                }
                            }
                        }
                    }
                }

                "RETURN" => {
                    // interrompe a execução do frame atual;
                    // o valor de retorno já está no topo da pilha
                    return Ok(());
                }

                "CALL_FUNCTION" => {
                    let nome = partes.get(1).ok_or("CALL_FUNCTION requer nome")?;
                    let nargs = partes
                        .get(2)
                        .ok_or("CALL_FUNCTION requer n")?
                        .parse::<usize>()
                        .map_err(|_| "n inválido")?;
                    if self.pilha.len() < nargs {
                        return Err("Pilha insuficiente para CALL_FUNCTION".into());
                    }
                    // argumentos em ordem
                    let args = self.pilha.split_off(self.pilha.len() - nargs);

                    // Intrínsecas simples de I/O: EscreverLinha e LerLinha
                    // Suporta nomes qualificados com namespace: pega o último segmento após '.'
                    let nome_simples = {
                        let full = *nome;
                        match full.rsplit('.').next() {
                            Some(s) => s,
                            None => full,
                        }
                    };
                    match nome_simples {
                        "EscreverLinha" => {
                            if args.is_empty() {
                                println!("");
                            } else {
                                let mut texto = String::new();
                                for v in &args {
                                    texto.push_str(&v.to_string());
                                }
                                println!("{}", texto);
                            }
                            // Retorna nulo
                            self.pilha.push(Valor::Nulo);
                            continue;
                        }
                        "LerLinha" => {
                            let mut entrada = String::new();
                            io::stdin()
                                .read_line(&mut entrada)
                                .map_err(|e| format!("Erro ao ler entrada: {}", e))?;
                            let s = entrada.trim_end_matches(['\r', '\n']).to_string();
                            self.pilha.push(Valor::Texto(s));
                            continue;
                        }
                        _ => {}
                    }
                    // procura função
                    let func = self
                        .functions
                        .get(*nome)
                        .ok_or_else(|| format!("Função \"{}\" não definida", nome))?
                        .clone();

                    // cria ambiente local: parametros -> argumentos
                    let mut vars = HashMap::new();
                    for (i, p) in func.parametros.iter().enumerate() {
                        let val = args.get(i).cloned().unwrap_or(Valor::Nulo);
                        vars.insert(p.clone(), val);
                    }

                    // executa corpo em mini-VM
                    let mut vm = VM {
                        pilha: Vec::new(),
                        variaveis: vars,
                        bytecode: func.corpo,
                        ip: 0,
                        classes: self.classes.clone(),
                        functions: self.functions.clone(),
                        loaded_modules: self.loaded_modules.clone(),
                        base_dir: self.base_dir.clone(),
                        debug: self.debug.clone(),
                        code_id: format!("func:{}", func.nome),
                        task_counter: self.task_counter.clone(),
                        tasks: self.tasks.clone(),
                        call_stack: Vec::new(),
                    };
                    Box::pin(vm.run()).await?;
                    self.pilha.push(vm.pilha.pop().unwrap_or(Valor::Nulo));
                }

                // === CHAMADAS NATIVAS (via atributo [Nativo("chave")]) ===
                // CALL_STATIC_NATIVE <chave> <nargs>
                "CALL_STATIC_NATIVE" => {
                    let chave = partes
                        .get(1)
                        .ok_or("CALL_STATIC_NATIVE requer chave")?
                        .to_string();
                    let nargs: usize = partes
                        .get(2)
                        .ok_or("CALL_STATIC_NATIVE requer nargs")?
                        .parse()
                        .map_err(|_| "nargs inválido em CALL_STATIC_NATIVE".to_string())?;
                    if self.pilha.len() < nargs {
                        return Err(
                            "Pilha insuficiente para argumentos de CALL_STATIC_NATIVE".to_string(),
                        );
                    }
                    let args = self.pilha.split_off(self.pilha.len() - nargs);
                    let resultado = despachar_nativo_estatico(&chave, args)?;
                    self.pilha.push(resultado);
                }

                // CALL_NATIVE <chave> <nargs>   (método de instância — 'este' já foi empilhado)
                "CALL_NATIVE" => {
                    let chave = partes.get(1).ok_or("CALL_NATIVE requer chave")?.to_string();
                    let nargs: usize = partes
                        .get(2)
                        .ok_or("CALL_NATIVE requer nargs")?
                        .parse()
                        .map_err(|_| "nargs inválido em CALL_NATIVE".to_string())?;
                    if self.pilha.len() < nargs {
                        return Err("Pilha insuficiente para argumentos de CALL_NATIVE".to_string());
                    }
                    let args = self.pilha.split_off(self.pilha.len() - nargs);
                    let este_val = self
                        .pilha
                        .pop()
                        .ok_or("Pilha vazia para 'este' em CALL_NATIVE")?;
                    let resultado = despachar_nativo_instancia(&chave, este_val, args)?;
                    self.pilha.push(resultado);
                }

                // === CHAMADAS NATIVAS ASSÍNCRONAS (I/O via tokio) ===
                // CALL_STATIC_NATIVE_ASYNC <chave> <nargs>
                // CALL_NATIVE_ASYNC <chave> <nargs>   ('este' já foi empilhado antes dos args)
                // Executa uma operação assíncrona e empilha um Valor::Task já resolvido.
                "CALL_STATIC_NATIVE_ASYNC" | "CALL_NATIVE_ASYNC" => {
                    let eh_instancia = partes[0] == "CALL_NATIVE_ASYNC";
                    let chave = partes
                        .get(1)
                        .ok_or("CALL_*_NATIVE_ASYNC requer chave")?
                        .to_string();
                    let nargs: usize = partes
                        .get(2)
                        .ok_or("CALL_*_NATIVE_ASYNC requer nargs")?
                        .parse()
                        .map_err(|_| "nargs inválido em CALL_*_NATIVE_ASYNC".to_string())?;
                    if self.pilha.len() < nargs {
                        return Err(
                            "Pilha insuficiente para argumentos de CALL_*_NATIVE_ASYNC".to_string(),
                        );
                    }
                    let args = self.pilha.split_off(self.pilha.len() - nargs);
                    let este = if eh_instancia {
                        Some(self.pilha.pop().ok_or("Pilha vazia para 'este' em CALL_NATIVE_ASYNC")?)
                    } else {
                        None
                    };
                    self.executar_nativo_async(chave, args, este).await?;
                }

                // === INSTRUÇÃO AWAIT ===
                // Resolve uma Task que já foi agendada (ou ainda está pendente).
                // Como CALL_STATIC_NATIVE_ASYNC já resolve inline, esta instrução
                // extrai o resultado da Task e o empilha.
                "AWAIT" => {
                    let task_val = self.pilha.pop().ok_or("Pilha vazia para AWAIT")?;

                    match task_val {
                        Valor::Task { id, status, result } => match status {
                            TaskStatus::Completed => {
                                let val = result.map(|b| *b).unwrap_or(Valor::Nulo);
                                self.pilha.push(val);
                            }
                            TaskStatus::Failed(msg) => {
                                return Err(format!("Task<{}> falhou: {}", id, msg));
                            }
                            TaskStatus::Pending | TaskStatus::Running => {
                                // Consulta o mapa de tasks para ver se já terminou
                                let tasks_map = self.tasks.lock().unwrap();
                                if let Some(task) = tasks_map.get(&id) {
                                    match &task.status {
                                        TaskStatus::Completed => {
                                            let val = task
                                                .result
                                                .as_ref()
                                                .map(|b| *b.clone())
                                                .unwrap_or(Valor::Nulo);
                                            drop(tasks_map);
                                            self.pilha.push(val);
                                        }
                                        TaskStatus::Failed(msg) => {
                                            let msg = msg.clone();
                                            drop(tasks_map);
                                            return Err(format!("Task<{}> falhou: {}", id, msg));
                                        }
                                        _ => {
                                            drop(tasks_map);
                                            // Task ainda em execução — empilha Nulo como
                                            // resultado provisório (será melhorado com
                                            // continuations no futuro)
                                            self.pilha.push(Valor::Nulo);
                                        }
                                    }
                                } else {
                                    drop(tasks_map);
                                    self.pilha.push(Valor::Nulo);
                                }
                            }
                        },
                        // Se não for uma Task, retorna o valor como está
                        // (compatibilidade com chamadas síncronas usadas com aguarde)
                        outro => {
                            self.pilha.push(outro);
                        }
                    }
                }

                // === CREATE_TASK — cria uma task pendente explicitamente ===
                "CREATE_TASK" => {
                    let task_id = {
                        let mut counter = self.task_counter.lock().unwrap();
                        let id = *counter;
                        *counter += 1;
                        id
                    };
                    {
                        let mut tasks_map = self.tasks.lock().unwrap();
                        tasks_map.insert(
                            task_id,
                            Task {
                                status: TaskStatus::Pending,
                                result: None,
                            },
                        );
                    }
                    self.pilha.push(Valor::Task {
                        id: task_id,
                        status: TaskStatus::Pending,
                        result: None,
                    });
                }

                // === TASK_COMPLETE — marca uma task como concluída ===
                "TASK_COMPLETE" => {
                    let result_val = self
                        .pilha
                        .pop()
                        .ok_or("Pilha vazia para resultado em TASK_COMPLETE")?;
                    let task_val = self
                        .pilha
                        .pop()
                        .ok_or("Pilha vazia para task em TASK_COMPLETE")?;
                    if let Valor::Task { id, .. } = task_val {
                        let mut tasks_map = self.tasks.lock().unwrap();
                        if let Some(task) = tasks_map.get_mut(&id) {
                            task.status = TaskStatus::Completed;
                            task.result = Some(Box::new(result_val.clone()));
                        }
                        self.pilha.push(Valor::Task {
                            id,
                            status: TaskStatus::Completed,
                            result: Some(Box::new(result_val)),
                        });
                    } else {
                        return Err("TASK_COMPLETE requer um Valor::Task na pilha".into());
                    }
                }

                // Ignora comentários ou linhas vazias
                op if op.starts_with(';') || op.is_empty() => {}
                _ => {
                    return Err(format!("Instrução desconhecida: {}", op));
                }
            }
        }

        Ok(())
    }

    pub(crate) async fn executar_codigo_global(&mut self) -> Result<(), String> {
        // Filtra o bytecode para obter apenas as instruções globais
        let mut codigo_global = Vec::new();
        let mut i = 0;
        while i < self.bytecode.len() {
            let instrucao = &self.bytecode[i];
            if instrucao.starts_with("DEFINE_CLASS") {
                // Pula a definição da classe e seus métodos
                i += 1;
                while i < self.bytecode.len() && !self.bytecode[i].starts_with("END_CLASS") {
                    i += 1;
                }
                i += 1; // Pula o END_CLASS
            } else if instrucao.starts_with("DEFINE_STATIC_CLASS") {
                // A classe estática não possui END_CLASS
                i += 1;
            } else if instrucao.starts_with("DEFINE_FUNCTION")
                || instrucao.starts_with("DEFINE_METHOD")
                || instrucao.starts_with("DEFINE_STATIC_METHOD")
            {
                // Pula a definição e seu corpo
                let partes: Vec<&str> = instrucao.split(' ').collect();
                let tamanho_str = if partes[0] == "DEFINE_CLASS" {
                    "0"
                } else {
                    if partes[0] == "DEFINE_FUNCTION" {
                        partes.get(2).unwrap_or(&"0")
                    } else {
                        // DEFINE_METHOD e DEFINE_STATIC_METHOD
                        partes.get(4).unwrap_or(&"0")
                    }
                };
                let tamanho: usize = tamanho_str.parse().unwrap_or(0);
                i = i.saturating_add(tamanho).saturating_add(1);
            } else {
                codigo_global.push(instrucao.clone());
                i += 1;
            }
        }

        if codigo_global.is_empty() {
            return Ok(());
        }

        // Executa o código global em uma nova VM para não interferir com o escopo principal
        let mut vm_global = VM {
            pilha: Vec::new(),
            variaveis: self.variaveis.clone(), // Pode herdar variáveis globais se necessário
            bytecode: codigo_global,
            ip: 0,
            classes: self.classes.clone(),
            functions: self.functions.clone(),
            loaded_modules: self.loaded_modules.clone(),
            base_dir: self.base_dir.clone(),
            debug: self.debug.clone(),
            code_id: "global:init".to_string(),
            call_stack: self.call_stack.clone(),
            task_counter: self.task_counter.clone(),
            tasks: self.tasks.clone(),
        };

        vm_global.run().await
    }

    pub(crate) fn run_apenas_inicializadores(&mut self) -> Result<(), String> {
        while self.ip < self.bytecode.len() {
            let instrucao_str = self.bytecode[self.ip].clone();
            let partes: Vec<&str> = instrucao_str.split_whitespace().collect();
            let op = partes.get(0).ok_or("Instrução vazia encontrada")?;

            self.ip += 1;

            match *op {
                "DEFINE_CLASS" => {
                    while self.ip < self.bytecode.len()
                        && !self.bytecode[self.ip].starts_with("END_CLASS")
                    {
                        self.ip += 1;
                    }
                    self.ip += 1; // Pula o END_CLASS
                }
                "DEFINE_STATIC_CLASS" => {
                    // Sem END_CLASS, apenas pule a instrução atual
                }
                "DEFINE_FUNCTION" | "DEFINE_METHOD" | "DEFINE_STATIC_METHOD" => {
                    let tamanho_str = if *op == "DEFINE_FUNCTION" {
                        partes.get(2).unwrap_or(&"0")
                    } else {
                        partes.get(4).unwrap_or(&"0")
                    };
                    let tamanho: usize = tamanho_str.parse().unwrap_or(0);
                    self.ip = self.ip.saturating_add(tamanho);
                }
                "LOAD_CONST_STR" | "LOAD_CONST_INT" | "LOAD_CONST_BOOL" | "LOAD_CONST_NULL"
                | "LOAD_CONST_FLOAT" | "LOAD_CONST_DOUBLE" => {
                    // Executa apenas as instruções de carregamento de constantes
                    // (Reciclando a lógica do `run` principal)
                    match *op {
                        "LOAD_CONST_STR" => {
                            let texto = crate::util::parse_load_const_str(&instrucao_str);
                            self.pilha.push(Valor::Texto(texto));
                        }
                        "LOAD_CONST_BOOL" => {
                            let valor = partes
                                .get(1)
                                .ok_or("LOAD_CONST_BOOL requer um argumento")?
                                .parse::<bool>()
                                .map_err(|e| {
                                    format!("Valor inválido para LOAD_CONST_BOOL: {}", e)
                                })?;
                            self.pilha.push(Valor::Booleano(valor));
                        }
                        "LOAD_CONST_INT" => {
                            let valor = partes
                                .get(1)
                                .ok_or("LOAD_CONST_INT requer um argumento")?
                                .parse::<i64>()
                                .map_err(|e| {
                                    format!("Valor inválido para LOAD_CONST_INT: {}", e)
                                })?;
                            self.pilha.push(Valor::Inteiro(valor));
                        }
                        "LOAD_CONST_FLOAT" => {
                            let valor = partes
                                .get(1)
                                .ok_or("LOAD_CONST_FLOAT requer um argumento")?
                                .parse::<f32>()
                                .map_err(|e| {
                                    format!("Valor inválido para LOAD_CONST_FLOAT: {}", e)
                                })?;
                            self.pilha.push(Valor::Flutuante(valor));
                        }
                        "LOAD_CONST_DOUBLE" => {
                            let valor = partes
                                .get(1)
                                .ok_or("LOAD_CONST_DOUBLE requer um argumento")?
                                .parse::<f64>()
                                .map_err(|e| {
                                    format!("Valor inválido para LOAD_CONST_DOUBLE: {}", e)
                                })?;
                            self.pilha.push(Valor::Duplo(valor));
                        }
                        _ => {}
                    }
                }
                "SET_STATIC_PROPERTY" => {
                    let nome_classe = partes
                        .get(1)
                        .ok_or("SET_STATIC_PROPERTY requer nome da classe")?;
                    let nome_prop = partes
                        .get(2)
                        .ok_or("SET_STATIC_PROPERTY requer nome da propriedade")?;
                    let valor = self
                        .pilha
                        .pop()
                        .ok_or("Pilha vazia em SET_STATIC_PROPERTY")?;
                    let classe = self
                        .classes
                        .get_mut(*nome_classe)
                        .ok_or_else(|| format!("Classe \"{}\" não encontrada", nome_classe))?;
                    classe
                        .campos_estaticos
                        .borrow_mut()
                        .insert(nome_prop.to_string(), valor);
                }
                // Ignora todas as outras instruções
                _ => {}
            }
        }
        Ok(())
    }

    /// Executa um nativo assíncrono, registra a task concluída e empilha Valor::Task.
    async fn executar_nativo_async(
        &mut self,
        chave: String,
        args: Vec<Valor>,
        este: Option<Valor>,
    ) -> Result<(), String> {
        // Aloca um ID de task
        let task_id = {
            let mut counter = self.task_counter.lock().unwrap();
            let id = *counter;
            *counter += 1;
            id
        };

        // Como já estamos dentro de um contexto async (fn run é async),
        // podemos fazer .await directamente.
        let resultado_async = despachar_nativo_assincrono(&chave, args, este).await;

        // Registra a task como concluída
        let (status, result) = match resultado_async {
            Ok(val) => (TaskStatus::Completed, Some(Box::new(val.clone()))),
            Err(msg) => (TaskStatus::Failed(msg), None),
        };
        {
            let mut tasks_map = self.tasks.lock().unwrap();
            tasks_map.insert(
                task_id,
                Task {
                    status: status.clone(),
                    result: result.clone(),
                },
            );
        }

        // Empilha um Valor::Task para que AWAIT possa resolver
        self.pilha.push(Valor::Task {
            id: task_id,
            status,
            result,
        });
        Ok(())
    }
}

