use logos::Logos;

#[derive(Logos, Debug, PartialEq)]
pub enum Token {
    #[token("se")]
    TSe,

    #[token("então")]
    TEntao,

    #[token("imprima")]
    TImprima,

    #[regex(r#""([^"\\]|\\t|\\u|\\n|\\")*""#)]
    TString(String),

    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice().to_string())]
    TIdentificador(String),

    #[regex(r"[0-9]+", |lex| lex.slice().parse())]
    TInteiro(i64),

    #[error]
    #[regex(r"[\s\t\n]+", logos::skip)]
    Erro,
}
