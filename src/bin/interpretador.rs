use std::collections::HashMap;
use std::env;
use std::fmt;
use std::fs;
use std::io::{self, Read};

//cargo run --bin compilador -- teste.pr --target=bytecode
//cargo run --bin interpretador -- teste.pbc

// Enum para representar os diferentes tipos de valores que a nossa VM pode manipular.
#[derive(Clone, Debug, PartialEq)]
enum Valor {
    Inteiro(i64),
    Texto(String),
    Booleano(bool),
    Nulo,
    Objeto {
        classe: String,
        propriedades: HashMap<String, Valor>,
        metodos: HashMap<String, Vec<String>>,
    },
}

// ✅ NOVO: Informações da classe
#[derive(Clone, Debug)]
struct ClasseInfo {
    nome: String,
    propriedades: Vec<String>,
    metodos: HashMap<String, Vec<String>>, // nome -> bytecode
    construtor: Option<Vec<String>>,       // bytecode do construtor
}

#[derive(Clone, Debug)]
struct FuncInfo {
    params: Vec<String>,
    corpo: Vec<String>,
}

// Implementa como um `Valor` deve ser exibido para o usuário (usado no `PRINT`).
impl fmt::Display for Valor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Valor::Inteiro(n) => write!(f, "{}", n),
            Valor::Texto(s) => write!(f, "{}", s),
            Valor::Booleano(b) => write!(f, "{}", if *b { "verdadeiro" } else { "falso" }),
            Valor::Nulo => write!(f, "nulo"),

            // ✅ NOVO: Display para objetos
            Valor::Objeto {
                classe,
                propriedades,
                ..
            } => {
                if let Some(nome) = propriedades.get("Nome") {
                    write!(f, "{}", nome)
                } else {
                    write!(f, "Objeto<{}>", classe)
                }
            }
        }
    }
}

// A Máquina Virtual (VM) que executa o bytecode.
struct VM {
    // A pilha de valores para operações.
    pilha: Vec<Valor>,
    // Armazena as variáveis globais.
    variaveis: HashMap<String, Valor>,
    // O bytecode a ser executado.
    bytecode: Vec<String>,
    // Ponteiro da instrução atual (Instruction Pointer).
    ip: usize,
    // ✅ NOVO: Registro de classes
    classes: HashMap<String, ClasseInfo>,
    functions: HashMap<String, FuncInfo>,
}

impl VM {
    // Cria uma nova instância da VM com o bytecode fornecido.
    fn new(bytecode: Vec<String>) -> Self {
        Self {
            pilha: Vec::new(),
            variaveis: HashMap::new(),
            bytecode,
            ip: 0,
            classes: HashMap::new(),
            functions: HashMap::new(),
        }
    }

    fn criar_objeto(&self, nome_classe: &str, argumentos: Vec<Valor>) -> Result<Valor, String> {
        let classe = self
            .classes
            .get(nome_classe)
            .ok_or_else(|| format!("Classe '{}' não encontrada", nome_classe))?;

        let mut propriedades = HashMap::new();

        // Inicializar propriedades com valores padrão
        for (i, prop_nome) in classe.propriedades.iter().enumerate() {
            let valor = argumentos.get(i).cloned().unwrap_or(Valor::Nulo);
            propriedades.insert(prop_nome.clone(), valor);
        }

        Ok(Valor::Objeto {
            classe: nome_classe.to_string(),
            propriedades,
            metodos: classe.metodos.clone(),
        })
    }

