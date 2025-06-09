use crate::ast::*;
use std::collections::HashMap;

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
}

impl AnalisadorOwnership {
    pub fn new() -> Self {
        Self {
            variaveis: HashMap::new(),
            escopo_atual: 0,
            instrucao_atual: 0,
            erros: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub fn analisar_programa(&mut self, programa: &Programa) -> Result<Vec<String>, Vec<String>> {
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

            Comando::AtribuirPropriedade(objeto, _propriedade, expr) => {
                self.analisar_expressao(expr);
                if let Some(info) = self.variaveis.get_mut(objeto) {
                    info.ultimo_uso = Some(self.instrucao_atual);
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

            // ✅ CORREÇÃO: Para loop corrigido
            Comando::Para(inicializacao, condicao, incremento, corpo) => {
                self.entrar_escopo();

                // ✅ Verificar se inicialização existe antes de analisar
                if let Some(init) = inicializacao {
                    self.analisar_comando(init); // ✅ Agora passa &Comando corretamente
                }

                // ✅ Verificar se condição existe antes de analisar
                if let Some(cond) = condicao {
                    self.analisar_expressao(cond); // ✅ Agora passa &Expressao corretamente
                }

                self.analisar_comando(corpo);

                // ✅ Verificar se incremento existe antes de analisar
                if let Some(inc) = incremento {
                    self.analisar_comando(inc); // ✅ Agora passa &Comando corretamente
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

            Comando::ChamarMetodo(objeto_nome, _metodo, argumentos) => {
                if let Some(info) = self.variaveis.get_mut(objeto_nome) {
                    info.ultimo_uso = Some(self.instrucao_atual);
                }
                for arg in argumentos {
                    self.analisar_expressao(arg);
                }
            }

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

            Expressao::AcessoMembro(obj, _) => {
                self.analisar_expressao(obj);
            }

            Expressao::ChamadaMetodo(obj, _, args) => {
                self.analisar_expressao(obj);
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

            // ✅ CORREÇÃO: Adicionar case faltante para operadores unários
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

            _ => {}
        }
    }

    fn analisar_movimento_em_expressao(&mut self, expr: &Expressao) {
        match expr {
            Expressao::Identificador(nome) => {
                if let Some(info) = self.variaveis.get_mut(nome) {
                    if info.status == StatusOwnership::Movido {
                        self.erros.push(format!(
                            "Uso de variável '{}' após movimento na linha {}",
                            nome, self.instrucao_atual
                        ));
                    } else if info.pode_ser_movido {
                        // Move a variável
                        info.status = StatusOwnership::Movido;
                        info.ultimo_uso = Some(self.instrucao_atual);
                    } else {
                        // Tipos primitivos são copiados
                        info.ultimo_uso = Some(self.instrucao_atual);
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

        // Adicionar 'este' implícito
        self.variaveis.insert(
            "este".to_string(),
            InfoOwnership {
                status: StatusOwnership::Emprestado, // 'este' é sempre emprestado
                escopo_criacao: self.escopo_atual,
                ultimo_uso: None,
                pode_ser_movido: false,
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
                },
            );
        }

        for comando in &metodo.corpo {
            self.analisar_comando(comando);
        }

        self.sair_escopo();
    }

    fn analisar_construtor(&mut self, construtor: &ConstrutorClasse) {
        self.entrar_escopo();

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
                },
            );
        }

        for comando in &construtor.corpo {
            self.analisar_comando(comando);
        }

        self.sair_escopo();
    }

    fn verificar_variaveis_nao_utilizadas(&mut self) {
        for (nome, info) in &self.variaveis {
            if info.ultimo_uso.is_none() && nome != "este" {
                self.warnings
                    .push(format!("Variável '{}' declarada mas nunca utilizada", nome));
            }
        }
    }

    fn entrar_escopo(&mut self) {
        self.escopo_atual += 1;
    }

    fn sair_escopo(&mut self) {
        // Remove variáveis do escopo atual
        self.variaveis
            .retain(|_, info| info.escopo_criacao < self.escopo_atual);
        self.escopo_atual -= 1;
    }
}