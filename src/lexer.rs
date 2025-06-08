use logos::Logos;

#[derive(Logos, Debug, PartialEq, Clone)]
pub enum Token {
    // Palavras-chave
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
    #[token("%")]  // Adicionado operador módulo
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
    #[token("->")]
    TSeta,
    
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