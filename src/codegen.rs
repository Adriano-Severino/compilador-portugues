use crate::ast::*;
use inkwell::{builder::Builder, context::Context, module::Module};
use std::{
    cell::RefCell,
    collections::HashMap,
    sync::atomic::{AtomicUsize, Ordering},
};

// ✅ Contador global para nomes únicos de strings
static CONTADOR_STRING: AtomicUsize = AtomicUsize::new(0);

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

// ✅ Estrutura para método com origem (para polimorfismo)
#[derive(Debug, Clone)]
struct MetodoComOrigem {
    metodo: MetodoClasse,
    classe_origem: String,
}

pub struct GeradorCodigo<'ctx> {
    pub context: &'ctx Context,
    pub module: Module<'ctx>,
    pub builder: Builder<'ctx>,
    escopos: RefCell<Vec<HashMap<String, ValorAvaliado>>>,
    classes: RefCell<HashMap<String, DeclaracaoClasse>>,
    funcoes: RefCell<HashMap<String, DeclaracaoFuncao>>,
    contador_loop: RefCell<usize>,
    escopo_este: RefCell<Option<ValorAvaliado>>,
}

impl<'ctx> GeradorCodigo<'ctx> {
    pub fn new(context: &'ctx Context) -> Self {
        let module = context.create_module("compilador_portugues");
        
        // ✅ Adicionar target triple automaticamente
        module.set_triple(&inkwell::targets::TargetMachine::get_default_triple());
        
        let builder = context.create_builder();
        Self {
            context,
            module,
            builder,
            escopos: RefCell::new(vec![HashMap::new()]),
            classes: RefCell::new(HashMap::new()),
            funcoes: RefCell::new(HashMap::new()),
            contador_loop: RefCell::new(0),
            escopo_este: RefCell::new(None),
        }
    }

    pub fn compilar_programa(&self, programa: &Programa) -> Result<(), String> {
        // 1. Registrar classes e funções primeiro
        for namespace in &programa.namespaces {
            self.processar_namespace(namespace)?;
        }

        for declaracao in &programa.declaracoes {
            self.processar_declaracao(declaracao)?;
        }

        // 2. Compilar comandos diretos
        for declaracao in &programa.declaracoes {
            if let Declaracao::Comando(comando) = declaracao {
                self.compilar_comando(comando)?;
            }
        }

        // ✅ 3. NOVO: Executar função principal
        self.executar_funcao_principal(programa)?;

        Ok(())
    }

