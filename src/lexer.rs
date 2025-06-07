use logos::Logos;

#[derive(Logos, Debug, PartialEq, Clone)]
pub enum Token {
    #[token("se")]
    TSe,

    #[token("então")]
    TEntao,

    #[token("imprima")]
    TImprima,

    #[token("(")]
    TParenEsq,

    #[token(")")]
    TParenDir,

    #[token("{")]
    TChaveEsq,

    #[token("}")]
    TChaveDir,

    #[token(";")]
    TPontoVirgula,

    #[regex(r#""[^"]*""#, |lex| lex.slice().trim_matches('"').to_string())]
    TString(String),

    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice().to_string())]
    TIdentificador(String),

    #[regex(r"[0-9]+", |lex| lex.slice().parse().ok())]
    TInteiro(i64),

    #[token(">")]
    TMaiorQue,

    #[regex(r"[\s\t\n]+", logos::skip)]
    Whitespace,
}