    fn chamar_metodo(
        &mut self,
        objeto: Valor,
        nome_metodo: &str,
        _argumentos: Vec<Valor>,
    ) -> Result<Valor, String> {
        match objeto {
            Valor::Objeto {
                classe,
                propriedades,
                metodos,
            } => {
                if let Some(corpo) = metodos.get(nome_metodo) {
                    // --- prepara o ambiente do método ---------------------------
                    // 1. variáveis locais contendo o objeto como "este"
                    let mut vars = HashMap::new();
                    vars.insert(
                        "este".to_string(),
                        Valor::Objeto {
                            classe: classe.clone(),
                            propriedades: propriedades.clone(),
                            metodos: metodos.clone(),
                        },
                    );

                    // 2. mini-VM que executará o corpo do método
                    let mut vm = VM {
                        pilha: Vec::new(), // pilha inicia vazia
                        variaveis: vars,   // "este" resolvido por LOAD_VAR
                        bytecode: corpo.clone(),
                        ip: 0,
                        classes: self.classes.clone(),
                        functions: self.functions.clone(),
                    };

                    vm.run()?; // executa método
                               // valor de retorno ou nulo
                    Ok(vm.pilha.pop().unwrap_or(Valor::Nulo))
                } else {
                    Err(format!(
                        "Método '{}.{}' não encontrado",
                        classe, nome_metodo
                    ))
                }
            }
            _ => Err("Tentativa de chamar método em não-objeto".to_string()),
        }
    }

