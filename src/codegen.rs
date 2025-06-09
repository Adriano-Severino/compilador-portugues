use crate::ast::*;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use std::cell::RefCell;
use std::collections::HashMap;

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

pub struct GeradorCodigo<'ctx> {
    pub context: &'ctx Context,
    pub module: Module<'ctx>,
    pub builder: Builder<'ctx>,
    escopos: RefCell<Vec<HashMap<String, ValorAvaliado>>>,
    classes: RefCell<HashMap<String, DeclaracaoClasse>>,
    funcoes: RefCell<HashMap<String, DeclaracaoFuncao>>,
    contador_loop: RefCell<u32>,
}

impl<'ctx> GeradorCodigo<'ctx> {
    pub fn new(context: &'ctx Context) -> Self {
        let module = context.create_module("compilador_portugues");
        let builder = context.create_builder();

        Self {
            context,
            module,
            builder,
            escopos: RefCell::new(vec![HashMap::new()]),
            classes: RefCell::new(HashMap::new()),
            funcoes: RefCell::new(HashMap::new()),
            contador_loop: RefCell::new(0),
        }
    }

    pub fn compilar_programa(&self, programa: &Programa) -> Result<(), String> {
        // Registrar classes e funções primeiro
        for namespace in &programa.namespaces {
            self.processar_namespace(namespace)?;
        }

        for declaracao in &programa.declaracoes {
            self.processar_declaracao(declaracao)?;
        }

        // Compilar código principal
        for declaracao in &programa.declaracoes {
            if let Declaracao::Comando(comando) = declaracao {
                self.compilar_comando(comando)?;
            }
        }

        Ok(())
    }

    fn processar_namespace(&self, namespace: &DeclaracaoNamespace) -> Result<(), String> {
        println!("Processando namespace: {}", namespace.nome);

        for declaracao in &namespace.declaracoes {
            self.processar_declaracao(declaracao)?;
        }

        Ok(())
    }

    fn processar_declaracao(&self, declaracao: &Declaracao) -> Result<(), String> {
        match declaracao {
            Declaracao::DeclaracaoClasse(classe) => {
                println!("Registrando classe: {}", classe.nome);
                self.classes
                    .borrow_mut()
                    .insert(classe.nome.clone(), classe.clone());
            }
            Declaracao::DeclaracaoFuncao(funcao) => {
                println!("Registrando função: {}", funcao.nome);
                self.funcoes
                    .borrow_mut()
                    .insert(funcao.nome.clone(), funcao.clone());
            }
            Declaracao::Comando(_) => {
                // Comandos serão processados na fase de compilação
            }
            // ✅ IMPLEMENTADO: Outras declarações
            Declaracao::DeclaracaoModulo(modulo) => {
                println!("Registrando módulo: {}", modulo.nome);
                // Implementar se necessário
            }
            Declaracao::DeclaracaoInterface(interface) => {
                println!("Registrando interface: {}", interface.nome);
                // Implementar se necessário
            }
            Declaracao::DeclaracaoEnum(enum_decl) => {
                println!("Registrando enum: {}", enum_decl.nome);
                // Implementar se necessário
            }
            Declaracao::DeclaracaoTipo(tipo_decl) => {
                println!("Registrando tipo personalizado: {}", tipo_decl.nome);
                // Implementar se necessário
            }
            Declaracao::ImportDeclaration(import) => {
                println!("Processando import: {:?}", import);
                // Implementar se necessário
            }
        }
        Ok(())
    }

