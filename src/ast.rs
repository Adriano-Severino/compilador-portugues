#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Programa {
    pub namespaces: Vec<DeclaracaoNamespace>,
    pub declaracoes: Vec<Declaracao>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct DeclaracaoNamespace {
    pub nome: String,
    pub declaracoes: Vec<Declaracao>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum Declaracao {
    Comando(Comando),
    DeclaracaoClasse(DeclaracaoClasse),
    DeclaracaoFuncao(DeclaracaoFuncao),
    DeclaracaoMetodo(DeclaracaoMetodo),
    DeclaracaoModulo(DeclaracaoModulo),
    Importacao(Importacao),
    Exportacao(Exportacao),
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum Comando {
    Se(Expressao, Box<Comando>, Option<Box<Comando>>),
    Enquanto(Expressao, Box<Comando>),
    Para(String, Expressao, Expressao, Box<Comando>),
    Imprima(Expressao),
    Bloco(Vec<Comando>),
    DeclaracaoVariavel(Tipo, String, Option<Expressao>),
    DeclaracaoVar(String, Expressao),
    Atribuicao(String, Expressao),
    AtribuirPropriedade(String, String, Expressao),
    Retorne(Option<Expressao>),
    Expressao(Expressao),
    CriarObjeto(String, String, Vec<Expressao>),
    ChamarMetodo(Expressao, String, Vec<Expressao>),
    AcessarCampo(Expressao, String),
    AtribuirCampo(Expressao, String, Expressao),
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum Tipo {
    Inteiro,
    Texto,
    Booleano,
    Vazio,
    Classe(String),
    Lista(Box<Tipo>),
    Funcao(Vec<Tipo>, Box<Tipo>),
    Generico(String),
    Opcional(Box<Tipo>),
    Inferido,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum Expressao {
    Inteiro(i64),
    Texto(String),
    StringInterpolada(Vec<PartStringInterpolada>),
    Booleano(bool),
    Identificador(String),
    Chamada(String, Vec<Expressao>),
    Comparacao(OperadorComparacao, Box<Expressao>, Box<Expressao>),
    Aritmetica(OperadorAritmetico, Box<Expressao>, Box<Expressao>),
    Logica(OperadorLogico, Box<Expressao>, Box<Expressao>),
    Unario(OperadorUnario, Box<Expressao>),
    AcessoMembro(Box<Expressao>, String),
    ChamadaMetodo(Box<Expressao>, String, Vec<Expressao>),
    NovoObjeto(String, Vec<Expressao>),
    Este,
    Super,
    Nulo,
}

// NOVO: Para string interpolation $"texto {variavel}"
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum PartStringInterpolada {
    Texto(String),
    Expressao(Expressao),
}

// NOVO: Estrutura para métodos independentes
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct DeclaracaoMetodo {
    pub nome: String,
    pub parametros: Vec<Parametro>,
    pub tipo_retorno: Option<Tipo>,
    pub modificador: ModificadorAcesso,
    pub eh_estatico: bool,
    pub corpo: Vec<Comando>,
}

// Estruturas para OOP com propriedades
#[allow(dead_code)]
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

// Adicionar estas estruturas se não existirem
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

// NOVO: Propriedades com get/set traduzidos
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct PropriedadeClasse {
    pub nome: String,
    pub tipo: Tipo,
    pub modificador: ModificadorAcesso,
    pub buscar: Option<AcessorPropriedade>, // get
    pub definir: Option<AcessorPropriedade>, // set
    pub valor_inicial: Option<Expressao>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct AcessorPropriedade {
    pub modificador: Option<ModificadorAcesso>,
    pub corpo: Option<Vec<Comando>>, // None para auto-implementado
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct CampoClasse {
    pub nome: String,
    pub tipo: Tipo,
    pub modificador: ModificadorAcesso,
    pub valor_inicial: Option<Expressao>,
    pub eh_estatico: bool,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct MetodoClasse {
    pub nome: String,
    pub parametros: Vec<Parametro>,
    pub tipo_retorno: Option<Tipo>,
    pub modificador: ModificadorAcesso,
    pub eh_virtual: bool,
    pub eh_override: bool,
    pub eh_abstrato: bool,
    pub eh_estatico: bool,
    pub corpo: Vec<Comando>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Construtor {
    pub parametros: Vec<Parametro>,
    pub modificador: ModificadorAcesso,
    pub corpo: Vec<Comando>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct DeclaracaoFuncao {
    pub nome: String,
    pub parametros: Vec<Parametro>,
    pub tipo_retorno: Option<Tipo>,
    pub modificador: ModificadorAcesso,
    pub corpo: Vec<Comando>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Parametro {
    pub nome: String,
    pub tipo: Tipo,
    pub valor_padrao: Option<Expressao>,
}

// Estruturas para módulos
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct DeclaracaoModulo {
    pub nome: String,
    pub declaracoes: Vec<Declaracao>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Importacao {
    pub caminho: String,
    pub items: Vec<String>,
    pub alias: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Exportacao {
    pub nome: String,
    pub publico: bool,
}

// Enums auxiliares
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum ModificadorAcesso {
    Publico,
    Privado,
    Protegido,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum ItemPrograma {
    Namespace(DeclaracaoNamespace),
    Declaracao(Declaracao),
}


#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum MembroClasse {
    Campo(CampoClasse),
    Propriedade(PropriedadeClasse),
    Metodo(MetodoClasse),
    Construtor(Construtor),
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum OperadorComparacao {
    Igual,
    Diferente,
    MaiorQue,
    MaiorIgual,
    Menor,
    MenorIgual,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum OperadorAritmetico {
    Soma,
    Subtracao,
    Multiplicacao,
    Divisao,
    Modulo,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum OperadorLogico {
    E,
    Ou,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum OperadorUnario {
    Nao,
    Menos,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub enum StatusOwnership {
    Dono,
    Emprestado,
    EmprestadoMutavel,
    Movido,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct InfoVariavel {
    pub tipo: Tipo,
    pub mutavel: bool,
    pub inicializada: bool,
    pub escopo: usize,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct InfoFuncao {
    pub nome: String,
    pub parametros: Vec<Tipo>,
    pub tipo_retorno: Option<Tipo>,
    pub eh_nativa: bool,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct InfoClasse {
    pub nome: String,
    pub campos: Vec<CampoClasse>,
    pub metodos: Vec<MetodoClasse>,
    pub classe_pai: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum TipoLoop {
    Enquanto,
    Para,
    Fazer,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct InfoLoop {
    pub tipo: TipoLoop,
    pub nivel: usize,
    pub tem_break: bool,
    pub tem_continue: bool,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum TipoErro {
    Compilacao,
    Execucao,
    TipoIncompativel,
    VariavelNaoDeclarada,
    FuncaoNaoEncontrada,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ErroCompilacao {
    pub tipo: TipoErro,
    pub mensagem: String,
    pub linha: usize,
    pub coluna: usize,
    pub arquivo: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct MetricasComplexidade {
    pub linhas_codigo: usize,
    pub funcoes: usize,
    pub classes: usize,
    pub complexidade_ciclomatica: usize,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct InfoEscopo {
    pub nivel: usize,
    pub variaveis: Vec<String>,
    pub funcoes: Vec<String>,
    pub eh_funcao: bool,
    pub eh_classe: bool,
    pub eh_loop: bool,
}