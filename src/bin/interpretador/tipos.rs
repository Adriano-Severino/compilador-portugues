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

use rust_decimal::Decimal;

// Disponibiliza o JIT da crate de biblioteca quando a feature estiver ativa
#[cfg(feature = "jit")]
use compilador_portugues::jit::CraneliftJit;

//cargo run --bin compilador -- teste.pr --target=bytecode
//cargo run --bin interpretador -- teste.pbc

// Enum para representar os diferentes tipos de valores que a nossa VM pode manipular.
#[derive(Clone, Debug)]
pub(crate) enum Valor {
    Inteiro(i64),
    Flutuante(f32),
    Duplo(f64),
    Texto(String),
    Booleano(bool),
    Decimal(Decimal),
    Array(Vec<Valor>),
    Nulo,
    Objeto {
        nome_classe: String,
        campos: Rc<RefCell<HashMap<String, Valor>>>,
        metodos: HashMap<String, FuncInfo>,
    },
    Task {
        id: usize,
        status: TaskStatus,
        result: Option<Box<Valor>>,
    },
}

#[derive(Clone, Debug)]
pub(crate) enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed(String),
}

//Informações da classe
#[derive(Clone, Debug)]
pub(crate) struct ClasseInfo {
    pub(crate) nome: String,
    pub(crate) campos: Vec<String>,
    pub(crate) metodos: HashMap<String, FuncInfo>,
    pub(crate) campos_estaticos: Rc<RefCell<HashMap<String, Valor>>>,
    pub(crate) metodos_estaticos: HashMap<String, FuncInfo>,
    pub(crate) construtor: Option<Vec<String>>,
    pub(crate) nome_classe_pai: Option<String>, // Adicionado para herança
    pub(crate) construtor_params: Vec<String>,
    pub(crate) base_construtor_args: Vec<String>,
    pub(crate) constructor_body: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FuncInfo {
    pub(crate) nome: String,
    pub(crate) parametros: Vec<String>,
    pub(crate) corpo: Vec<String>,
}

// Implementa como um `Valor` deve ser exibido para o usuário (usado no `PRINT`).
impl fmt::Display for Valor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Valor::Inteiro(n) => write!(f, "{}", n),
            Valor::Flutuante(x) => write!(f, "{:.6}", *x as f64),
            Valor::Duplo(x) => write!(f, "{:.6}", x),
            Valor::Texto(s) => write!(f, "{}", s),
            Valor::Booleano(b) => write!(f, "{}", if *b { "verdadeiro" } else { "falso" }),
            Valor::Decimal(d) => write!(f, "{}", d),
            Valor::Nulo => write!(f, "nulo"),
            Valor::Array(v) => {
                let s = v
                    .iter()
                    .map(|x| x.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "[{}]", s)
            }

            //Display para objetos
            Valor::Objeto {
                nome_classe,
                campos,
                ..
            } => {
                let campos_ref = campos.borrow();
                if let Some(nome) = campos_ref.get("Nome") {
                    write!(f, "{}", nome)
                } else {
                    write!(f, "Objeto<{}>", nome_classe)
                }
            }
            Valor::Task { id, status, result } => match result {
                Some(val) => write!(f, "Task<{}>: {:?} = {}", id, status, val),
                None => write!(f, "Task<{}>: {:?}", id, status),
            },
        }
    }
}

//Implementação manual de PartialEq para lidar com Rc<RefCell<...>>
impl PartialEq for Valor {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Valor::Inteiro(a), Valor::Inteiro(b)) => a == b,
            (Valor::Flutuante(a), Valor::Flutuante(b)) => a == b,
            (Valor::Duplo(a), Valor::Duplo(b)) => a == b,
            (Valor::Texto(a), Valor::Texto(b)) => a == b,
            (Valor::Booleano(a), Valor::Booleano(b)) => a == b,
            (Valor::Decimal(a), Valor::Decimal(b)) => a == b,
            (Valor::Nulo, Valor::Nulo) => true,
            (Valor::Array(a), Valor::Array(b)) => a == b,
            (Valor::Objeto { campos: a, .. }, Valor::Objeto { campos: b, .. }) => {
                // Compara os ponteiros dos `Rc` para verificar se são a mesma instância.
                Rc::ptr_eq(a, b)
            }
            _ => false, // Tipos diferentes não são iguais.
        }
    }
}

// A Máquina Virtual (VM) que executa o bytecode.
pub(crate) struct VM {
    // pilha, variaveis...

    // A pilha de valores para operações.
    pub(crate) pilha: Vec<Valor>,
    // Armazena as variáveis globais.
    pub(crate) variaveis: HashMap<String, Valor>,
    // O bytecode a ser executado.
    pub(crate) bytecode: Vec<String>,
    // Ponteiro da instrução atual (Instruction Pointer).
    pub(crate) ip: usize,
    // Registro de classes
    pub(crate) classes: HashMap<String, ClasseInfo>,
    pub(crate) functions: HashMap<String, FuncInfo>,
    // Rastreia módulos para evitar cargas duplicadas
    pub(crate) loaded_modules: std::collections::HashSet<String>,
    // NOVO: Diretório base para resolver caminhos de módulos
    pub(crate) base_dir: std::path::PathBuf,
    // Debugging support
    pub(crate) debug: Option<Rc<RefCell<DebugState>>>,
    pub(crate) code_id: String,
    // Gerenciador de tasks
    pub(crate) task_counter: Arc<Mutex<usize>>,
    pub(crate) tasks: Arc<Mutex<HashMap<usize, Task>>>,
    // Call stack para debugging
    pub(crate) call_stack: Vec<StackFrame>,
}

// Estrutura para representar uma task assíncrona.
// Pode estar pendente (Pending), em execução (Running), concluída (Completed)
// ou falhada (Failed) com mensagem de erro.
pub(crate) struct Task {
    pub(crate) status: TaskStatus,
    pub(crate) result: Option<Box<Valor>>,
}

// Frame de chamada para stack trace
#[derive(Debug, Clone)]
pub(crate) struct StackFrame {
    pub(crate) code_id: String,
    pub(crate) ip: usize,
    pub(crate) variaveis: HashMap<String, Valor>,
}

// Estado compartilhado do depurador entre VMs (para permitir step-into em chamadas)
#[derive(Debug)]
pub(crate) struct DebugState {
    pub(crate) enabled: bool,
    // breakpoints por código: code_id -> conjunto de IPs
    pub(crate) breakpoints: HashMap<String, std::collections::HashSet<usize>>,
    // modo de passo atual
    pub(crate) step_mode: Option<StepMode>,
    // última localização em que paramos (para comparar no step)
    pub(crate) last_break_location: Option<(String, usize)>,
    // profundidade da call stack para step over/step out
    pub(crate) call_depth: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StepMode {
    StepInto,
    StepOver,
    StepOut,
}