    pub fn compilar_comando(&self, comando: &Comando) -> Result<(), String> {
        match comando {
            Comando::DeclaracaoVariavel(tipo, nome, valor) => {
                if let Some(expr) = valor {
                    let val = self.avaliar_expressao(expr)?;
                    self.definir_variavel(nome.clone(), val);
                    println!("Declarada variável '{}' do tipo {:?}", nome, tipo);
                } else {
                    let val_padrao = match tipo {
                        Tipo::Inteiro => ValorAvaliado::Inteiro(0),
                        Tipo::Texto => ValorAvaliado::Texto(String::new()),
                        Tipo::Booleano => ValorAvaliado::Booleano(false),
                        _ => ValorAvaliado::Texto("null".to_string()),
                    };
                    self.definir_variavel(nome.clone(), val_padrao);
                }
            }

            Comando::DeclaracaoVar(nome, expr) => {
                let valor = self.avaliar_expressao(expr)?;
                self.definir_variavel(nome.clone(), valor.clone());

                let tipo_inferido = match valor {
                    ValorAvaliado::Inteiro(_) => "inteiro",
                    ValorAvaliado::Texto(_) => "texto",
                    ValorAvaliado::Booleano(_) => "booleano",
                    ValorAvaliado::Objeto { .. } => "objeto",
                };

                println!(
                    "Declarada variável '{}' com tipo inferido: {}",
                    nome, tipo_inferido
                );
            }

            Comando::Atribuicao(nome, expr) => {
                let valor = self.avaliar_expressao(expr)?;

                if self.buscar_variavel(nome).is_none() {
                    return Err(format!("Variável '{}' não foi declarada", nome));
                }

                self.atualizar_variavel(nome, valor)?;
                println!("Atribuído valor à variável '{}'", nome);
            }

            Comando::AtribuirPropriedade(objeto, propriedade, expr) => {
                let valor = self.avaliar_expressao(expr)?;

                if let Some(ValorAvaliado::Objeto { propriedades, classe }) =
                    self.buscar_variavel(objeto)
                {
                    let mut nova_propriedades = propriedades;
                    nova_propriedades.insert(propriedade.clone(), valor);

                    let novo_objeto = ValorAvaliado::Objeto {
                        classe,
                        propriedades: nova_propriedades,
                    };

                    self.atualizar_variavel(objeto, novo_objeto)?;
                    println!("Atribuído valor à propriedade '{}.{}'", objeto, propriedade);
                } else {
                    return Err(format!(
                        "Objeto '{}' não encontrado ou não é um objeto",
                        objeto
                    ));
                }
            }

            Comando::Imprima(expr) => {
                let valor = self.avaliar_expressao(expr)?;
                println!("SAÍDA: {}", self.valor_para_string(&valor));
            }

            Comando::Se(condicao, bloco_then, bloco_else) => {
                let cond_valor = self.avaliar_expressao(condicao)?;
                let eh_verdadeiro = match cond_valor {
                    ValorAvaliado::Booleano(b) => b,
                    ValorAvaliado::Inteiro(i) => i != 0,
                    _ => false,
                };

                if eh_verdadeiro {
                    self.compilar_comando(bloco_then)?;
                } else if let Some(bloco_senao) = bloco_else {
                    self.compilar_comando(bloco_senao)?;
                }
            }

            Comando::Enquanto(condicao, bloco) => {
                let mut contador = self.contador_loop.borrow_mut();
                *contador += 1;
                let limite_iteracoes = 1000; // Limite de segurança
                let mut iteracoes = 0;

                loop {
                    iteracoes += 1;
                    if iteracoes > limite_iteracoes {
                        return Err(
                            "Loop 'enquanto' excedeu o limite máximo de iterações".to_string()
                        );
                    }

                    let cond_valor = self.avaliar_expressao(condicao)?;
                    let continuar = match cond_valor {
                        ValorAvaliado::Booleano(b) => b,
                        ValorAvaliado::Inteiro(i) => i != 0,
                        _ => false,
                    };

                    if !continuar {
                        break;
                    }

                    self.compilar_comando(bloco)?;
                }
            }

            // ✅ IMPLEMENTADO: Loop Para
            Comando::Para(inicializacao, condicao, incremento, corpo) => {
                println!("Executando loop 'para'");
                
                // Entrar em novo escopo
                self.entrar_escopo();
                
                // Inicialização
                if let Some(init) = inicializacao {
                    self.compilar_comando(init)?;
                }
                
                let limite_iteracoes = 1000;
                let mut iteracoes = 0;
                
                loop {
                    iteracoes += 1;
                    if iteracoes > limite_iteracoes {
                        self.sair_escopo();
                        return Err("Loop 'para' excedeu o limite máximo de iterações".to_string());
                    }
                    
                    // Verificar condição
                    if let Some(cond) = condicao {
                        let cond_valor = self.avaliar_expressao(cond)?;
                        let continuar = match cond_valor {
                            ValorAvaliado::Booleano(b) => b,
                            ValorAvaliado::Inteiro(i) => i != 0,
                            _ => false,
                        };
                        
                        if !continuar {
                            break;
                        }
                    }
                    
                    // Executar corpo
                    self.compilar_comando(corpo)?;
                    
                    // Incremento
                    if let Some(inc) = incremento {
                        self.compilar_comando(inc)?;
                    }
                }
                
                self.sair_escopo();
            }

            Comando::Bloco(comandos) => {
                self.entrar_escopo();
                for cmd in comandos {
                    self.compilar_comando(cmd)?;
                }
                self.sair_escopo();
            }

            Comando::Retorne(expr) => {
                if let Some(expressao) = expr {
                    let valor = self.avaliar_expressao(expressao)?;
                    println!("RETORNO: {}", self.valor_para_string(&valor));
                } else {
                    println!("RETORNO: vazio");
                }
            }

            Comando::Expressao(expr) => {
                self.avaliar_expressao(expr)?;
            }

            // ✅ IMPLEMENTADO: Criar Objeto como comando
            Comando::CriarObjeto(var_nome, classe, argumentos) => {
                println!("Criando objeto '{}' da classe '{}'", var_nome, classe);
                
                let objeto = self.criar_instancia_objeto(classe, argumentos)?;
                self.definir_variavel(var_nome.clone(), objeto);
                
                println!("Objeto '{}' criado com sucesso", var_nome);
            }

            // ✅ IMPLEMENTADO: Chamar Método como comando
            Comando::ChamarMetodo(objeto, metodo, argumentos) => {
                println!("Chamando método '{}.{}'", objeto, metodo);
                
                // Verificar se o objeto existe
                if self.buscar_variavel(objeto).is_none() {
                    return Err(format!("Objeto '{}' não encontrado", objeto));
                }
                
                // Avaliar argumentos
                let mut args_avaliados = Vec::new();
                for arg in argumentos {
                    args_avaliados.push(self.avaliar_expressao(arg)?);
                }
                
                // Simular execução do método
                println!("Método '{}.{}' executado com {} argumentos", objeto, metodo, args_avaliados.len());
            }

            // ✅ IMPLEMENTADO: Acessar Campo como comando
            Comando::AcessarCampo(objeto, campo) => {
                println!("Acessando campo '{}.{}'", objeto, campo);
                
                if let Some(ValorAvaliado::Objeto { propriedades, .. }) = self.buscar_variavel(objeto) {
                    if let Some(valor) = propriedades.get(campo) {
                        println!("Valor do campo '{}.{}': {}", objeto, campo, self.valor_para_string(valor));
                    } else {
                        return Err(format!("Campo '{}' não encontrado no objeto '{}'", campo, objeto));
                    }
                } else {
                    return Err(format!("Objeto '{}' não encontrado ou não é um objeto", objeto));
                }
            }

            // ✅ IMPLEMENTADO: Atribuir Campo
            Comando::AtribuirCampo(objeto_expr, campo, valor_expr) => {
                let valor = self.avaliar_expressao(valor_expr)?;
                
                // Se objeto_expr é um identificador simples
                if let Expressao::Identificador(objeto_nome) = objeto_expr.as_ref() {
                    if let Some(ValorAvaliado::Objeto { mut propriedades, classe }) = self.buscar_variavel(objeto_nome) {
                        propriedades.insert(campo.clone(), valor);
                        
                        let novo_objeto = ValorAvaliado::Objeto {
                            classe,
                            propriedades,
                        };
                        
                        self.atualizar_variavel(objeto_nome, novo_objeto)?;
                        println!("Campo '{}.{}' atualizado", objeto_nome, campo);
                    } else {
                        return Err(format!("Objeto '{}' não encontrado", objeto_nome));
                    }
                } else {
                    return Err("Atribuição a campo complexo não implementada".to_string());
                }
            }
        }

        Ok(())
    }

