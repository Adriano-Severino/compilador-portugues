use crate::ast::*;
use std::collections::HashMap;

pub struct InferenciaTipos {
    tipos_inferidos: HashMap<String, Tipo>,
    classes: HashMap<String, DeclaracaoClasse>, // ✅ NOVO: Armazenar classes para herança
}

impl InferenciaTipos {
    pub fn new() -> Self {
        Self {
            tipos_inferidos: HashMap::new(),
            classes: HashMap::new(), // ✅ NOVO
        }
    }

    // ✅ NOVO: Registrar classes para herança
    pub fn registrar_classe(&mut self, classe: DeclaracaoClasse) {
        self.classes.insert(classe.nome.clone(), classe);
    }

    pub fn inferir_tipo(&mut self, expr: &Expressao) -> Result<Tipo, String> {
        match expr {
            Expressao::Inteiro(_) => Ok(Tipo::Inteiro),
            Expressao::Texto(_) => Ok(Tipo::Texto),
            Expressao::Booleano(_) => Ok(Tipo::Booleano),
            Expressao::FlutuanteLiteral(_) => Ok(Tipo::Flutuante),
            Expressao::DuploLiteral(_) => Ok(Tipo::Duplo),
            Expressao::Decimal(_) => Ok(Tipo::Decimal),
            Expressao::NovoObjeto(c, _) => Ok(Tipo::Classe(c.clone())),

            Expressao::Aritmetica(op, esq, dir) => {
                let t_esq = self.inferir_tipo(esq)?;
                let t_dir = self.inferir_tipo(dir)?;
                match (op, &t_esq, &t_dir) {
                    (OperadorAritmetico::Soma, Tipo::Texto, _)
                    | (OperadorAritmetico::Soma, _, Tipo::Texto) => Ok(Tipo::Texto),
                    _ => {
                        // Promoção numérica: inteiro < flutuante < duplo
                        let is_num =
                            |t: &Tipo| matches!(t, Tipo::Inteiro | Tipo::Flutuante | Tipo::Duplo);
                        if is_num(&t_esq) && is_num(&t_dir) {
                            if matches!(t_esq, Tipo::Duplo) || matches!(t_dir, Tipo::Duplo) {
                                Ok(Tipo::Duplo)
                            } else if matches!(t_esq, Tipo::Flutuante)
                                || matches!(t_dir, Tipo::Flutuante)
                            {
                                Ok(Tipo::Flutuante)
                            } else {
                                Ok(Tipo::Inteiro)
                            }
                        } else {
                            Err("Tipos incompatíveis para operação aritmética".into())
                        }
                    }
                }
            }

            Expressao::Comparacao(_, _, _) => Ok(Tipo::Booleano),

            Expressao::Logica(_, _, _) => Ok(Tipo::Booleano),

            Expressao::Unario(op, expr) => {
                let tipo_expr = self.inferir_tipo(expr)?;
                match (op, &tipo_expr) {
                    (OperadorUnario::NegacaoLogica, Tipo::Booleano) => Ok(Tipo::Booleano),
                    (OperadorUnario::NegacaoNumerica, Tipo::Inteiro) => Ok(Tipo::Inteiro),
                    _ => Err("Operador unário incompatível com tipo".into()),
                }
            }

            Expressao::Identificador(n) => {
                if n == "este" {
                    // Tipo 'este' deve ser inferido do contexto
                    Ok(Tipo::Texto) // Fallback genérico
                } else {
                    self.tipos_inferidos
                        .get(n)
                        .cloned()
                        .ok_or_else(|| format!("Não foi possível inferir o tipo de '{}'", n))
                }
            }

            // ✅ NOVO: Inferir tipo para acesso a membros com herança
            Expressao::AcessoMembro(obj_expr, membro) => {
                let tipo_obj = self.inferir_tipo(obj_expr)?;
                match tipo_obj {
                    Tipo::Classe(classe_nome) => {
                        // Buscar membro na hierarquia de herança
                        self.inferir_tipo_membro_hierarquia(&classe_nome, membro)
                    }
                    _ => Err(format!(
                        "Tentativa de acessar membro '{}' em tipo não-classe",
                        membro
                    )),
                }
            }

            // ✅ NOVO: Inferir tipo para chamadas de método com herança
            Expressao::ChamadaMetodo(obj_expr, metodo, _argumentos) => {
                let tipo_obj = self.inferir_tipo(obj_expr)?;
                match tipo_obj {
                    Tipo::Classe(classe_nome) => {
                        // Buscar método na hierarquia de herança
                        self.inferir_tipo_metodo_hierarquia(&classe_nome, metodo)
                    }
                    _ => {
                        // Métodos especiais genéricos
                        match metodo.as_str() {
                            "apresentar" => Ok(Tipo::Vazio),
                            "paraTexto" => Ok(Tipo::Texto),
                            _ => Ok(Tipo::Vazio), // Fallback para métodos desconhecidos
                        }
                    }
                }
            }

            Expressao::Chamada(nome, argumentos) => {
                // Inferir tipo de chamadas de função
                match nome.as_str() {
                    "tamanho" => Ok(Tipo::Inteiro),
                    "converter" => {
                        if !argumentos.is_empty() {
                            self.inferir_tipo(&argumentos[0])
                        } else {
                            Ok(Tipo::Texto)
                        }
                    }
                    _ => Ok(Tipo::Vazio), // Funções desconhecidas retornam vazio
                }
            }

            Expressao::StringInterpolada(_) => Ok(Tipo::Texto),

            Expressao::Este => {
                // Tipo 'este' deve ser inferido do contexto atual
                Ok(Tipo::Texto) // Fallback genérico
            }
        }
    }

