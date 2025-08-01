use serde::{Deserialize, Serialize};

/* ========================================================================== */
/* TIPOS BÁSICOS                                                              */
/* ========================================================================== */
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Tipo {
    Booleano,
    Texto,
    Inteiro,
    Decimal,
    Vazio,
    Lista(Box<Tipo>),
    Classe(String), // Stores FQN
    Funcao(Vec<Tipo>, Box<Tipo>),
    Generico(String),
    Opcional(Box<Tipo>),
    Inferido,
}

/* ========================================================================== */
/* CAMINHOS                                                                   */
/* ========================================================================== */
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Caminho {
    pub partes: Vec<String>,
}

impl ToString for Caminho {
    fn to_string(&self) -> String {
        self.partes.join(".")
    }
}

/* ========================================================================== */
/* PROGRAMA                                                                   */
/* ========================================================================== */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Programa {
    pub usings: Vec<DeclaracaoUsando>,
    pub namespaces: Vec<DeclaracaoNamespace>,
    pub declaracoes: Vec<Declaracao>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ItemPrograma {
    Usando(DeclaracaoUsando),
    Namespace(DeclaracaoNamespace),
    Declaracao(Declaracao),
}

/* ========================================================================== */
/* NAMESPACES                                                                 */
/* ========================================================================== */
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeclaracaoNamespace {
    pub nome: String,
    pub declaracoes: Vec<Declaracao>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeclaracaoUsando {
    pub caminho: String,
}

/* ========================================================================== */
/* DECLARAÇÕES TOP-LEVEL                                                      */
/* ========================================================================== */
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Declaracao {
    DeclaracaoClasse(DeclaracaoClasse),
    DeclaracaoFuncao(DeclaracaoFuncao),
    DeclaracaoModulo(DeclaracaoModulo),
    DeclaracaoInterface(DeclaracaoInterface),
    DeclaracaoEnum(DeclaracaoEnum),
    DeclaracaoTipo(DeclaracaoTipo),
    Importacao(Importacao),
    Exportacao(Exportacao),
    Comando(Comando),
    DeclaracaoNamespace(DeclaracaoNamespace), // ✅ ADICIONE ESTA VARIANTE
}

/* — módulos / interfaces / enums / type-alias — */
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeclaracaoModulo {
    pub nome: String,
    pub conteudo: Vec<Declaracao>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeclaracaoInterface {
    pub nome: String,
    pub metodos: Vec<AssinaturaMetodo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AssinaturaMetodo {
    pub nome: String,
    pub parametros: Vec<Parametro>,
    pub tipo_retorno: Option<Tipo>,
    pub modificador: ModificadorAcesso,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeclaracaoEnum {
    pub nome: String,
    pub valores: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeclaracaoTipo {
    pub nome: String,
    pub tipo_base: Tipo,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Importacao {
    pub caminho: String,
    pub itens: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Exportacao {
    pub nome: String,
    pub publico: bool,
}

/* ========================================================================== */
/* CLASSES                                                                    */
/* ========================================================================== */
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeclaracaoClasse {
    pub nome: String,
    pub classe_pai: Option<String>,
    pub modificador: ModificadorAcesso,
    pub campos: Vec<CampoClasse>,
    pub propriedades: Vec<PropriedadeClasse>,
    pub metodos: Vec<MetodoClasse>,
    pub construtores: Vec<ConstrutorClasse>,
    pub eh_abstrata: bool,
    pub eh_estatica: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MembroClasse {
    Campo(CampoClasse),
    Propriedade(PropriedadeClasse),
    Metodo(MetodoClasse),
    Construtor(ConstrutorClasse),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CampoClasse {
    pub nome: String,
    pub tipo: Tipo,
    pub modificador: ModificadorAcesso,
    pub valor_inicial: Option<Expressao>,
    pub eh_estatica: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PropriedadeClasse {
    pub nome: String,
    pub tipo: Tipo,
    pub modificador: ModificadorAcesso,
    pub obter: Option<AcessorPropriedade>,
    pub definir: Option<AcessorPropriedade>,
    pub valor_inicial: Option<Expressao>,
    pub eh_estatica: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AcessorPropriedade {
    pub modificador: Option<ModificadorAcesso>,
    pub corpo: Option<Vec<Comando>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MetodoClasse {
    pub nome: String,
    pub parametros: Vec<Parametro>,
    pub tipo_retorno: Option<Tipo>,
    pub modificador: ModificadorAcesso,
    pub corpo: Vec<Comando>,
    pub eh_virtual: bool,
    pub eh_override: bool,
    pub eh_abstrato: bool,
    pub eh_estatica: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConstrutorClasse {
    pub parametros: Vec<Parametro>,
    pub modificador: ModificadorAcesso,
    pub corpo: Vec<Comando>,
    pub chamada_pai: Option<Vec<Expressao>>,
    pub nome_escrito: Option<String>, // para construtor “Classe(...)”
}

/* ========================================================================== */
/* FUNÇÕES                                                                    */
/* ========================================================================== */
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeclaracaoFuncao {
    pub nome: String,
    pub parametros: Vec<Parametro>,
    pub tipo_retorno: Option<Tipo>,
    pub modificador: ModificadorAcesso,
    pub corpo: Vec<Comando>,
    pub eh_estatica: bool,
}

/* — parâmetros com valor padrão (C#-style) — */
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Parametro {
    pub nome: String,
    pub tipo: Tipo,
    pub valor_padrao: Option<Expressao>,
}

impl Parametro {
    pub fn obrigatorio(nome: String, tipo: Tipo) -> Self {
        Self {
            nome,
            tipo,
            valor_padrao: None,
        }
    }
    pub fn opcional(nome: String, tipo: Tipo, valor_padrao: Expressao) -> Self {
        Self {
            nome,
            tipo,
            valor_padrao: Some(valor_padrao),
        }
    }
    pub fn eh_opcional(&self) -> bool {
        self.valor_padrao.is_some()
    }
}

/* ========================================================================== */
/* MODIFICADORES                                                              */
/* ========================================================================== */
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ModificadorAcesso {
    Publico,
    Privado,
    Protegido,
}

/* ========================================================================== */
/* COMANDOS                                                                   */
/* ========================================================================== */
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Comando {
    DeclaracaoVariavel(Tipo, String, Option<Expressao>),
    DeclaracaoVar(String, Expressao),
    Atribuicao(String, Expressao),
    AtribuirPropriedade(Box<Expressao>, String, Expressao),
    AtribuirCampo(Box<Expressao>, String, Expressao),
    Imprima(Expressao),
    Se(Expressao, Box<Comando>, Option<Box<Comando>>),
    Enquanto(Expressao, Box<Comando>),
    Para(
        Option<Box<Comando>>,
        Option<Expressao>,
        Option<Box<Comando>>,
        Box<Comando>,
    ),
    Bloco(Vec<Comando>),
    Retorne(Option<Expressao>),
    Expressao(Expressao),
    CriarObjeto(String, String, Vec<Expressao>),
    ChamarMetodo(Box<Expressao>, String, Vec<Expressao>),
    AcessarCampo(String, String),
}

/* ========================================================================== */
/* EXPRESSÕES                                                                 */
/* ========================================================================== */
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Expressao {
    Inteiro(i64),
    Texto(String),
    Booleano(bool),
    Decimal(String),
    Identificador(String),
    Aritmetica(OperadorAritmetico, Box<Expressao>, Box<Expressao>),
    Comparacao(OperadorComparacao, Box<Expressao>, Box<Expressao>),
    Logica(OperadorLogico, Box<Expressao>, Box<Expressao>),
    NovoObjeto(String, Vec<Expressao>),
    AcessoMembro(Box<Expressao>, String),
    ChamadaMetodo(Box<Expressao>, String, Vec<Expressao>),
    Chamada(String, Vec<Expressao>),
    StringInterpolada(Vec<PartStringInterpolada>),
    Unario(OperadorUnario, Box<Expressao>),
    Este,
    }

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OperadorUnario {
    NegacaoLogica,
    NegacaoNumerica,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PartStringInterpolada {
    Texto(String),
    Expressao(Expressao),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OperadorAritmetico {
    Soma,
    Subtracao,
    Multiplicacao,
    Divisao,
    Modulo,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OperadorComparacao {
    Igual,
    Diferente,
    Menor,
    MaiorQue,
    MenorIgual,
    MaiorIgual,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OperadorLogico {
    E,
    Ou,
}