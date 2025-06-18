// src/runtime/execution_context.rs
use std::collections::HashMap;
use crate::ast::*;

#[derive(Clone, Debug)]
pub enum ValorRuntime {
    Inteiro(i64),
    Texto(String),
    Booleano(bool),
    Objeto {
        classe: String,
        propriedades: HashMap<String, ValorRuntime>,
        metodos: HashMap<String, MetodoInfo>,
    },
    Nulo,
}

#[derive(Clone, Debug)]
pub struct MetodoInfo {
    pub parametros: Vec<Parametro>,
    pub corpo: Vec<Comando>,
    pub eh_virtual: bool,
    pub eh_override: bool,
}

#[derive(Clone, Debug)]
pub struct ContextoExecucao {
    // Stack de escopos para variáveis locais
    pub escopos: Vec<HashMap<String, ValorRuntime>>,
    // Registro de classes compiladas (compile-time)
    pub classes_compiladas: HashMap<String, ClasseCompilada>,
    // Cache de objetos instanciados (runtime)
    pub cache_objetos: HashMap<String, ValorRuntime>,
    // Pilha de chamadas para debug
    pub pilha_chamadas: Vec<ChamadaInfo>,
    // Sistema de captura de argumentos dinâmicos
    pub argumentos_capturados: Vec<Vec<ValorRuntime>>,
    // Contexto de método atual
    pub contexto_metodo_atual: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ClasseCompilada {
    pub nome: String,
    pub classe_pai: Option<String>,
    pub propriedades: HashMap<String, TipoPropriedade>,
    pub metodos: HashMap<String, MetodoInfo>,
    pub construtores: Vec<ConstrutorInfo>,
}

#[derive(Clone, Debug)]
pub struct TipoPropriedade {
    pub tipo: Tipo,
    pub valor_inicial: Option<ValorRuntime>,
    pub eh_readonly: bool,
}

#[derive(Clone, Debug)]
pub struct ConstrutorInfo {
    pub parametros: Vec<Parametro>,
    pub corpo: Vec<Comando>,
}

#[derive(Clone, Debug)]
pub struct ChamadaInfo {
    pub funcao: String,
    pub linha: usize,
    pub parametros: Vec<ValorRuntime>,
}

impl ContextoExecucao {
    pub fn new() -> Self {
        Self {
            escopos: vec![HashMap::new()], // Escopo global
            classes_compiladas: HashMap::new(),
            cache_objetos: HashMap::new(),
            pilha_chamadas: Vec::new(),
            argumentos_capturados: Vec::new(),
            contexto_metodo_atual: None,
        }
    }

    // COMPILE-TIME: Registrar classe após verificação de tipos
    pub fn registrar_classe_compilada(&mut self, classe: &DeclaracaoClasse) -> Result<(), String> {
        let mut propriedades = HashMap::new();
        let mut metodos = HashMap::new();
        let mut construtores = Vec::new();

        // Processar propriedades
        for prop in &classe.propriedades {
            propriedades.insert(
                prop.nome.clone(),
                TipoPropriedade {
                    tipo: prop.tipo.clone(),
                    valor_inicial: prop.valor_inicial.as_ref()
                        .map(|expr| self.avaliar_expressao_estatica(expr))
                        .transpose()?,
                    eh_readonly: false,
                }
            );
        }

        // Processar métodos
        for metodo in &classe.metodos {
            metodos.insert(
                metodo.nome.clone(),
                MetodoInfo {
                    parametros: metodo.parametros.clone(),
                    corpo: metodo.corpo.clone(),
                    eh_virtual: metodo.eh_virtual,
                    eh_override: metodo.eh_override,
                }
            );
        }

        // Processar construtores
        for construtor in &classe.construtores {
            construtores.push(ConstrutorInfo {
                parametros: construtor.parametros.clone(),
                corpo: construtor.corpo.clone(),
            });
        }

        let classe_compilada = ClasseCompilada {
            nome: classe.nome.clone(),
            classe_pai: classe.classe_pai.clone(),
            propriedades,
            metodos,
            construtores,
        };

        self.classes_compiladas.insert(classe.nome.clone(), classe_compilada);
        Ok(())
    }