    // ✅ NOVO: Buscar tipo de membro na hierarquia de herança
    fn inferir_tipo_membro_hierarquia(&self, classe: &str, membro: &str) -> Result<Tipo, String> {
        let mut classe_atual = Some(classe.to_string());

        while let Some(nome_classe) = classe_atual {
            if let Some(def_classe) = self.classes.get(&nome_classe) {
                // Buscar propriedade na classe atual
                for propriedade in &def_classe.propriedades {
                    if propriedade.nome == membro {
                        return Ok(propriedade.tipo.clone());
                    }
                }

                // Buscar campo na classe atual
                for campo in &def_classe.campos {
                    if campo.nome == membro {
                        return Ok(campo.tipo.clone());
                    }
                }

                // Ir para classe pai
                classe_atual = def_classe.classe_pai.clone();
            } else {
                break;
            }
        }

        // Fallback para membros não encontrados
        Ok(Tipo::Texto)
    }

    // ✅ NOVO: Buscar tipo de retorno de método na hierarquia
    fn inferir_tipo_metodo_hierarquia(&self, classe: &str, metodo: &str) -> Result<Tipo, String> {
        let mut classe_atual = Some(classe.to_string());

        while let Some(nome_classe) = classe_atual {
            if let Some(def_classe) = self.classes.get(&nome_classe) {
                // Buscar método na classe atual
                for metodo_classe in &def_classe.metodos {
                    if metodo_classe.nome == metodo {
                        return match metodo_classe.tipo_retorno.as_ref() {
                            Some(tipo) => Ok(tipo.clone()),
                            None => Ok(Tipo::Vazio), // Métodos sem tipo de retorno explícito são Vazio
                        };
                    }
                }

                // Ir para classe pai
                classe_atual = def_classe.classe_pai.clone();
            } else {
                break;
            }
        }

        // Métodos especiais
        match metodo {
            "apresentar" => Ok(Tipo::Vazio),
            "paraTexto" => Ok(Tipo::Texto),
            "clone" => Ok(Tipo::Classe(classe.to_string())),
            _ => Ok(Tipo::Vazio), // Fallback
        }
    }

    // ✅ EXISTENTE: Manter funcionalidade original
    pub fn registrar_variavel(&mut self, nome: String, tipo: Tipo) {
        self.tipos_inferidos.insert(nome, tipo);
    }

    pub fn obter_tipo(&self, nome: &str) -> Option<&Tipo> {
        self.tipos_inferidos.get(nome)
    }

    // ✅ NOVO: Inferir tipo de comando (para análise completa)
    pub fn inferir_tipo_comando(&mut self, comando: &Comando) -> Result<(), String> {
        match comando {
            Comando::DeclaracaoVariavel(tipo, nome, valor) => {
                if let Some(expr) = valor {
                    let tipo_inferido = self.inferir_tipo(expr)?;
                    // Verificar compatibilidade
                    if !self.tipos_compativeis(tipo, &tipo_inferido) {
                        return Err(format!(
                            "Tipo declarado '{}' incompatível com tipo inferido '{}'",
                            self.tipo_para_string(tipo),
                            self.tipo_para_string(&tipo_inferido)
                        ));
                    }
                }
                self.registrar_variavel(nome.clone(), tipo.clone());
            }

            Comando::DeclaracaoVar(nome, expr) => {
                let tipo_inferido = self.inferir_tipo(expr)?;
                self.registrar_variavel(nome.clone(), tipo_inferido);
            }

            Comando::Atribuicao(nome, expr) => {
                let tipo_expr = self.inferir_tipo(expr)?;
                if let Some(tipo_var) = self.obter_tipo(nome) {
                    if !self.tipos_compativeis(tipo_var, &tipo_expr) {
                        return Err(format!(
                            "Atribuição incompatível: variável '{}' do tipo '{}' recebendo '{}'",
                            nome,
                            self.tipo_para_string(tipo_var),
                            self.tipo_para_string(&tipo_expr)
                        ));
                    }
                }
            }

            Comando::Bloco(comandos) => {
                for cmd in comandos {
                    self.inferir_tipo_comando(cmd)?;
                }
            }

            Comando::Se(condicao, cmd_if, cmd_else) => {
                let tipo_cond = self.inferir_tipo(condicao)?;
                if !matches!(tipo_cond, Tipo::Booleano) {
                    return Err("Condição 'se' deve ser do tipo booleano".to_string());
                }

                self.inferir_tipo_comando(cmd_if)?;
                if let Some(cmd) = cmd_else {
                    self.inferir_tipo_comando(cmd)?;
                }
            }

            Comando::Enquanto(condicao, corpo) => {
                let tipo_cond = self.inferir_tipo(condicao)?;
                if !matches!(tipo_cond, Tipo::Booleano) {
                    return Err("Condição 'enquanto' deve ser do tipo booleano".to_string());
                }

                self.inferir_tipo_comando(corpo)?;
            }

            Comando::Para(inicializacao, condicao, incremento, corpo) => {
                if let Some(init) = inicializacao {
                    self.inferir_tipo_comando(init)?;
                }

                if let Some(cond) = condicao {
                    let tipo_cond = self.inferir_tipo(cond)?;
                    if !matches!(tipo_cond, Tipo::Booleano) {
                        return Err("Condição 'para' deve ser do tipo booleano".to_string());
                    }
                }

                self.inferir_tipo_comando(corpo)?;

                if let Some(inc) = incremento {
                    self.inferir_tipo_comando(inc)?;
                }
            }

            _ => {
                // Outros comandos não precisam de inferência especial
            }
        }

        Ok(())
    }

