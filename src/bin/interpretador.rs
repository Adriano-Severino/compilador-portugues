use std::cell::RefCell;
use std::collections::HashMap;
use std::env;
use std::fmt;
use std::fs;
use std::io::Read;
use std::rc::Rc;

//cargo run --bin compilador -- teste.pr --target=bytecode
//cargo run --bin interpretador -- teste.pbc

// Enum para representar os diferentes tipos de valores que a nossa VM pode manipular.
#[derive(Clone, Debug)]
enum Valor {
    Inteiro(i64),
    Texto(String),
    Booleano(bool),
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
            Valor::Texto(s) => write!(f, "{}", s),
            Valor::Booleano(b) => write!(f, "{}", if *b { "verdadeiro" } else { "falso" }),
            Valor::Nulo => write!(f, "nulo"),

            // ✅ NOVO: Display para objetos
            Valor::Objeto { nome_classe, campos, .. } => {
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
            (Valor::Texto(a), Valor::Texto(b)) => a == b,
            (Valor::Booleano(a), Valor::Booleano(b)) => a == b,
            (Valor::Nulo, Valor::Nulo) => true,
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
    // ✅ NOVO: Rastreia módulos para evitar cargas duplicadas
    loaded_modules: std::collections::HashSet<String>,
    // ✅ NOVO: Diretório base para resolver caminhos de módulos
    base_dir: std::path::PathBuf,
}

impl VM {
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
        }
    }

    let objeto_rc = Rc::new(RefCell::new(campos_map)); // Use a new Rc for the object's fields
        let objeto = Valor::Objeto {
            nome_classe: nome_classe.to_string(),
            campos: objeto_rc.clone(),
            metodos: classe.metodos.clone(),
        };

        // Execute parent constructor if applicable
        if let Some(parent_name) = &classe.nome_classe_pai {
            if let Some(parent_class_info) = self.classes.get(parent_name) {
                let mut parent_constructor_vm = VM {
                    pilha: Vec::new(),
                    variaveis: HashMap::new(),
                    bytecode: parent_class_info.constructor_body.clone(),
                    ip: 0,
                    classes: self.classes.clone(),
                    functions: self.functions.clone(),
                    loaded_modules: self.loaded_modules.clone(),
                    base_dir: self.base_dir.clone(),
                };

                let mut parent_args_values = Vec::new();
                for arg_str in &classe.base_construtor_args {
                    if let Ok(int_val) = arg_str.parse::<i64>() {
                        parent_args_values.push(Valor::Inteiro(int_val));
                    } else if let Ok(bool_val) = arg_str.parse::<bool>() {
                        parent_args_values.push(Valor::Booleano(bool_val));
                    } else if arg_str.starts_with("Texto(") && arg_str.ends_with(")") {
                        parent_args_values.push(Valor::Texto(arg_str[7..arg_str.len()-2].to_string()));
                    } else if arg_str.starts_with("Inteiro(") && arg_str.ends_with(")") {
                        let int_str = &arg_str[8..arg_str.len()-1];
                        if let Ok(int_val) = int_str.parse::<i64>() {
                            parent_args_values.push(Valor::Inteiro(int_val));
                        } else {
                            parent_args_values.push(Valor::Nulo);
                        }
                    } else if arg_str.starts_with("Booleano(") && arg_str.ends_with(")") {
                        let bool_str = &arg_str[9..arg_str.len()-1];
                        if let Ok(bool_val) = bool_str.parse::<bool>() {
                            parent_args_values.push(Valor::Booleano(bool_val));
                        } else {
                            parent_args_values.push(Valor::Nulo);
                        }
                    } else {
                        parent_args_values.push(Valor::Nulo);
                    }
                }

                for (i, param_name) in parent_class_info.construtor_params.iter().enumerate() {
                    if let Some(arg_val) = parent_args_values.get(i) {
                        parent_constructor_vm.variaveis.insert(param_name.clone(), arg_val.clone());
                    }
                }

                parent_constructor_vm.variaveis.insert("este".to_string(), objeto.clone());

                parent_constructor_vm.run()?;
            }
        }

        for (i, param_name) in classe.construtor_params.iter().enumerate() {
            if let Some(arg_val) = argumentos.get(i) {
                objeto_rc.borrow_mut().insert(param_name.clone(), arg_val.clone());
            }
        }

        let mut constructor_vm = VM {
            pilha: Vec::new(),
            variaveis: HashMap::new(),
            bytecode: classe.constructor_body.clone(),
            ip: 0,
            classes: self.classes.clone(),
            functions: self.functions.clone(),
            loaded_modules: self.loaded_modules.clone(),
            base_dir: self.base_dir.clone(),
        };

        for (i, param_name) in classe.construtor_params.iter().enumerate() {
            if let Some(arg_val) = argumentos.get(i) {
                constructor_vm.variaveis.insert(param_name.clone(), arg_val.clone());
            }
        }

        constructor_vm.variaveis.insert("este".to_string(), objeto.clone());

        constructor_vm.run()?;

        Ok(objeto)
    }

    fn chamar_metodo(
        &mut self,
        objeto: &mut Valor,
        nome_metodo: &str,
        argumentos: Vec<Valor>,
    ) -> Result<(), String> {
        if let Valor::Objeto { ref nome_classe, .. } = objeto {
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
                };

                vm_metodo.run()?;

                return Ok(());
            } else {
                Err(format!("Método '{}.{}' não encontrado", nome_classe, nome_metodo))
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
                };

                vm_metodo.run()?;
                return Ok(vm_metodo.pilha.pop().unwrap_or(Valor::Nulo));
            } else {
                Err(format!("Método estático '{}.{}' não encontrado", nome_classe, nome_metodo))
            }
        } else {
            Err(format!("Classe '{}' não encontrada", nome_classe))
        }
    }

    // Analisa uma definição de função a partir do bytecode.
    fn parse_definicao_funcao(&self, start_index: usize) -> Result<(FuncInfo, usize), String> {
        let def_line = &self.bytecode[start_index];
        let partes: Vec<&str> = def_line.split_whitespace().collect();
        if partes.len() < 4 {
            return Err(format!("Instrução DEFINE_FUNCTION malformada: {}", def_line));
        }
        let nome = partes[1].to_string();
        let parametros: Vec<String> = partes[3].split(',').filter(|s| !s.is_empty()).map(String::from).collect();

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
                    let parent_class = if parent_class.as_deref() == Some("NULO") { None } else { parent_class };
                    let props_and_constructor_str = partes.get(3).ok_or("DEFINE_CLASS requer propriedades e parâmetros do construtor")?;
                    let parts: Vec<&str> = props_and_constructor_str.split('|').collect();
                    let campos: Vec<String> = parts.get(0).map_or(Vec::new(), |s| s.split_whitespace().map(String::from).collect());
                    let construtor_params: Vec<String> = parts.get(1).map_or(Vec::new(), |s| s.split_whitespace().map(String::from).collect());
                    let base_construtor_args: Vec<String> = parts.get(2).map_or(Vec::new(), |s| s.split_whitespace().map(String::from).collect());
                    let constructor_body: Vec<String> = parts.get(3).map_or(Vec::new(), |s| s.split(';').filter(|line| !line.trim().is_empty()).map(String::from).collect());

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

                    self.classes.insert(nome_classe.clone(), ClasseInfo {
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
                    });
                    i += 1;
                },
                "DEFINE_FUNCTION" => {
                    let nome_func = partes.get(1).ok_or("DEFINE_FUNCTION requer nome")?.to_string();
                    let tamanho: usize = partes.get(2).ok_or("DEFINE_FUNCTION requer tamanho")?.parse().map_err(|_| "Tamanho inválido")?;
                    let parametros: Vec<String> = partes.iter().skip(3).map(|s| s.to_string()).collect();
                    let corpo_inicio = i + 1;
                    let corpo_fim = corpo_inicio + tamanho;
                    if corpo_fim > self.bytecode.len() {
                        return Err("Bytecode truncado em DEFINE_FUNCTION".into());
                    }
                    let corpo = self.bytecode[corpo_inicio..corpo_fim].to_vec();
                    self.functions.insert(nome_func.clone(), FuncInfo { nome: nome_func, parametros, corpo });
                    i = corpo_fim;
                }
                "DEFINE_METHOD" => {
                    let classe_nome = partes.get(1).ok_or("DEFINE_METHOD requer classe")?.to_string();
                    let metodo_nome = partes.get(2).ok_or("DEFINE_METHOD requer nome")?.to_string();
                    let tamanho: usize = partes.get(3).ok_or("DEFINE_METHOD requer tamanho")?.parse().map_err(|_| "Tamanho inválido")?;
                    let corpo_inicio = i + 1;
                    let corpo_fim = corpo_inicio + tamanho;
                    if corpo_fim > self.bytecode.len() {
                        return Err("Bytecode truncado em DEFINE_METHOD".into());
                    }
                    let corpo = self.bytecode[corpo_inicio..corpo_fim].to_vec();
                    let metodo_info = FuncInfo { nome: metodo_nome.clone(), parametros: Vec::new(), corpo };
                    let entry = self.classes.entry(classe_nome.clone()).or_insert(ClasseInfo {
                        nome: classe_nome.clone(),
                        campos: Vec::new(),
                        metodos: HashMap::new(),
                        campos_estaticos: Rc::new(RefCell::new(HashMap::new())),
                        metodos_estaticos: HashMap::new(),
                        construtor: None,
                        nome_classe_pai: None,
                        construtor_params: Vec::new(),
                    });
                    entry.metodos.insert(metodo_nome, metodo_info);
                    i = corpo_fim;
                }
                "DEFINE_STATIC_METHOD" => {
                    let classe_nome = partes.get(1).ok_or("DEFINE_STATIC_METHOD requer classe")?.to_string();
                    let metodo_nome = partes.get(2).ok_or("DEFINE_STATIC_METHOD requer nome")?.to_string();
                    let tamanho: usize = partes.get(3).ok_or("DEFINE_STATIC_METHOD requer tamanho")?.parse().map_err(|_| "Tamanho inválido")?;
                    let parametros: Vec<String> = partes.iter().skip(4).map(|s| s.to_string()).collect();
                    let corpo_inicio = i + 1;
                    let corpo_fim = corpo_inicio + tamanho;
                    if corpo_fim > self.bytecode.len() {
                        return Err("Bytecode truncado em DEFINE_STATIC_METHOD".into());
                    }
                    let corpo = self.bytecode[corpo_inicio..corpo_fim].to_vec();
                    let metodo_info = FuncInfo { nome: metodo_nome.clone(), parametros, corpo };
                    let entry = self.classes.entry(classe_nome.clone()).or_insert(ClasseInfo {
                        nome: classe_nome.clone(),
                        campos: Vec::new(),
                        metodos: HashMap::new(),
                        campos_estaticos: Rc::new(RefCell::new(HashMap::new())),
                        metodos_estaticos: HashMap::new(),
                        construtor: None,
                        nome_classe_pai: None,
                        construtor_params: Vec::new(),
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
                    let valor = self.variaveis.get(*nome_var)
                        .cloned()
                        // Se não encontrar na pilha local, tenta nos campos de 'este'
                        .or_else(|| {
                            if let Some(Valor::Objeto { campos, .. }) = self.variaveis.get("este") {
                                campos.borrow().get(*nome_var).cloned()
                            } else {
                                None
                            }
                        })
                        .unwrap_or(Valor::Nulo);
                    self.pilha.push(valor);
                }
                "STORE_VAR" => {
                    let nome_var = partes.get(1).ok_or("STORE_VAR requer um nome de variável")?;
                    let valor = self.pilha.pop().ok_or("Pilha vazia em STORE_VAR")?;

                    // Tenta atualizar o campo de um objeto se 'este' existir e tiver o campo.
                    if let Some(Valor::Objeto { campos, .. }) = self.variaveis.get("este") {
                        if campos.borrow().contains_key(*nome_var) {
                            campos.borrow_mut().insert(nome_var.to_string(), valor);
                        } else {
                            self.variaveis.insert(nome_var.to_string(), valor);
                        }
                    } else {
                        self.variaveis.insert(nome_var.to_string(), valor);
                    }
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
                        Valor::Objeto { nome_classe, campos, .. } => {
                            let mut current_class_name = Some(nome_classe.clone());
                            let mut valor_encontrado = None;

                            while let Some(c_name) = current_class_name {
                                if let Some(_class_info) = self.classes.get(&c_name) {
                                    // Check if the property exists in the current class's fields
                                    if let Some(valor) = campos.borrow().get(*nome_propriedade) {
                                        valor_encontrado = Some(valor.clone());
                                        break;
                                    }
                                    current_class_name = self.classes.get(&c_name).and_then(|ci| ci.nome_classe_pai.clone());
                                } else {
                                    current_class_name = None;
                                }
                            }
                            // Ensure the correct type is pushed for boolean values
                            if let Some(Valor::Booleano(b)) = valor_encontrado {
                                self.pilha.push(Valor::Booleano(b));
                            } else {
                                self.pilha.push(valor_encontrado.unwrap_or(Valor::Nulo));
                            }
                        }
                        _ => return Err("GET_PROPERTY requer um objeto".to_string()),
                    }
                }

                "SET_PROPERTY" => {
                    let nome_propriedade = partes
                        .get(1)
                        .ok_or("SET_PROPERTY requer nome da propriedade")?;

                    // O valor a ser atribuído está no topo da pilha.
                    let valor = self
                        .pilha
                        .pop()
                        .ok_or("Pilha vazia para valor em SET_PROPERTY")?;

                    // O objeto está abaixo do valor.
                    let mut objeto = self
                        .pilha
                        .pop()
                        .ok_or("Pilha vazia para objeto em SET_PROPERTY")?;

                    match &mut objeto {
                        Valor::Objeto { nome_classe: _, campos, .. } => {
                            campos.borrow_mut().insert(nome_propriedade.to_string(), valor);
                        }
                        _ => {
                            return Err(
                                "SET_PROPERTY requer um objeto na pilha".to_string()
                            );
                        }
                    }

                // Devolve o objeto modificado para a pilha para permitir atribuições encadeadas.
                self.pilha.push(objeto);
                }

                "GET_STATIC_PROPERTY" => {
                    let nome_classe = partes.get(1).ok_or("GET_STATIC_PROPERTY requer nome da classe")?;
                    let nome_prop = partes.get(2).ok_or("GET_STATIC_PROPERTY requer nome da propriedade")?;
                    let classe = self.classes.get(*nome_classe).ok_or_else(|| format!("Classe '{}' não encontrada", nome_classe))?;
                    let valor = classe.campos_estaticos.borrow().get(*nome_prop).cloned().unwrap_or(Valor::Nulo);
                    self.pilha.push(valor);
                }

                "SET_STATIC_PROPERTY" => {
                    let nome_classe = partes.get(1).ok_or("SET_STATIC_PROPERTY requer nome da classe")?;
                    let nome_prop = partes.get(2).ok_or("SET_STATIC_PROPERTY requer nome da propriedade")?;
                    let valor = self.pilha.pop().ok_or("Pilha vazia em SET_STATIC_PROPERTY")?;
                    let classe = self.classes.get_mut(*nome_classe).ok_or_else(|| format!("Classe '{}' não encontrada", nome_classe))?;
                    classe.campos_estaticos.borrow_mut().insert(nome_prop.to_string(), valor);
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

                    let mut objeto = self.pilha.pop().ok_or("Pilha vazia para objeto em CALL_METHOD")?;
                    self.chamar_metodo(&mut objeto, nome_metodo, argumentos)?;
                    self.pilha.push(objeto);
                }

                "CALL_STATIC_METHOD" => {
                    let nome_classe = partes.get(1).ok_or("CALL_STATIC_METHOD requer nome da classe")?;
                    let nome_metodo = partes.get(2).ok_or("CALL_STATIC_METHOD requer nome do método")?;
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

                    let resultado = self.chamar_metodo_estatico(nome_classe, nome_metodo, argumentos)?;
                    self.pilha.push(resultado);
                }

                "POP" => {
                    // Remove o valor do topo da pilha.
                    // O uso de .ok_or previne um pânico se a pilha estiver vazia,
                    // transformando-o em um erro controlado.
                    self.pilha.pop().ok_or("Pilha vazia em POP")?;
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
                        .ok_or_else(|| format!("Função '{}' não definida", nome))?
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
                        functions: self.functions.clone(), // permite recursão
                        loaded_modules: self.loaded_modules.clone(), // ✅ NOVO
                        base_dir: self.base_dir.clone(),             // ✅ NOVO
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
        println!("=== Executando Código Global ===");

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
                let tamanho_str = if partes[0] == "DEFINE_CLASS" { "0" } else { partes.get(2).unwrap_or(&"0") };
                let tamanho: usize = tamanho_str.parse().unwrap_or(0);
                i += tamanho + 1;
            } else {
                codigo_global.push(instrucao.clone());
                i += 1;
            }
        }

        if codigo_global.is_empty() {
            println!("Nenhum código global para executar.");
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
                "LOAD_CONST_STR" | "LOAD_CONST_INT" | "LOAD_CONST_BOOL" | "LOAD_CONST_NULL" => {
                    // Executa apenas as instruções de carregamento de constantes
                    // (Reciclando a lógica do `run` principal)
                    match *op {
                        "LOAD_CONST_STR" => {
                            let valor = partes[1..].join(" ");
                            self.pilha.push(Valor::Texto(valor.trim_matches('"').to_string()));
                        }
                         "LOAD_CONST_INT" => {
                            let valor = partes.get(1).ok_or("LOAD_CONST_INT requer um argumento")?.parse::<i64>().map_err(|e| format!("Valor inválido para LOAD_CONST_INT: {}", e))?;
                            self.pilha.push(Valor::Inteiro(valor));
                        }
                        _ => {}
                    }
                }
                "SET_STATIC_PROPERTY" => {
                    let nome_classe = partes.get(1).ok_or("SET_STATIC_PROPERTY requer nome da classe")?;
                    let nome_prop = partes.get(2).ok_or("SET_STATIC_PROPERTY requer nome da propriedade")?;
                    let valor = self.pilha.pop().ok_or("Pilha vazia em SET_STATIC_PROPERTY")?;
                    let classe = self.classes.get_mut(*nome_classe).ok_or_else(|| format!("Classe '{}' não encontrada", nome_classe))?;
                    classe.campos_estaticos.borrow_mut().insert(nome_prop.to_string(), valor);
                }
                // Ignora todas as outras instruções
                _ => {}
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
    
    // Carregar definições (classes, funções)
    if let Err(e) = vm.carregar_definicoes() {
        eprintln!("Erro ao carregar definições: {}", e);
        return Err(e.into());
    }

    // Fase 2: Executar código de inicialização (atribuição de propriedades estáticas)
    if let Err(e) = vm.executar_codigo_global() {
        eprintln!("Erro ao executar código de inicialização: {}", e);
        return Err(e.into());
    }


    // Fase 4: Encontrar e executar a função 'Principal' (se existir)
    let funcao_principal = vm.functions.keys()
        .find(|nome| nome.ends_with("Principal") || nome == &&"Principal".to_string())
        .cloned();

    if let Some(nome_principal) = funcao_principal {
        println!("=== Executando função Principal ===");
        
        let principal_func = vm.functions.get(&nome_principal).unwrap().clone();
        let mut main_vm = VM {
            pilha: Vec::new(),
            variaveis: HashMap::new(),
            bytecode: principal_func.corpo.clone(),
            ip: 0,
            classes: vm.classes.clone(),
            functions: vm.functions.clone(),
            loaded_modules: vm.loaded_modules.clone(),
            base_dir: vm.base_dir.clone(),
        };

        if let Err(e) = main_vm.run() {
            eprintln!("❌ Erro na execução de Principal: {}", e);
            return Err("Execução de Principal falhou".into());
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