    // RUNTIME: Criar instância com argumentos reais dinâmicos
    pub fn criar_instancia(&mut self, nome_classe: &str, argumentos: Vec<ValorRuntime>) -> Result<ValorRuntime, String> {
        let classe = self.classes_compiladas.get(nome_classe)
            .ok_or_else(|| format!("Classe '{}' não encontrada", nome_classe))?
            .clone();

        // Capturar argumentos para uso dinâmico
        self.argumentos_capturados.push(argumentos.clone());

        // Encontrar construtor compatível
        let construtor = classe.construtores.iter()
            .find(|c| c.parametros.len() == argumentos.len())
            .ok_or_else(|| format!("Nenhum construtor compatível encontrado para {} argumentos", argumentos.len()))?;

        // Criar objeto com propriedades inicializadas
        let mut propriedades = HashMap::new();
        
        // Inicializar propriedades com valores padrão
        for (nome_prop, tipo_prop) in &classe.propriedades {
            let valor = if let Some(valor_inicial) = &tipo_prop.valor_inicial {
                valor_inicial.clone()
            } else {
                self.valor_padrao_para_tipo(&tipo_prop.tipo)
            };
            propriedades.insert(nome_prop.clone(), valor);
        }

        // **DINÂMICO**: Mapear argumentos reais do construtor para propriedades
        for (i, param) in construtor.parametros.iter().enumerate() {
            if let Some(arg) = argumentos.get(i) {
                if propriedades.contains_key(&param.nome) {
                    propriedades.insert(param.nome.clone(), arg.clone());
                }
            }
        }

        // Coletar métodos da classe e hierarquia
        let mut metodos_completos = HashMap::new();
        self.coletar_metodos_hierarquia(&nome_classe, &mut metodos_completos)?;

        let objeto = ValorRuntime::Objeto {
            classe: nome_classe.to_string(),
            propriedades,
            metodos: metodos_completos,
        };

        // Executar corpo do construtor com valores reais
        self.entrar_escopo();
        self.definir_variavel("este", objeto.clone());
        
        // Definir parâmetros com valores reais dos argumentos
        for (i, param) in construtor.parametros.iter().enumerate() {
            if let Some(arg) = argumentos.get(i) {
                self.definir_variavel(&param.nome, arg.clone());
            }
        }

        // Executar comandos do construtor
        for comando in &construtor.corpo {
            self.executar_comando(comando)?;
        }

        let objeto_final = self.obter_variavel("este")
            .ok_or_else(|| "Objeto 'este' perdido durante construção".to_string())?;
        
        self.sair_escopo();
        Ok(objeto_final)
    }

    // RUNTIME: Avaliação dinâmica de expressões
    pub fn avaliar_expressao(&mut self, expr: &Expressao) -> Result<ValorRuntime, String> {
        match expr {
            Expressao::Inteiro(n) => Ok(ValorRuntime::Inteiro(*n)),
            Expressao::Texto(s) => Ok(ValorRuntime::Texto(s.clone())),
            Expressao::Booleano(b) => Ok(ValorRuntime::Booleano(*b)),

            Expressao::Identificador(nome) => {
                self.obter_variavel(nome)
                    .ok_or_else(|| format!("Variável '{}' não encontrada", nome))
            }

            Expressao::Este => {
                self.obter_variavel("este")
                    .ok_or_else(|| "Referência 'este' não disponível neste contexto".to_string())
            }

            Expressao::AcessoMembro(obj_expr, membro) => {
                let objeto = self.avaliar_expressao(obj_expr)?;
                match objeto {
                    ValorRuntime::Objeto { propriedades, .. } => {
                        propriedades.get(membro)
                            .cloned()
                            .ok_or_else(|| format!("Propriedade '{}' não encontrada", membro))
                    }
                    _ => Err(format!("Tentativa de acessar membro '{}' em não-objeto", membro))
                }
            }

            Expressao::NovoObjeto(classe, args) => {
                let mut argumentos_avaliados = Vec::new();
                for arg in args {
                    argumentos_avaliados.push(self.avaliar_expressao(arg)?);
                }
                self.criar_instancia(classe, argumentos_avaliados)
            }

            Expressao::Aritmetica(op, esq, dir) => {
                let val_esq = self.avaliar_expressao(esq)?;
                let val_dir = self.avaliar_expressao(dir)?;
                self.executar_operacao_aritmetica(op, val_esq, val_dir)
            }

            Expressao::ChamadaMetodo(obj_expr, metodo, args) => {
                let objeto = self.avaliar_expressao(obj_expr)?;
                let mut argumentos = Vec::new();
                for arg in args {
                    argumentos.push(self.avaliar_expressao(arg)?);
                }
                self.chamar_metodo(objeto, metodo, argumentos)
            }

            Expressao::StringInterpolada(partes) => {
                let mut resultado = String::new();
                for parte in partes {
                    match parte {
                        PartStringInterpolada::Texto(texto) => {
                            resultado.push_str(texto);
                        }
                        PartStringInterpolada::Expressao(expr) => {
                            let valor = self.avaliar_expressao(expr)?;
                            resultado.push_str(&self.valor_para_string(&valor));
                        }
                    }
                }
                Ok(ValorRuntime::Texto(resultado))
            }

            _ => Err("Expressão não implementada".to_string())
        }
    }