    // ✅ NOVO: Verificar compatibilidade de tipos
    // ✅ VERIFICAR: Se necessário, adicionar case para Decimal
    fn tipos_compativeis(&self, tipo1: &Tipo, tipo2: &Tipo) -> bool {
        match (tipo1, tipo2) {
            (Tipo::Enum(a), Tipo::Enum(b)) => a == b,
            (Tipo::Inteiro, Tipo::Inteiro) => true,
            (Tipo::Flutuante, Tipo::Flutuante) => true,
            (Tipo::Duplo, Tipo::Duplo) => true,
            (Tipo::Texto, Tipo::Texto) => true,
            (Tipo::Decimal, Tipo::Decimal) => true, // ✅ ADICIONAR se não existir
            (Tipo::Booleano, Tipo::Booleano) => true,
            (Tipo::Vazio, Tipo::Vazio) => true,
            (Tipo::Classe(c1), Tipo::Classe(c2)) => c1 == c2 || self.eh_subclasse(c2, c1),
            (Tipo::Lista(t1), Tipo::Lista(t2)) => self.tipos_compativeis(t1, t2),
            // Conversões implícitas
            (Tipo::Texto, _) => true,
            (Tipo::Decimal, Tipo::Inteiro) => true, // ✅ ADICIONAR: Conversão implícita
            (Tipo::Inteiro, Tipo::Decimal) => true, // ✅ ADICIONAR: Conversão implícita
            (Tipo::Flutuante, Tipo::Inteiro) => true, // permitir atribuir inteiro a flutuante
            (Tipo::Duplo, Tipo::Inteiro) => true,   // permitir atribuir inteiro a duplo
            (Tipo::Duplo, Tipo::Flutuante) => true, // permitir atribuir flutuante a duplo
            _ => false,
        }
    }

    // ✅ NOVO: Verificar se uma classe é subclasse de outra
    fn eh_subclasse(&self, classe_filha: &str, classe_pai: &str) -> bool {
        let mut atual = Some(classe_filha.to_string());

        while let Some(nome_classe) = atual {
            if let Some(def_classe) = self.classes.get(&nome_classe) {
                if let Some(pai) = &def_classe.classe_pai {
                    if pai == classe_pai {
                        return true;
                    }
                    atual = Some(pai.clone());
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        false
    }

    // ✅ NOVO: Converter tipo para string (para mensagens de erro)
    // ✅ CORREÇÃO: Adicionar case para Tipo::Decimal
    fn tipo_para_string(&self, tipo: &Tipo) -> String {
        match tipo {
            Tipo::Inteiro => "inteiro".to_string(),
            Tipo::Flutuante => "flutuante".to_string(),
            Tipo::Duplo => "duplo".to_string(),
            Tipo::Texto => "texto".to_string(),
            Tipo::Decimal => "decimal".to_string(), // ✅ ADICIONADO: Case faltante
            Tipo::Booleano => "booleano".to_string(),
            Tipo::Vazio => "vazio".to_string(),
            Tipo::Enum(nome) => format!("enum {}", nome),
            Tipo::Classe(nome) => nome.clone(),
            Tipo::Lista(t) => format!("lista<{}>", self.tipo_para_string(t)),
            Tipo::Funcao(params, ret) => {
                let params_str = params
                    .iter()
                    .map(|p| self.tipo_para_string(p))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("função({}) -> {}", params_str, self.tipo_para_string(ret))
            }
            Tipo::Generico(nome) => format!("generico<{}>", nome),
            Tipo::Opcional(tipo_interno) => {
                format!("opcional<{}>", self.tipo_para_string(tipo_interno))
            }
            Tipo::Inferido => "inferido".to_string(),
        }
    }
}