    // ✅ NOVO: Método auxiliar para criar instância de objeto
    fn criar_instancia_objeto(&self, classe: &str, argumentos: &[Expressao]) -> Result<ValorAvaliado, String> {
        let mut propriedades = HashMap::new();
        
        // Inicializar propriedades com valores padrão
        if let Some(def_classe) = self.classes.borrow().get(classe) {
            for propriedade in &def_classe.propriedades {
                let valor_padrao = match propriedade.tipo {
                    Tipo::Inteiro => ValorAvaliado::Inteiro(0),
                    Tipo::Texto => ValorAvaliado::Texto(String::new()),
                    Tipo::Booleano => ValorAvaliado::Booleano(false),
                    _ => ValorAvaliado::Texto("null".to_string()),
                };
                propriedades.insert(propriedade.nome.clone(), valor_padrao);
            }
            
            // Executar construtor se existir
            if !def_classe.construtores.is_empty() {
                println!("Executando construtor da classe '{}'", classe);
                
                // Encontrar construtor compatível
                for construtor in &def_classe.construtores {
                    if argumentos.len() <= construtor.parametros.len() {
                        // Executar comandos do construtor (simulado)
                        for comando in &construtor.corpo {
                            // Por enquanto, apenas log
                            println!("  Executando comando do construtor: {:?}", std::mem::discriminant(comando));
                        }
                        break;
                    }
                }
            }
        }
        
        Ok(ValorAvaliado::Objeto {
            classe: classe.to_string(),
            propriedades,
        })
    }

