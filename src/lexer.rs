use logos::Logos;

#[derive(Logos, Debug, PartialEq, Clone)]
pub enum Token {
    // Palavras-chave básicas
    #[token("se")]
    TSe,
    #[token("então")]
    TEntao,
    #[token("senão")]
    TSenao,
    #[token("enquanto")]
    TEnquanto,
    #[token("para")]
    TPara,
    #[token("funcao")]
    TFuncao,
    #[token("retorne")]
    TRetorne,
    #[token("imprima")]
    TImprima,
    
    // Tipos
    #[token("inteiro")]
    TTipoInteiro,
    #[token("texto")]
    TTipoTexto,
    #[token("booleano")]
    TTipoBooleano,
    #[token("vazio")]
    TTipoVazio,
    #[token("Lista")]
    TTipoLista,
    #[token("verdadeiro")]
    TVerdadeiro,
    #[token("falso")]
    TFalso,
    
    // Operadores
    #[token("=")]
    TAtribuicao,
    #[token("==")]
    TIgual,
    #[token("!=")]
    TDiferente,
    #[token(">")]
    TMaiorQue,
    #[token(">=")]
    TMaiorIgual,
    #[token("<")]
    TMenor,
    #[token("<=")]
    TMenorIgual,
    #[token("+")]
    TMais,
    #[token("-")]
    TMenos,
    #[token("*")]
    TMultiplicacao,
    #[token("/")]
    TDivisao,
    #[token("%")]
    TModulo,
    #[token("&&")]
    TE,
    #[token("||")]
    TOu,
    #[token("!")]
    TNao,
    
    // Delimitadores
    #[token("(")]
    TParenEsq,
    #[token(")")]
    TParenDir,
    #[token("{")]
    TChaveEsq,
    #[token("}")]
    TChaveDir,
    #[token("[")]
    TColcheteEsq,
    #[token("]")]
    TColcheteDir,
    #[token(";")]
    TPontoVirgula,
    #[token(",")]
    TVirgula,
    #[token("=>")]
    TSeta,
    #[token(".")]
    TPonto,
    #[token("..")]
    TDoisPontos,
    
    // Tokens para OOP
    #[token("classe")]
    TClasse,
    #[token("herda")]
    THerda,
    #[token("construtor")]
    TConstrutor,
    #[token("metodo")]
    TMetodo,
    #[token("publico")]
    TPublico,
    #[token("privado")]
    TPrivado,
    #[token("protegido")]
    TProtegido,
    #[token("virtual")]
    TVirtual,
    #[token("override")]
    TOverride,
    #[token("novo")]
    TNovo,
    #[token("este")]
    TEste,
    #[token("super")]
    TSuper,

    // Tokens para módulos
    #[token("modulo")]
    TModuloToken,
    #[token("importar")]
    TImportar,
    #[token("exportar")]
    TExportar,
    #[token("de")]
    TDe,
    #[token("como")]
    TComo,
    #[token("usar")]
    TUsar,
    
    // Literais
    #[regex(r#""[^"]*""#, |lex| lex.slice().trim_matches('"').to_string())]
    TString(String),
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice().to_string())]
    TIdentificador(String),
    #[regex(r"[0-9]+", |lex| lex.slice().parse().ok())]
    TInteiro(i64),
    
    // Comentários e espaços
    #[regex(r"//[^\n]*", logos::skip)]
    ComentarioLinha,
    #[regex(r"/\*[^*]*\*+(?:[^/*][^*]*\*+)*/", logos::skip)]
    ComentarioBloco,
    #[regex(r"[\s\t\n]+", logos::skip)]
    Whitespace,
}
