use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::env;
use std::fmt;
use std::fs;
use std::io::{self, Read, Write};
use std::rc::Rc;

use rust_decimal::Decimal;

// Disponibiliza o JIT da crate de biblioteca quando a feature estiver ativa
#[cfg(feature = "jit")]
use compilador_portugues::jit::CraneliftJit;

//cargo run --bin compilador -- teste.pr --target=bytecode
//cargo run --bin interpretador -- teste.pbc

// Enum para representar os diferentes tipos de valores que a nossa VM pode manipular.
#[derive(Clone, Debug)]
enum Valor {
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
}

// ✅ NOVO: Informações da classe
#[derive(Clone, Debug)]
struct ClasseInfo {
    nome: String,
    campos: Vec<String>,
    metodos: HashMap<String, FuncInfo>,
    campos_estaticos: Rc<RefCell<HashMap<String, Valor>>>,
    metodos_estaticos: HashMap<String, FuncInfo>,
    construtor: Option<Vec<String>>,
    nome_classe_pai: Option<String>, // Adicionado para herança
    construtor_params: Vec<String>,
    base_construtor_args: Vec<String>,
    constructor_body: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
struct FuncInfo {
    nome: String,
    parametros: Vec<String>,
    corpo: Vec<String>,
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

            // ✅ NOVO: Display para objetos
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
        }
    }
}

// ✅ NOVO: Implementação manual de PartialEq para lidar com Rc<RefCell<...>>
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
struct VM {
    // pilha, variaveis...

    // A pilha de valores para operações.
    pilha: Vec<Valor>,
    // Armazena as variáveis globais.
    variaveis: HashMap<String, Valor>,
    // O bytecode a ser executado.
    bytecode: Vec<String>,
    // Ponteiro da instrução atual (Instruction Pointer).
    ip: usize,
    // Registro de classes
    classes: HashMap<String, ClasseInfo>,
    functions: HashMap<String, FuncInfo>,
    // Rastreia módulos para evitar cargas duplicadas
    loaded_modules: std::collections::HashSet<String>,
    // NOVO: Diretório base para resolver caminhos de módulos
    base_dir: std::path::PathBuf,
    // Debugging support
    debug: Option<Rc<RefCell<DebugState>>>,
    code_id: String,
}

// Estado compartilhado do depurador entre VMs (para permitir step-into em chamadas)
#[derive(Debug)]
struct DebugState {
    enabled: bool,
    // breakpoints por código: code_id -> conjunto de IPs
    breakpoints: HashMap<String, std::collections::HashSet<usize>>,
    // modo de passo atual
    step_mode: Option<StepMode>,
    // última localização em que paramos (para comparar no step)
    last_break_location: Option<(String, usize)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StepMode {
    StepInto,
}

impl VM {
    fn executar_funcao(
        &mut self,
        func: &FuncInfo,
        args: Vec<Valor>,
        este: Option<Valor>,
    ) -> Result<Option<Valor>, String> {
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
        child.run()?;
        Ok(child.pilha.pop())
    }

    // Cria uma nova instância da VM com o bytecode fornecido.
    fn new(bytecode: Vec<String>, base_dir: std::path::PathBuf) -> Self {
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
        }
    }

    fn criar_objeto(&mut self, nome_classe: &str, argumentos: Vec<Valor>) -> Result<Valor, String> {
        let classe_info = self
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
                classes: self.classes.clone(),
                functions: self.functions.clone(),
                loaded_modules: self.loaded_modules.clone(),
                base_dir: self.base_dir.clone(),
                debug: self.debug.clone(),
                code_id: format!("ctor:{}", nome_classe),
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

            constructor_vm.run()?;
        }