    // O laço principal de execução da VM.
    fn run(&mut self) -> Result<(), String> {
        while self.ip < self.bytecode.len() {
            let instrucao_str = self.bytecode[self.ip].clone();
            // Divide a instrução em partes (ex: "LOAD_CONST_INT", "42")
            let partes: Vec<&str> = instrucao_str.split_whitespace().collect();
            let op = partes.get(0).ok_or("Instrução vazia encontrada")?;

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
                "LOAD_CONST_STR" => {
                    // Junta as partes da string, removendo as aspas.
                    let valor = partes[1..].join(" ");
                    self.pilha
                        .push(Valor::Texto(valor.trim_matches('"').to_string()));
                }
                "LOAD_VAR" => {
                    let nome_var = partes.get(1).ok_or("LOAD_VAR requer um nome de variável")?;
                    let valor = self
                        .variaveis
                        .get(*nome_var)
                        .cloned()
                        .unwrap_or(Valor::Nulo);
                    self.pilha.push(valor);
                }
                "STORE_VAR" => {
                    let nome_var = partes
                        .get(1)
                        .ok_or("STORE_VAR requer um nome de variável")?;
                    let valor = self.pilha.pop().ok_or("Pilha vazia em STORE_VAR")?;
                    self.variaveis.insert(nome_var.to_string(), valor);
                }
                "PRINT" => {
                    let valor = self.pilha.pop().ok_or("Pilha vazia em PRINT")?;
                    println!("{}", valor);
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

                "LOAD_CONST_BOOL" => {
                    let valor = partes
                        .get(1)
                        .ok_or("LOAD_CONST_BOOL requer um argumento")?
                        .parse::<bool>()
                        .map_err(|e| format!("Valor inválido para LOAD_CONST_BOOL: {}", e))?;
                    self.pilha.push(Valor::Booleano(valor));
                }

                "ADD" => {
                    let dir = self.pilha.pop().ok_or("Pilha vazia para ADD")?;
                    let esq = self.pilha.pop().ok_or("Pilha vazia para ADD")?;
                    match (esq, dir) {
                        (Valor::Inteiro(a), Valor::Inteiro(b)) => {
                            self.pilha.push(Valor::Inteiro(a + b))
                        }
                        _ => return Err("Tipos incompatíveis para ADD".to_string()),
                    }
                }
                "SUB" => {
                    let dir = self.pilha.pop().ok_or("Pilha vazia para SUB")?;
                    let esq = self.pilha.pop().ok_or("Pilha vazia para SUB")?;
                    match (esq, dir) {
                        (Valor::Inteiro(a), Valor::Inteiro(b)) => {
                            self.pilha.push(Valor::Inteiro(a - b))
                        }
                        _ => return Err("Tipos incompatíveis para SUB".to_string()),
                    }
                }
                "MUL" => {
                    let dir = self.pilha.pop().ok_or("Pilha vazia para MUL")?;
                    let esq = self.pilha.pop().ok_or("Pilha vazia para MUL")?;
                    match (esq, dir) {
                        (Valor::Inteiro(a), Valor::Inteiro(b)) => {
                            self.pilha.push(Valor::Inteiro(a * b))
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
                            self.pilha.push(Valor::Inteiro(a / b));
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
                            self.pilha.push(Valor::Inteiro(a % b));
                        }
                        _ => return Err("Tipos incompatíveis para MOD".to_string()),
                    }
                }
                "NEGATE_INT" => {
                    //Negação numérica
                    let val = self.pilha.pop().ok_or("Pilha vazia para NEGATE_INT")?;
                    match val {
                        Valor::Inteiro(n) => self.pilha.push(Valor::Inteiro(-n)),
                        _ => return Err("Tipo incompatível para NEGATE_INT".to_string()),
                    }
                }
                "NEGATE_BOOL" => {
                    //Negação lógica
                    let val = self.pilha.pop().ok_or("Pilha vazia para NEGATE_BOOL")?;
                    match val {
                        Valor::Booleano(b) => self.pilha.push(Valor::Booleano(!b)),
                        _ => return Err("Tipo incompatível para NEGATE_BOOL".to_string()),
                    }
                }

                // Operações de Comparação (para inteiros e booleanos)
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
                    self.ip = target_ip;
                }
                "JUMP_IF_FALSE" => {
                    // Salto condicional
                    let target_ip: usize = partes
                        .get(1)
                        .ok_or("JUMP_IF_FALSE requer um endereço de destino")?
                        .parse()
                        .map_err(|e| format!("Endereço inválido para JUMP_IF_FALSE: {}", e))?;
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
                "DEFINE_CLASS" => {
                    let nome_classe = partes.get(1).ok_or("DEFINE_CLASS requer nome da classe")?;
                    let propriedades: Vec<String> = if partes.len() > 2 {
                        partes[2..].iter().map(|s| s.to_string()).collect()
                    } else {
                        Vec::new()
                    };

                    self.classes.insert(
                        nome_classe.to_string(),
                        ClasseInfo {
                            nome: nome_classe.to_string(),
                            propriedades,
                            metodos: HashMap::new(),
                            construtor: None,
                        },
                    );
                }
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
                    let objeto = self.criar_objeto(nome_classe, argumentos)?;
                    self.pilha.push(objeto);
                }

                "GET_PROPERTY" => {
                    let nome_propriedade = partes
                        .get(1)
                        .ok_or("GET_PROPERTY requer nome da propriedade")?;
                    let objeto = self.pilha.pop().ok_or("Pilha vazia para GET_PROPERTY")?;

                    match objeto {
                        Valor::Objeto { propriedades, .. } => {
                            let valor = propriedades
                                .get(*nome_propriedade)
                                .cloned()
                                .unwrap_or(Valor::Nulo);
                            self.pilha.push(valor);
                        }
                        _ => return Err("GET_PROPERTY requer um objeto".to_string()),
                    }
                }

                "SET_PROPERTY" => {
                    let nome_propriedade = partes
                        .get(1)
                        .ok_or("SET_PROPERTY requer nome da propriedade")?;

                    // 1. Desempilha o OBJETO (que está no topo da pilha).
                    let mut objeto = self
                        .pilha
                        .pop()
                        .ok_or("Pilha vazia para objeto em SET_PROPERTY")?;

                    // 2. Desempilha o VALOR (que está abaixo do objeto).
                    let valor = self
                        .pilha
                        .pop()
                        .ok_or("Pilha vazia para valor em SET_PROPERTY")?;

                    // Agora 'objeto' contém o objeto e 'valor' contém o valor, como esperado.
                    match &mut objeto {
                        Valor::Objeto { propriedades, .. } => {
                            // Modifica as propriedades do objeto diretamente.
                            propriedades.insert(nome_propriedade.to_string(), valor);
                        }
                        _ => {
                            // Este erro agora será acionado corretamente se a pilha não contiver um objeto no topo.
                            return Err(
                                "SET_PROPERTY requer um objeto no topo da pilha".to_string()
                            );
                        }
                    }

                    // Devolve o objeto modificado para a pilha.
                    // Isso mantém a VM em um estado consistente e permite futuras atribuições encadeadas.
                    self.pilha.push(objeto);
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
                    let objeto = self
                        .pilha
                        .pop()
                        .ok_or("Objeto não encontrado para CALL_METHOD")?;

                    let resultado = self.chamar_metodo(objeto, nome_metodo, argumentos)?;
                    self.pilha.push(resultado);
                }

                "POP" => {
                    // Remove o valor do topo da pilha.
                    // O uso de .ok_or previne um pânico se a pilha estiver vazia,
                    // transformando-o em um erro controlado.
                    self.pilha.pop().ok_or("Pilha vazia em POP")?;
                }

                "DEFINE_METHOD" => {
                    let classe = partes.get(1).ok_or("DEFINE_METHOD requer classe")?;
                    let nome = partes.get(2).ok_or("DEFINE_METHOD requer nome")?;
                    let n = partes
                        .get(3)
                        .ok_or("DEFINE_METHOD requer tamanho")?
                        .parse::<usize>()
                        .map_err(|_| "Tamanho inválido")?;
                    // pega as N linhas imediatamente após o cabeçalho
                    let corpo = self.bytecode[self.ip..self.ip + n].to_vec();
                    self.ip += n; // pula o corpo no bytecode principal
                    if let Some(info) = self.classes.get_mut(*classe) {
                        info.metodos.insert((*nome).to_string(), corpo);
                    } else {
                        return Err(format!("Classe '{}' não definida", classe));
                    }
                }

                "LOAD_CONST_NULL" => self.pilha.push(Valor::Nulo),

                "RETURN" => {
                    // interrompe a execução do frame atual;
                    // o valor de retorno já está no topo da pilha
                    return Ok(());
                }

                "DEFINE_FUNCTION" => {
                    let nome = partes.get(1).ok_or("DEFINE_FUNCTION requer nome")?;
                    let tam = partes
                        .get(2)
                        .ok_or("DEFINE_FUNCTION requer tamanho")?
                        .parse::<usize>()
                        .map_err(|_| "Tamanho inválido")?;
                    let params: Vec<String> = partes[3..].iter().map(|s| s.to_string()).collect();
                    let corpo = self.bytecode[self.ip..self.ip + tam].to_vec();
                    self.ip += tam;
                    self.functions
                        .insert((*nome).to_string(), FuncInfo { params, corpo });
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
                    let mut args = self.pilha.split_off(self.pilha.len() - nargs);
                    // procura função
                    let func = self
                        .functions
                        .get(*nome)
                        .ok_or_else(|| format!("Função '{}' não definida", nome))?
                        .clone();

                    // cria ambiente local: parametros -> argumentos
                    let mut vars = HashMap::new();
                    for (i, p) in func.params.iter().enumerate() {
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
                        functions: self.functions.clone(), // permite recursão
                    };
                    vm.run()?;
                    let retorno = vm.pilha.pop().unwrap_or(Valor::Nulo);
                    self.pilha.push(retorno);
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
}

// Ponto de entrada do programa interpretador.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Uso: {} <arquivo.pbc>", args[0]);
        return Err("Argumento inválido".into());
    }

    let caminho_arquivo = &args[1];
    let mut arquivo = fs::File::open(caminho_arquivo)?;
    let mut conteudo = String::new();
    arquivo.read_to_string(&mut conteudo)?;

    // Converte o conteúdo do arquivo em uma lista de instruções, ignorando linhas vazias.
    let bytecode: Vec<String> = conteudo
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(String::from)
        .collect();

    //println!("--- Executando Bytecode ---");
    let mut vm = VM::new(bytecode);
    if let Err(e) = vm.run() {
        eprintln!("\n--- Erro de Execução ---");
        eprintln!("{}", e);
        return Err("Execução falhou".into());
    }

    //println!("\n--- Execução Concluída ---");
    Ok(())
}