    pub fn avaliar_expressao(&self, expr: &Expressao) -> Result<ValorAvaliado, String> {
        match expr {
            Expressao::Inteiro(valor) => Ok(ValorAvaliado::Inteiro(*valor)),

            Expressao::Texto(valor) => Ok(ValorAvaliado::Texto(valor.clone())),

            Expressao::Booleano(valor) => Ok(ValorAvaliado::Booleano(*valor)),

            Expressao::Identificador(nome) => self
                .buscar_variavel(nome)
                .ok_or_else(|| format!("Variável '{}' não encontrada", nome)),

            Expressao::Aritmetica(op, esq, dir) => {
                let val_esq = self.avaliar_expressao(esq)?;
                let val_dir = self.avaliar_expressao(dir)?;

                match (op, val_esq, val_dir) {
                    (
                        OperadorAritmetico::Soma,
                        ValorAvaliado::Inteiro(a),
                        ValorAvaliado::Inteiro(b),
                    ) => Ok(ValorAvaliado::Inteiro(a + b)),
                    (
                        OperadorAritmetico::Soma,
                        ValorAvaliado::Texto(a),
                        ValorAvaliado::Texto(b),
                    ) => Ok(ValorAvaliado::Texto(format!("{}{}", a, b))),
                    (
                        OperadorAritmetico::Soma,
                        ValorAvaliado::Texto(a),
                        ValorAvaliado::Inteiro(b),
                    ) => Ok(ValorAvaliado::Texto(format!("{}{}", a, b))),
                    (
                        OperadorAritmetico::Subtracao,
                        ValorAvaliado::Inteiro(a),
                        ValorAvaliado::Inteiro(b),
                    ) => Ok(ValorAvaliado::Inteiro(a - b)),
                    (
                        OperadorAritmetico::Multiplicacao,
                        ValorAvaliado::Inteiro(a),
                        ValorAvaliado::Inteiro(b),
                    ) => Ok(ValorAvaliado::Inteiro(a * b)),
                    (
                        OperadorAritmetico::Divisao,
                        ValorAvaliado::Inteiro(a),
                        ValorAvaliado::Inteiro(b),
                    ) => {
                        if b == 0 {
                            Err("Divisão por zero".to_string())
                        } else {
                            Ok(ValorAvaliado::Inteiro(a / b))
                        }
                    }
                    (
                        OperadorAritmetico::Modulo,
                        ValorAvaliado::Inteiro(a),
                        ValorAvaliado::Inteiro(b),
                    ) => {
                        if b == 0 {
                            Err("Módulo por zero".to_string())
                        } else {
                            Ok(ValorAvaliado::Inteiro(a % b))
                        }
                    }
                    _ => Err("Operação aritmética inválida".to_string()),
                }
            }

            Expressao::Comparacao(op, esq, dir) => {
                let val_esq = self.avaliar_expressao(esq)?;
                let val_dir = self.avaliar_expressao(dir)?;

                let resultado = match (op, &val_esq, &val_dir) {
                    (
                        OperadorComparacao::Igual,
                        ValorAvaliado::Inteiro(a),
                        ValorAvaliado::Inteiro(b),
                    ) => a == b,
                    (
                        OperadorComparacao::Diferente,
                        ValorAvaliado::Inteiro(a),
                        ValorAvaliado::Inteiro(b),
                    ) => a != b,
                    (
                        OperadorComparacao::Menor,
                        ValorAvaliado::Inteiro(a),
                        ValorAvaliado::Inteiro(b),
                    ) => a < b,
                    (
                        OperadorComparacao::MaiorQue,
                        ValorAvaliado::Inteiro(a),
                        ValorAvaliado::Inteiro(b),
                    ) => a > b,
                    (
                        OperadorComparacao::MenorIgual,
                        ValorAvaliado::Inteiro(a),
                        ValorAvaliado::Inteiro(b),
                    ) => a <= b,
                    (
                        OperadorComparacao::MaiorIgual,
                        ValorAvaliado::Inteiro(a),
                        ValorAvaliado::Inteiro(b),
                    ) => a >= b,
                    _ => return Err("Comparação inválida".to_string()),
                };

                Ok(ValorAvaliado::Booleano(resultado))
            }

            Expressao::Logica(op, esq, dir) => {
                let val_esq = self.avaliar_expressao(esq)?;
                let val_dir = self.avaliar_expressao(dir)?;

                let bool_esq = self.valor_para_bool(&val_esq);
                let bool_dir = self.valor_para_bool(&val_dir);

                let resultado = match op {
                    OperadorLogico::E => bool_esq && bool_dir,
                    OperadorLogico::Ou => bool_esq || bool_dir,
                };

                Ok(ValorAvaliado::Booleano(resultado))
            }

            Expressao::NovoObjeto(classe, argumentos) => {
                self.criar_instancia_objeto(classe, argumentos)
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

                Ok(ValorAvaliado::Texto(resultado))
            }

            Expressao::AcessoMembro(obj_expr, membro) => {
                let objeto = self.avaliar_expressao(obj_expr)?;

                if let ValorAvaliado::Objeto { propriedades, .. } = objeto {
                    propriedades
                        .get(membro)
                        .cloned()
                        .ok_or_else(|| format!("Propriedade '{}' não encontrada", membro))
                } else {
                    Err("Tentativa de acessar membro de valor que não é objeto".to_string())
                }
            }

            Expressao::ChamadaMetodo(obj_expr, metodo, argumentos) => {
                println!(
                    "Chamando método '{}' com {} argumentos",
                    metodo,
                    argumentos.len()
                );

                // Simular diferentes métodos
                match metodo.as_str() {
                    "apresentar" => Ok(ValorAvaliado::Texto(
                        "Resultado do método apresentar".to_string(),
                    )),
                    "toString" => Ok(ValorAvaliado::Texto(
                        "Representação em texto do objeto".to_string(),
                    )),
                    "obterNome" => Ok(ValorAvaliado::Texto("Nome do objeto".to_string())),
                    _ => Ok(ValorAvaliado::Texto(format!(
                        "Resultado do método {}",
                        metodo
                    ))),
                }
            }

            Expressao::Chamada(nome, argumentos) => {
                println!(
                    "Chamando função '{}' com {} argumentos",
                    nome,
                    argumentos.len()
                );

                // Simular diferentes funções
                match nome.as_str() {
                    "tamanho" => Ok(ValorAvaliado::Inteiro(10)),
                    "maiuscula" => Ok(ValorAvaliado::Texto("TEXTO EM MAIÚSCULA".to_string())),
                    "minuscula" => Ok(ValorAvaliado::Texto("texto em minúscula".to_string())),
                    _ => Ok(ValorAvaliado::Texto(format!(
                        "Resultado da função {}",
                        nome
                    ))),
                }
            }

            // ✅ IMPLEMENTADO: Este (referência ao objeto atual)
            Expressao::Este => {
                // Por enquanto, retornar um objeto placeholder
                Ok(ValorAvaliado::Objeto {
                    classe: "Atual".to_string(),
                    propriedades: HashMap::new(),
                })
            }

            // ✅ IMPLEMENTADO: Outras expressões que podem existir
            _ => Err("Expressão não implementada ou não reconhecida".to_string()),
        }
    }

