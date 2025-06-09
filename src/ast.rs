#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Programa {
    pub declaracoes: Vec<Declaracao>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum Declaracao {
    Comando(Comando),
    DeclaracaoClasse(DeclaracaoClasse),
    DeclaracaoFuncao(DeclaracaoFuncao),
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
    Atribuicao(String, Expressao),
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
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum Expressao {
    Inteiro(i64),
    Texto(String),
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

// Estruturas para OOP
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct DeclaracaoClasse {
    pub nome: String,
    pub classe_pai: Option<String>,
    pub modificador: ModificadorAcesso,
    pub campos: Vec<CampoClasse>,
    pub metodos: Vec<MetodoClasse>,
    pub construtores: Vec<Construtor>,
    pub eh_abstrata: bool,
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

// Enums para análise de ownership
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub enum StatusOwnership {
    Dono,
    Emprestado,
    EmprestadoMutavel,
    Movido,
}

// Estruturas auxiliares para análise semântica
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

// Estruturas para controle de fluxo avançado
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum TipoLoop {
    Enquanto,
    Para,
    Fazer, // Para futuro suporte a do-while
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct InfoLoop {
    pub tipo: TipoLoop,
    pub nivel: usize,
    pub tem_break: bool,
    pub tem_continue: bool,
}

// Estruturas para tratamento de erros (futuro)
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

// Estruturas para análise estática avançada
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