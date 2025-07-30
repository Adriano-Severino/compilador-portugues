use crate::ast;
use crate::ast::*;
use std::collections::HashMap;

fn get_expr_name(expr: &ast::Expressao) -> Option<String> {
    match expr {
        ast::Expressao::Identificador(s) => Some(s.clone()),
        ast::Expressao::Este => Some("este".to_string()),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum StatusOwnership {
    Dono,
    Emprestado,
    EmprestadoMutavel,
    Movido,
}

#[derive(Debug, Clone)]
pub struct InfoOwnership {
    pub status: StatusOwnership,
    pub escopo_criacao: usize,
    pub ultimo_uso: Option<usize>,
    pub pode_ser_movido: bool,
    pub eh_parametro_este: bool, // ✅ NOVO: Marcar se é contexto 'este'
}

#[derive(Debug, Clone)]
pub enum ValorAvaliado {
    Inteiro(i64),
    Texto(String),
    Booleano(bool),
    Objeto {
        classe: String,
        propriedades: HashMap<String, ValorAvaliado>,
    },
}

pub struct AnalisadorOwnership {
    variaveis: HashMap<String, InfoOwnership>,
    escopo_atual: usize,
    instrucao_atual: usize,
    erros: Vec<String>,
    warnings: Vec<String>,
    classes: HashMap<String, DeclaracaoClasse>, // ✅ NOVO: Armazenar classes para herança
    contexto_metodo_atual: Option<String>, // ✅ NOVO: Rastrear método atual
}

impl AnalisadorOwnership {
    pub fn new() -> Self {
        Self {
            variaveis: HashMap::new(),
            escopo_atual: 0,
            instrucao_atual: 0,
            erros: Vec::new(),
            warnings: Vec::new(),
            classes: HashMap::new(), // ✅ NOVO
            contexto_metodo_atual: None, // ✅ NOVO
        }
    }

    // ✅ NOVO: Registrar classes para análise de herança
    pub fn registrar_classe(&mut self, classe: DeclaracaoClasse) {
        self.classes.insert(classe.nome.clone(), classe);
    }

    pub fn analisar_programa(&mut self, programa: &Programa) -> Result<Vec<String>, Vec<String>> {
        // ✅ NOVO: Primeiro registrar todas as classes
        for declaracao in &programa.declaracoes {
            if let Declaracao::DeclaracaoClasse(classe) = declaracao {
                self.registrar_classe(classe.clone());
            }
        }

        // Analisar declarações
        for declaracao in &programa.declaracoes {
            self.analisar_declaracao(declaracao);
        }

        // Verificar variáveis não utilizadas
        self.verificar_variaveis_nao_utilizadas();

        if self.erros.is_empty() {
            Ok(self.warnings.clone())
        } else {
            Err(self.erros.clone())
        }
    }

    fn analisar_declaracao(&mut self, declaracao: &Declaracao) {
        match declaracao {
            Declaracao::Comando(cmd) => self.analisar_comando(cmd),
            Declaracao::DeclaracaoClasse(classe) => self.analisar_classe(classe),
            Declaracao::DeclaracaoFuncao(funcao) => self.analisar_funcao(funcao),
            _ => {}
        }
    }

    fn analisar_comando(&mut self, comando: &Comando) {
        self.instrucao_atual += 1;
        
        match comando {
            Comando::DeclaracaoVariavel(tipo, nome, valor) => {
                if let Some(expr) = valor {
                    self.analisar_expressao(expr);
                }

                let pode_ser_movido = match tipo {
                    Tipo::Inteiro | Tipo::Booleano => false, // Tipos primitivos são copiados
                    Tipo::Texto | Tipo::Classe(_) | Tipo::Lista(_) => true, // Tipos complexos podem ser movidos
                    _ => false,
                };

                self.variaveis.insert(
                    nome.clone(),
                    InfoOwnership {
                        status: StatusOwnership::Dono,
                        escopo_criacao: self.escopo_atual,
                        ultimo_uso: None,
                        pode_ser_movido,
                        eh_parametro_este: false, // ✅ NOVO
                    },
                );
            }

            Comando::DeclaracaoVar(nome, expr) => {
                self.analisar_expressao(expr);
                self.variaveis.insert(
                    nome.clone(),
                    InfoOwnership {
                        status: StatusOwnership::Dono,
                        escopo_criacao: self.escopo_atual,
                        ultimo_uso: None,
                        pode_ser_movido: true,
                        eh_parametro_este: false, // ✅ NOVO
                    },
                );
            }

            Comando::Atribuicao(nome, expr) => {
                self.analisar_movimento_em_expressao(expr);
                if let Some(info) = self.variaveis.get_mut(nome) {
                    info.ultimo_uso = Some(self.instrucao_atual);
                    info.status = StatusOwnership::Dono; // Reassinação restaura ownership
                }
            }

            Comando::AtribuirPropriedade(objeto_expr, _propriedade, expr) => {
                self.analisar_expressao(expr); // Analyze the value being assigned

                // Analyze the object/expression on the left-hand side of the assignment
                self.analisar_expressao(objeto_expr);

                // Check if the base of the assignment is an identifier (variable or 'este')
                if let Expressao::Identificador(nome_base) = &**objeto_expr {
                    if nome_base == "este" {
                        // 'este' is always available in methods
                        if self.contexto_metodo_atual.is_none() {
                            self.warnings.push(
                                "Uso de 'este' fora de contexto de método".to_string()
                            );
                        }
                    } else {
                        // It's a variable assignment
                        if let Some(info) = self.variaveis.get_mut(nome_base) {
                            info.ultimo_uso = Some(self.instrucao_atual);
                            info.status = StatusOwnership::Dono; // Reassociação restaura ownership
                        }
                    }
                }
            }

            Comando::Se(cond, cmd_if, cmd_else) => {
                self.analisar_expressao(cond);
                self.analisar_comando(cmd_if);
                if let Some(cmd) = cmd_else {
                    self.analisar_comando(cmd);
                }
            }

            Comando::Enquanto(cond, corpo) => {
                self.analisar_expressao(cond);
                self.analisar_comando(corpo);
            }

            Comando::Para(inicializacao, condicao, incremento, corpo) => {
                self.entrar_escopo();
                
                if let Some(init) = inicializacao {
                    self.analisar_comando(init);
                }

                if let Some(cond) = condicao {
                    self.analisar_expressao(cond);
                }

                self.analisar_comando(corpo);
                
                if let Some(inc) = incremento {
                    self.analisar_comando(inc);
                }

                self.sair_escopo();
            }

            Comando::Bloco(comandos) => {
                self.entrar_escopo();
                for cmd in comandos {
                    self.analisar_comando(cmd);
                }
                self.sair_escopo();
            }

            Comando::Retorne(expr) => {
                if let Some(expr) = expr {
                    self.analisar_movimento_em_expressao(expr);
                }
            }

            Comando::Expressao(expr) => {
                self.analisar_expressao(expr);
            }

            Comando::CriarObjeto(_var_nome, _classe, argumentos) => {
                for arg in argumentos {
                    self.analisar_expressao(arg);
                }
            }

            Comando::ChamarMetodo(objeto_expr, metodo, argumentos) => {
                if let Some(objeto_nome) = get_expr_name(objeto_expr) {
                    // ✅ NOVO: Análise especial para métodos redefiníveis
                    if let Some(info) = self.variaveis.get_mut(&objeto_nome) {
                        info.ultimo_uso = Some(self.instrucao_atual);
                        
                        // ✅ NOVO: Verificar se método existe na hierarquia
                        if let Some(classe_obj) = self.obter_classe_objeto(objeto_expr) {
                            if !self.metodo_existe_na_hierarquia(&classe_obj, metodo) {
                                self.warnings.push(format!(
                                    "Método '{}' pode não existir na hierarquia da classe '{}'",
                                    metodo, classe_obj
                                ));
                            }
                            
                            // ✅ NOVO: Verificar se é método polimórfico
                            if self.eh_metodo_polimorfismo(&classe_obj, metodo) {
                                self.warnings.push(format!(
                                    "Chamada polimórfica detectada: '{}.{}'",
                                    objeto_nome, metodo
                                ));
                            }
                        }
                    }
                }

                for arg in argumentos {
                    self.analisar_expressao(arg);
                }
            },

            Comando::AcessarCampo(objeto_nome, _campo) => {
                if let Some(info) = self.variaveis.get_mut(objeto_nome) {
                    info.ultimo_uso = Some(self.instrucao_atual);
                }
            }

            Comando::AtribuirCampo(objeto_expr, _campo, valor_expr) => {
                self.analisar_expressao(objeto_expr);
                self.analisar_expressao(valor_expr);
            }

            Comando::Imprima(expr) => {
                self.analisar_expressao(expr);
            }
        }
    }

    fn analisar_expressao(&mut self, expr: &Expressao) {
        match expr {
            Expressao::Identificador(nome) => {
                if nome == "este" {
                    // ✅ NOVO: Tratamento especial para 'este'
                    if self.contexto_metodo_atual.is_none() {
                        self.warnings.push(
                            "Uso de 'este' fora de contexto de método".to_string()
                        );
                    }
                } else {
                    if let Some(info) = self.variaveis.get_mut(nome) {
                        if info.status == StatusOwnership::Movido {
                            self.erros.push(format!(
                                "Uso de variável '{}' após movimento na linha {}",
                                nome, self.instrucao_atual
                            ));
                        } else {
                            info.ultimo_uso = Some(self.instrucao_atual);
                            // Leitura simples cria empréstimo
                            if info.status == StatusOwnership::Dono {
                                info.status = StatusOwnership::Emprestado;
                            }
                        }
                    }
                }
            }

            Expressao::AcessoMembro(obj, membro) => {
                self.analisar_expressao(obj);
                
                // ✅ NOVO: Verificar acesso a membro herdado
                if let Some(obj_nome) = get_expr_name(obj) {
                    if let Some(classe_obj) = self.obter_classe_objeto(obj) {
                        if !self.membro_existe_na_hierarquia(&classe_obj, membro) {
                            self.warnings.push(format!(
                                "Membro '{}' pode não existir na hierarquia da classe '{}'",
                                membro, classe_obj
                            ));
                        }
                    }
                }
            },

            Expressao::ChamadaMetodo(obj, metodo, args) => {
                self.analisar_expressao(obj);
                
                // ✅ NOVO: Análise de método polimórfico
                if let Some(obj_nome) = get_expr_name(obj) {
                    if let Some(classe_obj) = self.obter_classe_objeto(obj) {
                        if self.eh_metodo_redefinivel(&classe_obj, metodo) {
                            self.warnings.push(format!(
                                "Chamada a método redefinível '{}' - comportamento pode variar",
                                metodo
                            ));
                        }
                    }
                }
                
                for arg in args {
                    self.analisar_movimento_em_expressao(arg);
                }
            }

            Expressao::Chamada(_, args) => {
                for arg in args {
                    self.analisar_movimento_em_expressao(arg);
                }
            }

            Expressao::Aritmetica(_, esq, dir) => {
                self.analisar_expressao(esq);
                self.analisar_expressao(dir);
            }

            Expressao::Comparacao(_, esq, dir) => {
                self.analisar_expressao(esq);
                self.analisar_expressao(dir);
            }

            Expressao::Logica(_, esq, dir) => {
                self.analisar_expressao(esq);
                self.analisar_expressao(dir);
            }

            Expressao::Unario(_, expr) => {
                self.analisar_expressao(expr);
            }

            Expressao::NovoObjeto(_classe, argumentos) => {
                for arg in argumentos {
                    self.analisar_expressao(arg);
                }
            }

            Expressao::StringInterpolada(partes) => {
                for parte in partes {
                    if let PartStringInterpolada::Expressao(expr) = parte {
                        self.analisar_expressao(expr);
                    }
                }
            }

            Expressao::Este => {
                // ✅ NOVO: Verificar contexto de 'este'
                if self.contexto_metodo_atual.is_none() {
                    self.warnings.push(
                        "Uso de 'este' fora de contexto de método".to_string()
                    );
                }
            }

            _ => {}
        }
    }

    fn analisar_movimento_em_expressao(&mut self, expr: &Expressao) {
        match expr {
            Expressao::Identificador(nome) => {
                if nome == "este" {
                    // ✅ NOVO: 'este' nunca é movido
                    if self.contexto_metodo_atual.is_none() {
                        self.warnings.push(
                            "Uso de 'este' fora de contexto de método".to_string()
                        );
                    }
                } else {
                    if let Some(info) = self.variaveis.get_mut(nome) {
                        if info.status == StatusOwnership::Movido {
                            self.erros.push(format!(
                                "Uso de variável '{}' após movimento na linha {}",
                                nome, self.instrucao_atual
                            ));
                        } else if info.pode_ser_movido && !info.eh_parametro_este {
                            // Move a variável
                            info.status = StatusOwnership::Movido;
                            info.ultimo_uso = Some(self.instrucao_atual);
                        } else {
                            // Tipos primitivos são copiados
                            info.ultimo_uso = Some(self.instrucao_atual);
                        }
                    }
                }
            }
            _ => self.analisar_expressao(expr),
        }
    }

    fn analisar_classe(&mut self, classe: &DeclaracaoClasse) {
        for metodo in &classe.metodos {
            self.analisar_metodo(metodo);
        }

        for construtor in &classe.construtores {
            self.analisar_construtor(construtor);
        }
    }

    fn analisar_funcao(&mut self, funcao: &DeclaracaoFuncao) {
        self.entrar_escopo();
        
        // Parâmetros são donos de seus valores
        for param in &funcao.parametros {
            let pode_ser_movido = match param.tipo {
                Tipo::Inteiro | Tipo::Booleano => false,
                _ => true,
            };
            
            self.variaveis.insert(
                param.nome.clone(),
                InfoOwnership {
                    status: StatusOwnership::Dono,
                    escopo_criacao: self.escopo_atual,
                    ultimo_uso: None,
                    pode_ser_movido,
                    eh_parametro_este: false, // ✅ NOVO
                },
            );
        }

        for comando in &funcao.corpo {
            self.analisar_comando(comando);
        }

        self.sair_escopo();
    }

    fn analisar_metodo(&mut self, metodo: &MetodoClasse) {
        self.entrar_escopo();
        
        // ✅ NOVO: Definir contexto do método atual
        self.contexto_metodo_atual = Some(metodo.nome.clone());
        
        // ✅ NOVO: Verificar método redefinível/sobrescreve
        if metodo.eh_virtual && metodo.eh_override {
            self.erros.push(format!(
                "Método '{}' não pode ser redefinível e sobrescreve ao mesmo tempo",
                metodo.nome
            ));
        }
        
        if metodo.eh_override {
            self.warnings.push(format!(
                "Método '{}' sobrescreve método da classe pai - verificar compatibilidade",
                metodo.nome
            ));
        }
        
        if metodo.eh_virtual {
            self.warnings.push(format!(
                "Método '{}' é redefinível - pode ser sobrescrito por subclasses",
                metodo.nome
            ));
        }

        // Adicionar 'este' implícito
        self.variaveis.insert(
            "este".to_string(),
            InfoOwnership {
                status: StatusOwnership::Emprestado, // 'este' é sempre emprestado
                escopo_criacao: self.escopo_atual,
                ultimo_uso: None,
                pode_ser_movido: false,
                eh_parametro_este: true, // ✅ NOVO
            },
        );

        // Parâmetros
        for param in &metodo.parametros {
            let pode_ser_movido = match param.tipo {
                Tipo::Inteiro | Tipo::Booleano => false,
                _ => true,
            };
            
            self.variaveis.insert(
                param.nome.clone(),
                InfoOwnership {
                    status: StatusOwnership::Dono,
                    escopo_criacao: self.escopo_atual,
                    ultimo_uso: None,
                    pode_ser_movido,
                    eh_parametro_este: false, // ✅ NOVO
                },
            );
        }

        for comando in &metodo.corpo {
            self.analisar_comando(comando);
        }

        // ✅ NOVO: Limpar contexto do método
        self.contexto_metodo_atual = None;
        
        self.sair_escopo();
    }

    fn analisar_construtor(&mut self, construtor: &ConstrutorClasse) {
        self.entrar_escopo();
        
        // ✅ NOVO: Construtor tem contexto implícito de 'este'
        self.contexto_metodo_atual = Some("construtor".to_string());
        
        // Adicionar 'este' implícito no construtor
        self.variaveis.insert(
            "este".to_string(),
            InfoOwnership {
                status: StatusOwnership::Dono, // Em construtor, 'este' é dono
                escopo_criacao: self.escopo_atual,
                ultimo_uso: None,
                pode_ser_movido: false,
                eh_parametro_este: true, // ✅ NOVO
            },
        );

        // Parâmetros
        for param in &construtor.parametros {
            let pode_ser_movido = match param.tipo {
                Tipo::Inteiro | Tipo::Booleano => false,
                _ => true,
            };
            
            self.variaveis.insert(
                param.nome.clone(),
                InfoOwnership {
                    status: StatusOwnership::Dono,
                    escopo_criacao: self.escopo_atual,
                    ultimo_uso: None,
                    pode_ser_movido,
                    eh_parametro_este: false, // ✅ NOVO
                },
            );
        }

        for comando in &construtor.corpo {
            self.analisar_comando(comando);
        }

        // ✅ NOVO: Limpar contexto
        self.contexto_metodo_atual = None;
        
        self.sair_escopo();
    }

    // ✅ NOVO: Obter classe de um objeto
    fn obter_classe_objeto(&self, objeto_expr: &Expressao) -> Option<String> {
        if let Some(objeto_nome) = get_expr_name(objeto_expr) {
            if let Some(info) = self.variaveis.get(&objeto_nome) {
                // Em uma implementação completa, você inferiria o tipo da variável
                // e retornaria o nome da classe desse tipo.
                // Por enquanto, um fallback genérico.
                Some("ObjetoGenerico".to_string()) 
            } else {
                None
            }
        } else {
            None
        }
    }

    // ✅ NOVO: Verificar se método existe na hierarquia
    fn metodo_existe_na_hierarquia(&self, classe: &str, metodo: &str) -> bool {
        let mut classe_atual = Some(classe.to_string());
        
        while let Some(nome_classe) = classe_atual {
            if let Some(def_classe) = self.classes.get(&nome_classe) {
                // Verificar se método existe nesta classe
                for metodo_classe in &def_classe.metodos {
                    if metodo_classe.nome == metodo {
                        return true;
                    }
                }
                
                // Ir para classe pai
                classe_atual = def_classe.classe_pai.clone();
            } else {
                break;
            }
        }
        
        false
    }

    // ✅ NOVO: Verificar se membro existe na hierarquia
    fn membro_existe_na_hierarquia(&self, classe: &str, membro: &str) -> bool {
        let mut classe_atual = Some(classe.to_string());
        
        while let Some(nome_classe) = classe_atual {
            if let Some(def_classe) = self.classes.get(&nome_classe) {
                // Verificar propriedades
                for prop in &def_classe.propriedades {
                    if prop.nome == membro {
                        return true;
                    }
                }
                
                // Verificar campos
                for campo in &def_classe.campos {
                    if campo.nome == membro {
                        return true;
                    }
                }
                
                // Ir para classe pai
                classe_atual = def_classe.classe_pai.clone();
            } else {
                break;
            }
        }
        
        false
    }

    // ✅ NOVO: Verificar se método é redefinível
    fn eh_metodo_redefinivel(&self, classe: &str, metodo: &str) -> bool {
        if let Some(def_classe) = self.classes.get(classe) {
            for metodo_classe in &def_classe.metodos {
                if metodo_classe.nome == metodo {
                    return metodo_classe.eh_virtual;
                }
            }
        }
        false
    }

    // ✅ NOVO: Verificar se há polimorfismo
    fn eh_metodo_polimorfismo(&self, classe: &str, metodo: &str) -> bool {
        // Verificar se método é redefinível e a classe tem subclasses
        self.eh_metodo_redefinivel(classe, metodo)
    }

    fn verificar_variaveis_nao_utilizadas(&mut self) {
        for (nome, info) in &self.variaveis {
            if info.ultimo_uso.is_none() && nome != "este" {
                self.warnings.push(format!(
                    "Variável '{}' declarada mas nunca utilizada", 
                    nome
                ));
            }
        }
    }

    fn entrar_escopo(&mut self) {
        self.escopo_atual += 1;
    }

    fn sair_escopo(&mut self) {
        // Remove variáveis do escopo atual
        self.variaveis.retain(|_, info| info.escopo_criacao < self.escopo_atual);
        self.escopo_atual -= 1;
    }
}