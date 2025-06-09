use std::collections::HashMap;

// === TIPOS BÁSICOS ===
#[derive(Debug, Clone, PartialEq)]
pub enum Tipo {
    Inteiro,
    Texto,
    Booleano,
    Vazio,
    Lista(Box<Tipo>),
    Classe(String),
    Funcao(Vec<Tipo>, Box<Tipo>),
    Generico(String),
    Opcional(Box<Tipo>),
    Inferido,
}

// === PROGRAMA ===
#[derive(Debug, Clone)]
pub struct Programa {
    pub namespaces: Vec<DeclaracaoNamespace>,
    pub declaracoes: Vec<Declaracao>,
}

#[derive(Debug, Clone)]
pub enum ItemPrograma {
    Namespace(DeclaracaoNamespace),
    Declaracao(Declaracao),
}

// === NAMESPACES ===
#[derive(Debug, Clone)]
pub struct DeclaracaoNamespace {
    pub nome: String,
    pub declaracoes: Vec<Declaracao>,
}

// === DECLARAÇÕES ===
#[derive(Debug, Clone)]
pub enum Declaracao {
    DeclaracaoClasse(DeclaracaoClasse),
    DeclaracaoFuncao(DeclaracaoFuncao),
    DeclaracaoModulo(DeclaracaoModulo),
    DeclaracaoInterface(DeclaracaoInterface),
    DeclaracaoEnum(DeclaracaoEnum),
    DeclaracaoTipo(DeclaracaoTipo),
    ImportDeclaration(ImportDeclaration),
    Comando(Comando),
}

// === ESTRUTURAS ADICIONAIS ===
#[derive(Debug, Clone)]
pub struct DeclaracaoModulo {
    pub nome: String,
    pub conteudo: Vec<Declaracao>,
}

#[derive(Debug, Clone)]
pub struct DeclaracaoInterface {
    pub nome: String,
    pub metodos: Vec<AssinaturaMetodo>,
}

#[derive(Debug, Clone)]
pub struct AssinaturaMetodo {
    pub nome: String,
    pub parametros: Vec<Parametro>,
    pub tipo_retorno: Option<Tipo>,
    pub modificador: ModificadorAcesso,
}

#[derive(Debug, Clone)]
pub struct DeclaracaoEnum {
    pub nome: String,
    pub valores: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct DeclaracaoTipo {
    pub nome: String,
    pub tipo_base: Tipo,
}

#[derive(Debug, Clone)]
pub struct ImportDeclaration {
    pub caminho: String,
    pub itens: Vec<String>,
}

// === CLASSES ===
#[derive(Debug, Clone)]
pub struct DeclaracaoClasse {
    pub nome: String,
    pub classe_pai: Option<String>,
    pub modificador: ModificadorAcesso,
    pub campos: Vec<CampoClasse>,
    pub propriedades: Vec<PropriedadeClasse>,
    pub metodos: Vec<MetodoClasse>,
    pub construtores: Vec<Construtor>,
    pub eh_abstrata: bool,
}

#[derive(Debug, Clone)]
pub enum MembroClasse {
    Campo(CampoClasse),
    Propriedade(PropriedadeClasse),
    Metodo(MetodoClasse),
    Construtor(Construtor),
}

#[derive(Debug, Clone)]
pub struct CampoClasse {
    pub nome: String,
    pub tipo: Tipo,
    pub modificador: ModificadorAcesso,
    pub valor_inicial: Option<Expressao>,
    pub eh_estatico: bool,
}

#[derive(Debug, Clone)]
pub struct PropriedadeClasse {
    pub nome: String,
    pub tipo: Tipo,
    pub modificador: ModificadorAcesso,
    pub buscar: Option<AcessorPropriedade>,
    pub definir: Option<AcessorPropriedade>,
    pub valor_inicial: Option<Expressao>,
}

#[derive(Debug, Clone)]
pub struct AcessorPropriedade {
    pub modificador: Option<ModificadorAcesso>,
    pub corpo: Option<Vec<Comando>>,
}

#[derive(Debug, Clone)]
pub struct MetodoClasse {
    pub nome: String,
    pub parametros: Vec<Parametro>,
    pub tipo_retorno: Option<Tipo>,
    pub modificador: ModificadorAcesso,
    pub corpo: Vec<Comando>,
    pub eh_virtual: bool,
    pub eh_override: bool,
    pub eh_abstrato: bool,
    pub eh_estatico: bool,
}

#[derive(Debug, Clone)]
pub struct Construtor {
    pub parametros: Vec<Parametro>,
    pub modificador: ModificadorAcesso,
    pub corpo: Vec<Comando>,
}

// === FUNÇÕES ===
#[derive(Debug, Clone)]
pub struct DeclaracaoFuncao {
    pub nome: String,
    pub parametros: Vec<Parametro>,
    pub tipo_retorno: Option<Tipo>,
    pub modificador: ModificadorAcesso,
    pub corpo: Vec<Comando>,
}

#[derive(Debug, Clone)]
pub struct Parametro {
    pub nome: String,
    pub tipo: Tipo,
    pub valor_padrao: Option<Expressao>,
}

// === MODIFICADORES DE ACESSO ===
#[derive(Debug, Clone, PartialEq)]
pub enum ModificadorAcesso {
    Publico,
    Privado,
    Protegido,
}

// === COMANDOS ===
#[derive(Debug, Clone)]
pub enum Comando {
    DeclaracaoVariavel(Tipo, String, Option<Expressao>),
    DeclaracaoVar(String, Expressao),
    Atribuicao(String, Expressao),
    AtribuirPropriedade(String, String, Expressao),
    AtribuirCampo(Box<Expressao>, String, Expressao),
    Imprima(Expressao),
    Se(Expressao, Box<Comando>, Option<Box<Comando>>),
    Enquanto(Expressao, Box<Comando>),
    Para(Option<Box<Comando>>, Option<Expressao>, Option<Box<Comando>>, Box<Comando>),
    Bloco(Vec<Comando>),
    Retorne(Option<Expressao>),
    Expressao(Expressao),
    CriarObjeto(String, String, Vec<Expressao>),
    ChamarMetodo(String, String, Vec<Expressao>),
    AcessarCampo(String, String),
}

// === EXPRESSÕES ===
#[derive(Debug, Clone)]
pub enum Expressao {
    Inteiro(i64),
    Texto(String),
    Booleano(bool),
    Identificador(String),
    Aritmetica(OperadorAritmetico, Box<Expressao>, Box<Expressao>),
    Comparacao(OperadorComparacao, Box<Expressao>, Box<Expressao>),
    Logica(OperadorLogico, Box<Expressao>, Box<Expressao>),
    NovoObjeto(String, Vec<Expressao>),
    AcessoMembro(Box<Expressao>, String),
    ChamadaMetodo(Box<Expressao>, String, Vec<Expressao>),
    Chamada(String, Vec<Expressao>),
    StringInterpolada(Vec<PartStringInterpolada>),
    Este,
}

#[derive(Debug, Clone)]
pub enum PartStringInterpolada {
    Texto(String),
    Expressao(Expressao),
}

// === OPERADORES ===
#[derive(Debug, Clone)]
pub enum OperadorAritmetico {
    Soma,
    Subtracao,
    Multiplicacao,
    Divisao,
    Modulo,
}

#[derive(Debug, Clone)]
pub enum OperadorComparacao {
    Igual,
    Diferente,
    Menor,
    MaiorQue,
    MenorIgual,
    MaiorIgual,
}

#[derive(Debug, Clone)]
pub enum OperadorLogico {
    E,
    Ou,
}