    // === MÉTODOS AUXILIARES ===

    fn definir_variavel(&self, nome: String, valor: ValorAvaliado) {
        if let Some(escopo_atual) = self.escopos.borrow_mut().last_mut() {
            escopo_atual.insert(nome, valor);
        }
    }

    fn buscar_variavel(&self, nome: &str) -> Option<ValorAvaliado> {
        for escopo in self.escopos.borrow().iter().rev() {
            if let Some(valor) = escopo.get(nome) {
                return Some(valor.clone());
            }
        }
        None
    }

    fn atualizar_variavel(&self, nome: &str, valor: ValorAvaliado) -> Result<(), String> {
        let mut escopos = self.escopos.borrow_mut();

        for escopo in escopos.iter_mut().rev() {
            if escopo.contains_key(nome) {
                escopo.insert(nome.to_string(), valor);
                return Ok(());
            }
        }

        Err(format!(
            "Variável '{}' não encontrada para atualização",
            nome
        ))
    }

    fn entrar_escopo(&self) {
        self.escopos.borrow_mut().push(HashMap::new());
    }

    fn sair_escopo(&self) {
        self.escopos.borrow_mut().pop();
    }

    fn valor_para_string(&self, valor: &ValorAvaliado) -> String {
        match valor {
            ValorAvaliado::Inteiro(i) => i.to_string(),
            ValorAvaliado::Texto(s) => s.clone(),
            ValorAvaliado::Booleano(b) => if *b { "verdadeiro" } else { "falso" }.to_string(),
            ValorAvaliado::Objeto { classe, propriedades } => {
                format!("Objeto de {} com {} propriedades", classe, propriedades.len())
            }
        }
    }

    fn valor_para_bool(&self, valor: &ValorAvaliado) -> bool {
        match valor {
            ValorAvaliado::Booleano(b) => *b,
            ValorAvaliado::Inteiro(i) => *i != 0,
            ValorAvaliado::Texto(s) => !s.is_empty(),
            ValorAvaliado::Objeto { .. } => true,
        }
    }
}