    // COMPILE-TIME: Avaliação estática para valores iniciais
    fn avaliar_expressao_estatica(&self, expr: &Expressao) -> Result<ValorRuntime, String> {
        match expr {
            Expressao::Inteiro(n) => Ok(ValorRuntime::Inteiro(*n)),
            Expressao::Texto(s) => Ok(ValorRuntime::Texto(s.clone())),
            Expressao::Booleano(b) => Ok(ValorRuntime::Booleano(*b)),
            _ => Err("Apenas valores literais são permitidos como valores iniciais".to_string())
        }
    }

    // **CORRIGIDO**: Métodos auxiliares agora públicos
    fn executar_operacao_aritmetica(&self, op: &OperadorAritmetico, esq: ValorRuntime, dir: ValorRuntime) -> Result<ValorRuntime, String> {
        match op {
            OperadorAritmetico::Soma => {
                match (&esq, &dir) {
                    (ValorRuntime::Inteiro(a), ValorRuntime::Inteiro(b)) => Ok(ValorRuntime::Inteiro(a + b)),
                    _ => {
                        let str_esq = self.valor_para_string(&esq);
                        let str_dir = self.valor_para_string(&dir);
                        Ok(ValorRuntime::Texto(format!("{}{}", str_esq, str_dir)))
                    }
                }
            }
            OperadorAritmetico::Subtracao => {
                match (&esq, &dir) {
                    (ValorRuntime::Inteiro(a), ValorRuntime::Inteiro(b)) => Ok(ValorRuntime::Inteiro(a - b)),
                    _ => Err("Subtração só é válida entre inteiros".to_string())
                }
            }
            OperadorAritmetico::Multiplicacao => {
                match (&esq, &dir) {
                    (ValorRuntime::Inteiro(a), ValorRuntime::Inteiro(b)) => Ok(ValorRuntime::Inteiro(a * b)),
                    _ => Err("Multiplicação só é válida entre inteiros".to_string())
                }
            }
            OperadorAritmetico::Divisao => {
                match (&esq, &dir) {
                    (ValorRuntime::Inteiro(a), ValorRuntime::Inteiro(b)) if *b != 0 => Ok(ValorRuntime::Inteiro(a / b)),
                    _ => Err("Divisão inválida ou por zero".to_string())
                }
            }
            OperadorAritmetico::Modulo => {
                match (&esq, &dir) {
                    (ValorRuntime::Inteiro(a), ValorRuntime::Inteiro(b)) if *b != 0 => Ok(ValorRuntime::Inteiro(a % b)),
                    _ => Err("Módulo inválido ou por zero".to_string())
                }
            }
        }
    }

    fn chamar_metodo(&mut self, mut objeto: ValorRuntime, nome_metodo: &str, argumentos: Vec<ValorRuntime>) -> Result<ValorRuntime, String> {
        if let ValorRuntime::Objeto { ref metodos, .. } = objeto {
            if let Some(metodo_info) = metodos.get(nome_metodo).cloned() {
                // Entrar em novo escopo para o método
                self.entrar_escopo();
                self.contexto_metodo_atual = Some(nome_metodo.to_string());
                self.definir_variavel("este", objeto.clone());
                
                // Definir parâmetros
                for (i, param) in metodo_info.parametros.iter().enumerate() {
                    if let Some(arg) = argumentos.get(i) {
                        self.definir_variavel(&param.nome, arg.clone());
                    }
                }

                // Executar corpo do método
                let mut resultado = ValorRuntime::Nulo;
                for comando in &metodo_info.corpo {
                    if let Ok(val) = self.executar_comando(comando) {
                        if let Some(ret) = val {
                            resultado = ret;
                            break;
                        }
                    }
                }

                // Atualizar objeto se foi modificado
                if let Some(objeto_modificado) = self.obter_variavel("este") {
                   objeto = objeto_modificado.clone();
                   return Ok(objeto);
                }

                self.contexto_metodo_atual = None;
                self.sair_escopo();
                Ok(resultado)
            } else {
                Err(format!("Método '{}' não encontrado", nome_metodo))
            }
        } else {
            Err("Tentativa de chamar método em não-objeto".to_string())
        }
    }

    fn coletar_metodos_hierarquia(&self, nome_classe: &str, metodos: &mut HashMap<String, MetodoInfo>) -> Result<(), String> {
        if let Some(classe) = self.classes_compiladas.get(nome_classe) {
            // Adicionar métodos da classe atual
            for (nome, metodo) in &classe.metodos {
                metodos.insert(nome.clone(), metodo.clone());
            }

            // Recursivamente coletar métodos da classe pai
            if let Some(classe_pai) = &classe.classe_pai {
                self.coletar_metodos_hierarquia(classe_pai, metodos)?;
            }
        }
        Ok(())
    }