        Ok(objeto)
    }

    fn chamar_metodo(
        &mut self,
        objeto: &mut Valor,
        nome_metodo: &str,
        argumentos: Vec<Valor>,
    ) -> Result<Valor, String> {
        if let Valor::Texto(s) = objeto {
            if nome_metodo == "comprimento" {
                return Ok(Valor::Inteiro(s.len() as i64));
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
                if let Some(class_info) = self.classes.get(&c_name) {
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
                    classes: self.classes.clone(),
                    functions: self.functions.clone(),
                    loaded_modules: self.loaded_modules.clone(),
                    base_dir: self.base_dir.clone(),
                    debug: self.debug.clone(),
                    code_id: format!("method:{}::{}", nome_classe, nome_metodo),
                };

                vm_metodo.run()?;

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

    fn chamar_metodo_estatico(
        &mut self,
        nome_classe: &str,
        nome_metodo: &str,
        argumentos: Vec<Valor>,
    ) -> Result<Valor, String> {
        if let Some(classe_info) = self.classes.get(nome_classe) {
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
                    classes: self.classes.clone(),
                    functions: self.functions.clone(),
                    loaded_modules: self.loaded_modules.clone(),
                    base_dir: self.base_dir.clone(),
                    debug: self.debug.clone(),
                    code_id: format!("static:{}::{}", nome_classe, nome_metodo),
                };

                vm_metodo.run()?;
                return Ok(vm_metodo.pilha.pop().unwrap_or(Valor::Nulo));
            } else {
                Err(format!(
                    "Método estático \"'{}.{}'\" não encontrado",
                    nome_classe, nome_metodo
                ))
            }
        } else {
            Err(format!("Classe \"{}\" não encontrada", nome_classe))
        }
    }

    // Analisa uma definição de função a partir do bytecode.
    fn parse_definicao_funcao(&self, start_index: usize) -> Result<(FuncInfo, usize), String> {
        let def_line = &self.bytecode[start_index];
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
        while i < self.bytecode.len() && !self.bytecode[i].starts_with("END_FUNCTION") {
            corpo.push(self.bytecode[i].clone());
            i += 1;
        }

        let func_info = FuncInfo {
            nome,
            parametros,
            corpo,
        };
        Ok((func_info, i - start_index))
    }

    fn carregar_definicoes(&mut self) -> Result<(), String> {
        let mut i = 0;
        while i < self.bytecode.len() {
            let instrucao = self.bytecode[i].clone();
            let partes: Vec<&str> = instrucao.split_whitespace().collect();
            let op = partes.get(0).unwrap_or(&"");

            match *op {
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
                        if let Some(parent_info) = self.classes.get(parent_name) {
                            let mut inherited_campos = parent_info.campos.clone();
                            inherited_campos.extend(campos);
                            inherited_campos
                        } else {
                            campos
                        }
                    } else {
                        campos
                    };

                    self.classes.insert(
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
                        partes.iter().skip(3).map(|s| s.to_string()).collect();
                    let corpo_inicio = i + 1;
                    let corpo_fim = corpo_inicio + tamanho;
                    if corpo_fim > self.bytecode.len() {
                        return Err("Bytecode truncado em DEFINE_FUNCTION".into());
                    }
                    let corpo = self.bytecode[corpo_inicio..corpo_fim].to_vec();
                    self.functions.insert(
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
                    let tamanho: usize = partes
                        .get(3)
                        .ok_or("DEFINE_METHOD requer tamanho")?
                        .parse()
                        .map_err(|_| "Tamanho inválido")?;
                    let parametros: Vec<String> =
                        partes.iter().skip(4).map(|s| s.to_string()).collect();
                    let corpo_inicio = i + 1;
                    let corpo_fim = corpo_inicio + tamanho;
                    if corpo_fim > self.bytecode.len() {
                        return Err("Bytecode truncado em DEFINE_METHOD".into());
                    }
                    let corpo = self.bytecode[corpo_inicio..corpo_fim].to_vec();
                    let metodo_info = FuncInfo {
                        nome: metodo_nome.clone(),
                        parametros,
                        corpo,
                    };
                    let entry = self
                        .classes
                        .entry(classe_nome.clone())
                        .or_insert(ClasseInfo {
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
                    let tamanho: usize = partes
                        .get(3)
                        .ok_or("DEFINE_STATIC_METHOD requer tamanho")?
                        .parse()
                        .map_err(|_| "Tamanho inválido")?;
                    let parametros: Vec<String> =
                        partes.iter().skip(4).map(|s| s.to_string()).collect();
                    let corpo_inicio = i + 1;
                    let corpo_fim = corpo_inicio + tamanho;
                    if corpo_fim > self.bytecode.len() {
                        return Err("Bytecode truncado em DEFINE_STATIC_METHOD".into());
                    }
                    let corpo = self.bytecode[corpo_inicio..corpo_fim].to_vec();
                    let metodo_info = FuncInfo {
                        nome: metodo_nome.clone(),
                        parametros,
                        corpo,
                    };
                    let entry = self
                        .classes
                        .entry(classe_nome.clone())
                        .or_insert(ClasseInfo {
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

    // O laço principal de execução da VM.
    fn run(&mut self) -> Result<(), String> {
        while self.ip < self.bytecode.len() {
            let instrucao_str = self.bytecode[self.ip].clone();
            // Divide a instrução em partes (ex: "LOAD_CONST_INT", "42")
            let partes: Vec<&str> = instrucao_str.split_whitespace().collect();
            let op = partes.get(0).ok_or("Instrução vazia encontrada")?;

            // Ponto de parada para debug antes de executar a instrução
            self.debug_pause_if_needed(&instrucao_str)?;

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
                            self.pilha.push(Valor::Inteiro(a + b))
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
                            self.pilha.push(Valor::Inteiro(a - b))
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
                            self.pilha.push(Valor::Inteiro(a * b))
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
                            self.pilha.push(Valor::Inteiro(a / b));
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
                        Valor::Objeto { campos, .. } => {
                            let valor = campos
                                .borrow()
                                .get(*nome_propriedade)
                                .cloned()
                                .unwrap_or(Valor::Nulo);
                            self.pilha.push(valor);
                        }
                        _ => return Err("GET_PROPERTY requer um objeto".to_string()),
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
                    let valor_retorno = self.chamar_metodo(&mut objeto, nome_metodo, argumentos)?;
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

                    let resultado =
                        self.chamar_metodo_estatico(nome_classe, nome_metodo, argumentos)?;
                    self.pilha.push(resultado);
                }

                "SET_DEFAULT" => {
                    let nome_var = partes
                        .get(1)
                        .ok_or("SET_DEFAULT requer um nome de variável")?;
                    if !self.variaveis.contains_key(*nome_var) {
                        let default_expr_bytecode_str = partes[2..].join(" ");
                        let mut temp_vm =
                            VM::new(vec![default_expr_bytecode_str], self.base_dir.clone());
                        temp_vm.debug = self.debug.clone();
                        temp_vm.code_id = format!("expr-default:{}", nome_var);
                        temp_vm.run()?;
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
                                        constructor_vm.run()?;
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
                    };
                    vm.run()?;
                    self.pilha.push(vm.pilha.pop().unwrap_or(Valor::Nulo));
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

    fn executar_codigo_global(&mut self) -> Result<(), String> {
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
            } else if instrucao.starts_with("DEFINE_FUNCTION") {
                // Pula a definição e seu corpo
                let partes: Vec<&str> = instrucao.split(' ').collect();
                let tamanho_str = if partes[0] == "DEFINE_CLASS" {
                    "0"
                } else {
                    partes.get(2).unwrap_or(&"0")
                };
                let tamanho: usize = tamanho_str.parse().unwrap_or(0);
                i += tamanho + 1;
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
        };

        vm_global.run()
    }

    fn run_apenas_inicializadores(&mut self) -> Result<(), String> {
        while self.ip < self.bytecode.len() {
            let instrucao_str = self.bytecode[self.ip].clone();
            let partes: Vec<&str> = instrucao_str.split_whitespace().collect();
            let op = partes.get(0).ok_or("Instrução vazia encontrada")?;

            self.ip += 1;

            match *op {
                "LOAD_CONST_STR" | "LOAD_CONST_INT" | "LOAD_CONST_BOOL" | "LOAD_CONST_NULL"
                | "LOAD_CONST_FLOAT" | "LOAD_CONST_DOUBLE" => {
                    // Executa apenas as instruções de carregamento de constantes
                    // (Reciclando a lógica do `run` principal)
                    match *op {
                        "LOAD_CONST_STR" => {
                            let valor = partes[1..].join(" ");
                            self.pilha
                                .push(Valor::Texto(valor.trim_matches('"').to_string()));
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

    fn debug_pause_if_needed(&mut self, instr: &str) -> Result<(), String> {
        let Some(dbg_rc) = self.debug.clone() else {
            return Ok(());
        };
        let mut st = dbg_rc.borrow_mut();
        if !st.enabled {
            return Ok(());
        }

        let mut should_pause = matches!(st.step_mode, Some(StepMode::StepInto));
        if !should_pause {
            if let Some(bps) = st.breakpoints.get(&self.code_id) {
                // Para instruções não-JUMP, ip já foi incrementado no loop run
                let cur_ip = self.ip.saturating_sub(1);
                if bps.contains(&cur_ip) {
                    should_pause = true;
                }
            }
        }
        if !should_pause {
            return Ok(());
        }

        st.last_break_location = Some((self.code_id.clone(), self.ip.saturating_sub(1)));
        drop(st);

        loop {
            println!(
                "\n[depurador] {}@ip={} -> {}\ncomandos: c(continue), s(step), p(pilha), vars, v <nome>, dis [n], bp add|del <ip>|list, bp add|del <code_id> <ip>, bp list [code_id], where, help, q(quit)",
                self.code_id, self.ip.saturating_sub(1), instr
            );
            print!("dbg> ");
            io::stdout().flush().ok();
            let mut entrada = String::new();
            io::stdin()
                .read_line(&mut entrada)
                .map_err(|e| e.to_string())?;
            let cmd = entrada.trim();
            if cmd.is_empty() || cmd == "c" || cmd == "cont" || cmd == "continue" {
                if let Some(d) = &self.debug {
                    d.borrow_mut().step_mode = None;
                }
                break;
            } else if cmd == "s" || cmd == "step" || cmd == "next" || cmd == "n" {
                if let Some(d) = &self.debug {
                    d.borrow_mut().step_mode = Some(StepMode::StepInto);
                }
                break;
            } else if cmd == "p" || cmd == "pilha" {
                println!("pilha ({} itens):", self.pilha.len());
                for (i, v) in self.pilha.iter().enumerate() {
                    println!("  [{}] {}", i, v);
                }
            } else if cmd == "vars" {
                println!("variaveis ({}):", self.variaveis.len());
                for (k, v) in &self.variaveis {
                    println!("  {} = {}", k, v);
                }
            } else if cmd.starts_with("v ") {
                let nome = cmd.splitn(2, ' ').nth(1).unwrap_or("");
                if let Some(v) = self.variaveis.get(nome) {
                    println!("{} = {}", nome, v);
                } else {
                    println!("(sem variável '{}')", nome);
                }
            } else if cmd.starts_with("dis") {
                let parts: Vec<&str> = cmd.split_whitespace().collect();
                let n: usize = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(8);
                let start = self.ip.saturating_sub(1);
                let end = (start + n).min(self.bytecode.len());
                for i in start..end {
                    let mark = if i + 1 == self.ip { "->" } else { "  " };
                    println!("{} {:04}: {}", mark, i, self.bytecode[i]);
                }
            } else if cmd.starts_with("bp ") {
                let parts: Vec<&str> = cmd.split_whitespace().collect();
                if parts.len() >= 2 {
                    match parts[1] {
                        // bp add <ip>
                        "add" if parts.len() == 3 => {
                            if let Ok(ip) = parts[2].parse::<usize>() {
                                if let Some(d) = &self.debug { let mut s = d.borrow_mut(); let set = s.breakpoints.entry(self.code_id.clone()).or_insert_with(HashSet::new); set.insert(ip); }
                                println!("Breakpoint adicionado em {}:{}", self.code_id, ip);
                            } else { println!("ip inválido"); }
                        }
                        // bp add <code_id> <ip>
                        "add" if parts.len() >= 4 => {
                            let code_id = parts[2].to_string();
                            if let Ok(ip) = parts[3].parse::<usize>() {
                                if let Some(d) = &self.debug { let mut s = d.borrow_mut(); let set = s.breakpoints.entry(code_id.clone()).or_insert_with(HashSet::new); set.insert(ip); }
                                println!("Breakpoint adicionado em {}:{}", code_id, ip);
                            } else { println!("ip inválido"); }
                        }
                        // bp del <ip>
                        "del" if parts.len() == 3 => {
                            if let Ok(ip) = parts[2].parse::<usize>() {
                                if let Some(d) = &self.debug { let mut s = d.borrow_mut(); if let Some(set) = s.breakpoints.get_mut(&self.code_id) { set.remove(&ip); } }
                                println!("Breakpoint removido em {}:{}", self.code_id, ip);
                            } else { println!("ip inválido"); }
                        }
                        // bp del <code_id> <ip>
                        "del" if parts.len() >= 4 => {
                            let code_id = parts[2].to_string();
                            if let Ok(ip) = parts[3].parse::<usize>() {
                                if let Some(d) = &self.debug { let mut s = d.borrow_mut(); if let Some(set) = s.breakpoints.get_mut(&code_id) { set.remove(&ip); } }
                                println!("Breakpoint removido em {}:{}", code_id, ip);
                            } else { println!("ip inválido"); }
                        }
                        // bp list [code_id]
                        "list" => {
                            let target = if parts.len() >= 3 { parts[2] } else { &self.code_id };
                            if let Some(d) = &self.debug { let s = d.borrow(); if let Some(set) = s.breakpoints.get(target) { println!("breakpoints em {}: {:?}", target, set); } else { println!("sem breakpoints em {}", target); } }
                        }
                        _ => println!("uso: bp add <ip> | bp add <code_id> <ip> | bp del <ip> | bp del <code_id> <ip> | bp list [code_id]"),
                    }
                } else {
                    println!("uso: bp add <ip> | bp add <code_id> <ip> | bp del <ip> | bp del <code_id> <ip> | bp list [code_id]");
                }
            } else if cmd == "where" {
                println!(
                    "em {} ip={} -> {}",
                    self.code_id,
                    self.ip.saturating_sub(1),
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
}

// Ponto de entrada do programa interpretador.
fn main() -> Result<(), Box<dyn std::error::Error>> {
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

    // ✅ NOVO: Obter o diretório base do arquivo de bytecode.
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
        };
        vm.debug = Some(Rc::new(RefCell::new(dbg)));
    }

    // Carregar definições (classes, funções)
    if let Err(e) = vm.carregar_definicoes() {
        eprintln!("Erro ao carregar definições: {}", e);
        return Err(e.into());
    }

    // Fase 2: Executar inicializadores de propriedades estáticas
    if let Err(e) = vm.run_apenas_inicializadores() {
        eprintln!("Erro em inicializadores: {}", e);
        return Err(e.into());
    }

    // Fase 3: Executar código global (funções main, etc.)
    if let Err(e) = vm.executar_codigo_global() {
        eprintln!("Erro ao executar código de inicialização: {}", e);
        return Err(e.into());
    }

    // Fase 4: Encontrar e executar a função especificada ou 'Principal'
    let func_to_run = if let Some(func_name) = function_to_execute {
        Some(func_name)
    } else {
        vm.functions
            .keys()
            .find(|nome| nome.ends_with("Principal") || nome == &&"Principal".to_string())
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
        };

        if let Err(e) = main_vm.run() {
            eprintln!("❌ Erro na execução da função {}: {}", nome_funcao, e);
            return Err(e.into());
        }
    }

    Ok(())
}

// ✅ NOVO: Função auxiliar para ler o bytecode do arquivo.
fn ler_bytecode(caminho_arquivo: &str) -> Result<Vec<String>, std::io::Error> {
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