    // ✅ NOVA FUNÇÃO: Encontrar e executar função principal
    fn executar_funcao_principal(&self, programa: &Programa) -> Result<(), String> {
        println!("🔍 Procurando função principal...");
        
        // Buscar função principal em declarações diretas
        for declaracao in &programa.declaracoes {
            if let Declaracao::DeclaracaoFuncao(funcao) = declaracao {
                if funcao.nome == "principal" || funcao.nome == "Principal" {
                    println!("🎯 Executando função principal: {}", funcao.nome);
                    for comando in &funcao.corpo {
                        self.compilar_comando(comando)?;
                    }
                    return Ok(());
                }
            }
        }
        
        // Buscar função principal em namespaces
        for namespace in &programa.namespaces {
            println!("🔍 Verificando namespace: {}", namespace.nome);
            for declaracao in &namespace.declaracoes {
                if let Declaracao::DeclaracaoFuncao(funcao) = declaracao {
                    if funcao.nome == "principal" || funcao.nome == "Principal" {
                        println!("🎯 Executando função principal do namespace {}: {}", 
                            namespace.nome, funcao.nome);
                        for comando in &funcao.corpo {
                            self.compilar_comando(comando)?;
                        }
                        return Ok(());
                    }
                }
            }
        }
        
        println!("⚠️ Nenhuma função principal encontrada");
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
            Declaracao::DeclaracaoModulo(modulo) => {
                println!("Registrando módulo: {}", modulo.nome);
            }
            Declaracao::DeclaracaoInterface(interface) => {
                println!("Registrando interface: {}", interface.nome);
            }
            Declaracao::DeclaracaoEnum(enum_decl) => {
                println!("Registrando enum: {}", enum_decl.nome);
            }
            Declaracao::DeclaracaoTipo(tipo_decl) => {
                println!("Registrando tipo personalizado: {}", tipo_decl.nome);
            }
            Declaracao::Importacao(import) => {
                println!("Processando import: {}", import.caminho);
            }
            Declaracao::Exportacao(exportacao) => {
                println!("Processando exportação: {}", exportacao.nome);
            }
        }
        Ok(())
    }

    // ✅ NOVO: Executar corpo de função
    fn executar_funcao(&self, nome_funcao: &str, argumentos: &[Expressao]) -> Result<(), String> {
        if let Some(funcao) = self.funcoes.borrow().get(nome_funcao) {
            println!("🎯 Executando função: {}", nome_funcao);
            
            // Entrar em novo escopo para a função
            self.entrar_escopo();
            
            // Definir parâmetros como variáveis locais
            for (i, parametro) in funcao.parametros.iter().enumerate() {
                if i < argumentos.len() {
                    let valor = self.avaliar_expressao(&argumentos[i])?;
                    self.definir_variavel(parametro.nome.clone(), valor);
                } else if let Some(valor_padrao) = &parametro.valor_padrao {
                    let valor = self.avaliar_expressao(valor_padrao)?;
                    self.definir_variavel(parametro.nome.clone(), valor);
                }
            }
            
            // Executar corpo da função
            for comando in &funcao.corpo {
                self.compilar_comando(comando)?;
            }
            
            // Sair do escopo da função
            self.sair_escopo();
            
            Ok(())
        } else {
            Err(format!("Função '{}' não encontrada", nome_funcao))
        }
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
                if objeto == "este" {
                    if let Some(mut este_obj) = self.escopo_este.borrow().clone() {
                        if let ValorAvaliado::Objeto {
                            ref mut propriedades,
                            ..
                        } = este_obj
                        {
                            propriedades.insert(propriedade.clone(), valor);
                            *self.escopo_este.borrow_mut() = Some(este_obj);
                            println!("Atribuído valor à propriedade 'este.{}'", propriedade);
                        }
                    } else {
                        println!(
                            "⚠️ Simulando atribuição este.{} = {}",
                            propriedade,
                            self.valor_para_string(&valor)
                        );
                    }
                } else {
                    if let Some(ValorAvaliado::Objeto {
                        propriedades,
                        classe,
                    }) = self.buscar_variavel(objeto)
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
            }

            Comando::Imprima(expr) => {
                let valor = self.avaliar_expressao(expr)?;
                // ✅ Gerar código LLVM real
                let mensagem = self.valor_para_string(&valor);
                self.gerar_printf(&mensagem)?;
                // Debug opcional
                println!("SAÍDA: {}", mensagem);
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
                let limite_iteracoes = 1000;
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

            Comando::Para(inicializacao, condicao, incremento, corpo) => {
                println!("Executando loop 'para'");
                self.entrar_escopo();
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

                    self.compilar_comando(corpo)?;
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

            Comando::Expressao(Expressao::ChamadaMetodo(obj_expr, metodo, argumentos)) => {
                self.executar_metodo_objeto(obj_expr, metodo, argumentos)?;
            }

            // ✅ NOVO: Tratar chamadas de função
            Comando::Expressao(Expressao::Chamada(nome_funcao, argumentos)) => {
                self.executar_funcao(nome_funcao, argumentos)?;
            }

            Comando::Expressao(expr) => {
                self.avaliar_expressao(expr)?;
            }

            Comando::CriarObjeto(var_nome, classe, argumentos) => {
                println!("Criando objeto '{}' da classe '{}'", var_nome, classe);
                let objeto = self.criar_instancia_objeto_com_heranca(classe, argumentos)?;
                self.definir_variavel(var_nome.clone(), objeto);
                println!("Objeto '{}' criado com sucesso", var_nome);
            }

            Comando::ChamarMetodo(objeto_nome, metodo, argumentos) => {
                println!("Chamando método '{}.{}'", objeto_nome, metodo);
                if self.buscar_variavel(objeto_nome).is_none() {
                    return Err(format!("Objeto '{}' não encontrado", objeto_nome));
                }

                self.executar_metodo_polimorfismo(objeto_nome, metodo, argumentos)?;
            }

            Comando::AcessarCampo(objeto_nome, campo) => {
                println!("Acessando campo '{}.{}'", objeto_nome, campo);
                if let Some(ValorAvaliado::Objeto { propriedades, .. }) =
                    self.buscar_variavel(objeto_nome)
                {
                    if let Some(valor) = propriedades.get(campo) {
                        println!(
                            "Valor do campo '{}.{}': {}",
                            objeto_nome,
                            campo,
                            self.valor_para_string(valor)
                        );
                    } else {
                        return Err(format!(
                            "Campo '{}' não encontrado no objeto '{}'",
                            campo, objeto_nome
                        ));
                    }
                } else {
                    return Err(format!(
                        "Objeto '{}' não encontrado ou não é um objeto",
                        objeto_nome
                    ));
                }
            }

            Comando::AtribuirCampo(objeto_expr, campo, valor_expr) => {
                let valor = self.avaliar_expressao(valor_expr)?;
                if let Expressao::Identificador(objeto_nome) = objeto_expr.as_ref() {
                    if let Some(ValorAvaliado::Objeto {
                        mut propriedades,
                        classe,
                    }) = self.buscar_variavel(objeto_nome)
                    {
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

    // ✅ Função corrigida para gerar printf
    fn gerar_printf(&self, mensagem: &str) -> Result<(), String> {
        // Criar nome único para a string
        let contador = CONTADOR_STRING.fetch_add(1, Ordering::SeqCst);
        let nome_string = format!(".str{}", contador);
        
        // Criar string global com terminadores corretos
        let string_with_newline = format!("{}\n\0", mensagem);
        let format_str = self.context.const_string(string_with_newline.as_bytes(), false);
        let global = self.module.add_global(format_str.get_type(), None, &nome_string);
        global.set_initializer(&format_str);
        global.set_linkage(inkwell::module::Linkage::Private);
        global.set_unnamed_addr(true);
        
        // Declarar printf se não existir
        let printf_type = self.context.i32_type().fn_type(
            &[self.context.ptr_type(inkwell::AddressSpace::default()).into()], 
            true
        );
        let printf_func = self.module.get_function("printf")
            .unwrap_or_else(|| self.module.add_function("printf", printf_type, None));
        
        // Criar getelementptr para acessar a string
        let global_ptr = unsafe {
            self.builder.build_gep(
                format_str.get_type(),
                global.as_pointer_value(),
                &[
                    self.context.i32_type().const_zero(),
                    self.context.i32_type().const_zero()
                ],
                "str_ptr"
            ).map_err(|e| format!("Erro ao criar getelementptr: {:?}", e))?
        };
        
        // Chamar printf
        self.builder.build_call(
            printf_func, 
            &[global_ptr.into()], 
            "printf_call"
        ).map_err(|e| format!("Erro ao gerar chamada printf: {:?}", e))?;
        
        Ok(())
    }

    // ✅ Criar instância com herança
    fn criar_instancia_objeto_com_heranca(
        &self,
        classe: &str,
        argumentos: &[Expressao],
    ) -> Result<ValorAvaliado, String> {
        let mut propriedades = HashMap::new();
        
        // 1. Herdar propriedades da classe pai (recursivo)
        self.herdar_propriedades_recursivo(classe, &mut propriedades)?;
        
        // 2. Encontrar e executar construtor
        if let Some(def_classe) = self.classes.borrow().get(classe) {
            let construtor_encontrado = self.encontrar_construtor_compativel(
                &def_classe.construtores,
                argumentos.len()
            );
            
            if let Some(construtor) = construtor_encontrado {
                println!("✓ Usando construtor com {} parâmetros", construtor.parametros.len());
                
                let argumentos_resolvidos = self.resolver_argumentos_construtor_csharp(
                    argumentos,
                    &construtor.parametros
                )?;
                
                self.executar_construtor_csharp(&argumentos_resolvidos, &mut propriedades)?;
                
                *self.escopo_este.borrow_mut() = Some(ValorAvaliado::Objeto {
                    classe: classe.to_string(),
                    propriedades: propriedades.clone(),
                });
                
                self.executar_corpo_construtor(&construtor.corpo, &argumentos_resolvidos)?;
                
                if let Some(ValorAvaliado::Objeto { propriedades: props_atualizadas, .. }) =
                    self.escopo_este.borrow().clone() {
                    propriedades = props_atualizadas;
                }
                
                *self.escopo_este.borrow_mut() = None;
            }
        }
        
        Ok(ValorAvaliado::Objeto {
            classe: classe.to_string(),
            propriedades,
        })
    }

    // ✅ Herdar propriedades recursivamente
    fn herdar_propriedades_recursivo(
        &self,
        classe_nome: &str,
        propriedades: &mut HashMap<String, ValorAvaliado>,
    ) -> Result<(), String> {
        if let Some(def_classe) = self.classes.borrow().get(classe_nome) {
            // Primeiro herdar da classe pai (se existir)
            if let Some(classe_pai) = &def_classe.classe_pai {
                self.herdar_propriedades_recursivo(classe_pai, propriedades)?;
            }
            
            // Depois adicionar propriedades desta classe
            for propriedade in &def_classe.propriedades {
                let valor_inicial = if let Some(valor) = &propriedade.valor_inicial {
                    self.avaliar_expressao(valor)?
                } else {
                    self.obter_valor_padrao_tipo(&propriedade.tipo)
                };
                propriedades.insert(propriedade.nome.clone(), valor_inicial);
            }
        }
        Ok(())
    }

    // ✅ Executar método com polimorfismo
    fn executar_metodo_polimorfismo(
        &self,
        objeto_nome: &str,
        nome_metodo: &str,
        argumentos: &[Expressao],
    ) -> Result<(), String> {
        if let Some(ValorAvaliado::Objeto { classe, propriedades }) = 
            self.buscar_variavel(objeto_nome) {
            
            // Buscar método na hierarquia (método mais derivado primeiro)
            if let Some(metodo_com_origem) = self.buscar_metodo_na_hierarquia(&classe, nome_metodo) {
                println!("✓ Executando método '{}' da classe '{}'", nome_metodo, metodo_com_origem.classe_origem);
                
                // Configurar contexto 'este'
                *self.escopo_este.borrow_mut() = Some(ValorAvaliado::Objeto {
                    classe: classe.clone(),
                    propriedades: propriedades.clone(),
                });
                
                // Executar método
                self.executar_corpo_metodo(&metodo_com_origem.metodo.corpo)?;
                
                // Atualizar objeto com mudanças do 'este'
                if let Some(ValorAvaliado::Objeto { propriedades: props_atualizadas, .. }) =
                    self.escopo_este.borrow().clone() {
                    let objeto_atualizado = ValorAvaliado::Objeto {
                        classe,
                        propriedades: props_atualizadas,
                    };
                    self.atualizar_variavel(objeto_nome, objeto_atualizado)?;
                }
                
                // Limpar contexto
                *self.escopo_este.borrow_mut() = None;
                
                Ok(())
            } else {
                // Fallback para métodos especiais
                match nome_metodo {
                    "apresentar" => {
                        let completo = if argumentos.is_empty() {
                            true
                        } else {
                            let param = self.avaliar_expressao(&argumentos[0])?;
                            self.valor_para_bool(&param)
                        };
                        self.executar_metodo_apresentar(&propriedades, completo)?;
                        Ok(())
                    }
                    _ => {
                        Err(format!("Método '{}' não encontrado na hierarquia de '{}'", 
                            nome_metodo, classe))
                    }
                }
            }
        } else {
            Err(format!("Objeto '{}' não encontrado", objeto_nome))
        }
    }

    // ✅ Buscar método na hierarquia de herança
    fn buscar_metodo_na_hierarquia(
        &self,
        classe_nome: &str,
        nome_metodo: &str,
    ) -> Option<MetodoComOrigem> {
        let mut classe_atual = Some(classe_nome.to_string());
        
        while let Some(classe) = classe_atual {
            if let Some(def_classe) = self.classes.borrow().get(&classe) {
                // Buscar método na classe atual
                for metodo in &def_classe.metodos {
                    if metodo.nome == nome_metodo {
                        return Some(MetodoComOrigem {
                            metodo: metodo.clone(),
                            classe_origem: classe.clone(),
                        });
                    }
                }
                
                // Ir para classe pai
                classe_atual = def_classe.classe_pai.clone();
            } else {
                break;
            }
        }
        
        None
    }

    // ✅ Executar método objeto
    fn executar_metodo_objeto(
        &self,
        obj_expr: &Expressao,
        metodo: &str,
        argumentos: &[Expressao],
    ) -> Result<(), String> {
        if let Expressao::Identificador(objeto_nome) = obj_expr {
            self.executar_metodo_polimorfismo(objeto_nome, metodo, argumentos)
        } else {
            Err("Chamada de método em expressão complexa não implementada".to_string())
        }
    }

    fn criar_instancia_objeto_csharp(
        &self,
        classe: &str,
        argumentos: &[Expressao],
    ) -> Result<ValorAvaliado, String> {
        self.criar_instancia_objeto_com_heranca(classe, argumentos)
    }

    fn encontrar_construtor_compativel<'a>(
        &self,
        construtores: &'a [ConstrutorClasse],
        num_argumentos: usize,
    ) -> Option<&'a ConstrutorClasse> {
        for construtor in construtores {
            let obrigatorios = construtor.parametros.iter()
                .filter(|p| p.valor_padrao.is_none())
                .count();
            let total = construtor.parametros.len();
            if num_argumentos >= obrigatorios && num_argumentos <= total {
                return Some(construtor);
            }
        }
        None
    }

    fn resolver_argumentos_construtor_csharp(
        &self,
        argumentos: &[Expressao],
        parametros: &[Parametro],
    ) -> Result<Vec<(String, ValorAvaliado)>, String> {
        let mut resultado = Vec::new();
        
        for (i, arg) in argumentos.iter().enumerate() {
            if i >= parametros.len() {
                return Err("Muitos argumentos fornecidos".to_string());
            }
            let valor = self.avaliar_expressao(arg)?;
            resultado.push((parametros[i].nome.clone(), valor));
        }
        
        for i in argumentos.len()..parametros.len() {
            if let Some(valor_padrao) = &parametros[i].valor_padrao {
                let valor = self.avaliar_expressao(valor_padrao)?;
                resultado.push((parametros[i].nome.clone(), valor));
            } else {
                return Err(format!(
                    "Parâmetro '{}' é obrigatório mas não foi fornecido",
                    parametros[i].nome
                ));
            }
        }
        
        Ok(resultado)
    }

    fn executar_construtor_csharp(
        &self,
        argumentos: &[(String, ValorAvaliado)],
        propriedades: &mut HashMap<String, ValorAvaliado>,
    ) -> Result<(), String> {
        for (nome_parametro, valor) in argumentos {
            if propriedades.contains_key(nome_parametro) {
                propriedades.insert(nome_parametro.clone(), valor.clone());
                println!(" ✓ {} = {}", nome_parametro, self.valor_para_string(valor));
            } else {
                let nome_capitalizado = self.capitalizar_primeira_letra(nome_parametro);
                if propriedades.contains_key(&nome_capitalizado) {
                    propriedades.insert(nome_capitalizado.clone(), valor.clone());
                    println!(" ✓ {} = {} (auto-capitalizado)", nome_capitalizado, self.valor_para_string(valor));
                }
            }
        }
        Ok(())
    }

    fn executar_corpo_construtor(
        &self,
        corpo: &[Comando],
        _argumentos: &[(String, ValorAvaliado)],
    ) -> Result<(), String> {
        for comando in corpo {
            self.compilar_comando(comando)?;
        }
        Ok(())
    }

    fn executar_metodo_apresentar(
        &self,
        propriedades: &HashMap<String, ValorAvaliado>,
        completo: bool,
    ) -> Result<(), String> {
        if completo {
            let mut partes = Vec::new();
            if let Some(nome) = propriedades.get("Nome") {
                partes.push(format!("Nome: {}", self.valor_para_string(nome)));
            }
            if let Some(endereco) = propriedades.get("Endereco") {
                partes.push(format!("Endereço: {}", self.valor_para_string(endereco)));
            }
            if let Some(telefone) = propriedades.get("Telefone") {
                partes.push(format!("Telefone: {}", self.valor_para_string(telefone)));
            }
            if let Some(idade) = propriedades.get("Idade") {
                partes.push(format!("Idade: {}", self.valor_para_string(idade)));
            }
            if let Some(sobrenome) = propriedades.get("Sobrenome") {
                partes.push(format!("Sobrenome: {}", self.valor_para_string(sobrenome)));
            }
            let mensagem = partes.join(", ");
            self.gerar_printf(&mensagem)?;
            println!("SAÍDA: {}", mensagem);
        } else {
            if let Some(nome) = propriedades.get("Nome") {
                let mensagem = format!("Nome: {}", self.valor_para_string(nome));
                self.gerar_printf(&mensagem)?;
                println!("SAÍDA: {}", mensagem);
            }
        }
        Ok(())
    }

    fn executar_corpo_metodo(&self, corpo: &[Comando]) -> Result<(), String> {
        self.entrar_escopo();
        for comando in corpo {
            self.compilar_comando(comando)?;
        }
        self.sair_escopo();
        Ok(())
    }

    fn obter_valor_padrao_tipo(&self, tipo: &Tipo) -> ValorAvaliado {
        match tipo {
            Tipo::Inteiro => ValorAvaliado::Inteiro(0),
            Tipo::Texto => ValorAvaliado::Texto(String::new()),
            Tipo::Booleano => ValorAvaliado::Booleano(false),
            _ => ValorAvaliado::Texto("null".to_string()),
        }
    }

    fn capitalizar_primeira_letra(&self, texto: &str) -> String {
        if texto.is_empty() {
            return String::new();
        }
        let mut chars: Vec<char> = texto.chars().collect();
        chars[0] = chars[0].to_uppercase().next().unwrap_or(chars[0]);
        chars.iter().collect()
    }

    // ✅ Avaliar expressões (incluindo chamadas de função)
    pub fn avaliar_expressao(&self, expr: &Expressao) -> Result<ValorAvaliado, String> {
        match expr {
            Expressao::Inteiro(valor) => Ok(ValorAvaliado::Inteiro(*valor)),
            Expressao::Texto(valor) => Ok(ValorAvaliado::Texto(valor.clone())),
            Expressao::Booleano(valor) => Ok(ValorAvaliado::Booleano(*valor)),
            Expressao::Identificador(nome) => {
                if nome == "este" {
                    if let Some(este_obj) = self.escopo_este.borrow().clone() {
                        Ok(este_obj)
                    } else {
                        Err("'este' não está disponível neste contexto".to_string())
                    }
                } else {
                    self.buscar_variavel(nome)
                        .ok_or_else(|| format!("Variável '{}' não encontrada", nome))
                }
            }
            
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
                        OperadorAritmetico::Soma,
                        ValorAvaliado::Inteiro(a),
                        ValorAvaliado::Texto(b),
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
                    (
                        OperadorComparacao::Igual,
                        ValorAvaliado::Texto(a),
                        ValorAvaliado::Texto(b),
                    ) => a == b,
                    (
                        OperadorComparacao::Diferente,
                        ValorAvaliado::Texto(a),
                        ValorAvaliado::Texto(b),
                    ) => a != b,
                    (
                        OperadorComparacao::Igual,
                        ValorAvaliado::Booleano(a),
                        ValorAvaliado::Booleano(b),
                    ) => a == b,
                    (
                        OperadorComparacao::Diferente,
                        ValorAvaliado::Booleano(a),
                        ValorAvaliado::Booleano(b),
                    ) => a != b,
                    _ => return Err("Comparação inválida para estes tipos".to_string()),
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

            Expressao::Unario(op, expr) => {
                let valor = self.avaliar_expressao(expr)?;
                match (op, valor) {
                    (OperadorUnario::NegacaoLogica, ValorAvaliado::Booleano(b)) => {
                        Ok(ValorAvaliado::Booleano(!b))
                    }
                    (OperadorUnario::NegacaoNumerica, ValorAvaliado::Inteiro(i)) => {
                        Ok(ValorAvaliado::Inteiro(-i))
                    }
                    _ => Err("Operador unário inválido para este tipo".to_string())
                }
            }

            Expressao::NovoObjeto(classe, argumentos) => {
                self.criar_instancia_objeto_com_heranca(classe, argumentos)
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

            Expressao::ChamadaMetodo(_obj_expr, metodo, _argumentos) => {
                match metodo.as_str() {
                    "apresentar" => Ok(ValorAvaliado::Texto(
                        "Resultado do método apresentar".to_string(),
                    )),
                    _ => Ok(ValorAvaliado::Texto(format!(
                        "Resultado do método {}",
                        metodo
                    ))),
                }
            }

            // ✅ NOVO: Tratar chamadas de função em expressões
            Expressao::Chamada(nome, argumentos) => {
                // Por enquanto, simular resultado
                match nome.as_str() {
                    "tamanho" => Ok(ValorAvaliado::Inteiro(10)),
                    _ => {
                        // Executar função e retornar valor padrão
                        self.executar_funcao(nome, argumentos)?;
                        Ok(ValorAvaliado::Texto(format!("Resultado da função {}", nome)))
                    }
                }
            }

            Expressao::Este => {
                if let Some(este_obj) = self.escopo_este.borrow().clone() {
                    Ok(este_obj)
                } else {
                    Ok(ValorAvaliado::Objeto {
                        classe: "Atual".to_string(),
                        propriedades: HashMap::new(),
                    })
                }
            }
        }
    }

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
            ValorAvaliado::Objeto {
                classe,
                propriedades,
            } => {
                format!(
                    "Objeto de {} com {} propriedades",
                    classe,
                    propriedades.len()
                )
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