    // **CORRIGIDO**: Método público para execução de comandos
    pub fn executar_comando(&mut self, comando: &Comando) -> Result<Option<ValorRuntime>, String> {
        match comando {
            Comando::Imprima(expr) => {
                let valor = self.avaliar_expressao(expr)?;
                println!("{}", self.valor_para_string(&valor));
                Ok(None)
            }

            Comando::AtribuirPropriedade(obj_nome, prop, expr) => {
                let valor = self.avaliar_expressao(expr)?;
                
                if let Some(objeto) = self.obter_variavel_mut(obj_nome) {
                    if let ValorRuntime::Objeto { ref mut propriedades, .. } = objeto {
                        propriedades.insert(prop.clone(), valor);
                    }
                }
                Ok(None)
            }

            Comando::DeclaracaoVar(nome, expr) => {
                let valor = self.avaliar_expressao(expr)?;
                self.definir_variavel(nome, valor);
                Ok(None)
            }

            Comando::Atribuicao(nome, expr) => {
                let valor = self.avaliar_expressao(expr)?;
                self.definir_variavel(nome, valor);
                Ok(None)
            }

            Comando::Retorne(expr_opt) => {
                if let Some(expr) = expr_opt {
                    Ok(Some(self.avaliar_expressao(expr)?))
                } else {
                    Ok(Some(ValorRuntime::Nulo))
                }
            }

            Comando::Bloco(comandos) => {
                self.entrar_escopo();
                let mut resultado = None;
                for cmd in comandos {
                    if let Some(ret) = self.executar_comando(cmd)? {
                        resultado = Some(ret);
                        break;
                    }
                }
                self.sair_escopo();
                Ok(resultado)
            }

            _ => Ok(None)
        }
    }

    fn valor_padrao_para_tipo(&self, tipo: &Tipo) -> ValorRuntime {
        match tipo {
            Tipo::Inteiro => ValorRuntime::Inteiro(0),
            Tipo::Texto => ValorRuntime::Texto("".to_string()),
            Tipo::Booleano => ValorRuntime::Booleano(false),
            _ => ValorRuntime::Nulo,
        }
    }

    fn valor_para_string(&self, valor: &ValorRuntime) -> String {
        match valor {
            ValorRuntime::Inteiro(n) => n.to_string(),
            ValorRuntime::Texto(s) => s.clone(),
            ValorRuntime::Booleano(b) => if *b { "verdadeiro" } else { "falso" }.to_string(),
            ValorRuntime::Objeto { classe, propriedades, .. } => {
                if let Some(nome) = propriedades.get("Nome") {
                    self.valor_para_string(nome)
                } else {
                    format!("Objeto<{}>", classe)
                }
            }
            ValorRuntime::Nulo => "nulo".to_string(),
        }
    }

    // **CORRIGIDO**: Métodos de gerenciamento de escopo agora públicos
    pub fn entrar_escopo(&mut self) {
        self.escopos.push(HashMap::new());
    }

    pub fn sair_escopo(&mut self) {
        self.escopos.pop();
    }

    pub fn definir_variavel(&mut self, nome: &str, valor: ValorRuntime) {
        if let Some(escopo_atual) = self.escopos.last_mut() {
            escopo_atual.insert(nome.to_string(), valor);
        }
    }

    pub fn obter_variavel(&self, nome: &str) -> Option<ValorRuntime> {
        // Buscar do escopo mais interno para o mais externo
        for escopo in self.escopos.iter().rev() {
            if let Some(valor) = escopo.get(nome) {
                return Some(valor.clone());
            }
        }
        None
    }

    pub fn obter_variavel_mut(&mut self, nome: &str) -> Option<&mut ValorRuntime> {
        // Buscar do escopo mais interno para o mais externo
        for escopo in self.escopos.iter_mut().rev() {
            if escopo.contains_key(nome) {
                return escopo.get_mut(nome);
            }
        }
        None
    }

    // **NOVO**: Método para capturar argumentos dinâmicos
    pub fn capturar_argumentos_dinamicos(&mut self, expressoes: &[Expressao]) -> Result<Vec<ValorRuntime>, String> {
        let mut argumentos = Vec::new();
        for expr in expressoes {
            argumentos.push(self.avaliar_expressao(expr)?);
        }
        Ok(argumentos)
    }

    // **NOVO**: Método para resetar contexto
    pub fn resetar_contexto(&mut self) {
        self.argumentos_capturados.clear();
        self.contexto_metodo_atual = None;
        self.pilha_chamadas.clear();
    }
}