use logos::Logos;

#[derive(Logos, Debug, PartialEq, Clone)]
pub enum Token {
    /* palavras-chave básicas */
    #[token("se")]        TSe,
    #[token("então")]     TEntao,
    #[token("senão")]     TSenao,
    #[token("enquanto")]  TEnquanto,
    #[token("para")]      TPara,
    #[token("funcao")]    TFuncao,
    #[token("retorne")]   TRetorne,
    #[token("imprima")]   TImprima,
    #[token("var")]       TVar,
    #[token("espaco")]    TEspaco,

    /* tipos */
    #[token("inteiro")]   TTipoInteiro,
    #[token("texto")]     TTipoTexto,
    #[token("booleano")]  TTipoBooleano,
    #[token("vazio")]     TTipoVazio,
    #[token("verdadeiro")]TVerdadeiro,
    #[token("falso")]     TFalso,

    /* OOP */
    #[token("classe")]    TClasse,
    #[token("construtor")]TConstrutor,
    #[token("publico")]   TPublico, 
    #[token("privado")]   TPrivado,
    #[token("protegido")] TProtegido,
    #[token("base")]      TBase,
    #[token("redefinível")] TRedefinivel,
    #[token("sobrescreve")] TSobrescreve,
    #[token("abstrato")] TAbstrato,
    #[token("novo")]      TNovo,
    #[token("este")]      TEste,
    #[token("buscar")]    TBuscar,
    #[token("definir")]   TDefinir,

    /* operadores */
    #[token("==")] TIgual,
    #[token("!=")] TDiferente,
    #[token(">=")] TMaiorIgual,
    #[token("<=")] TMenorIgual,
    #[token(">")]  TMaiorQue,
    #[token("<")]  TMenor,
    #[token("+")]  TMais,
    #[token("-")]  TMenos,
    #[token("*")]  TMultiplicacao,
    #[token("/")]  TDivisao,
    #[token("%")]  TModulo,
    #[token("&&")] TE,
    #[token("||")] TOu,
    #[token("!")]  TNao,
    #[token("=")]  TAtribuicao,

    /* delimitadores */
    #[token("(")] TParenEsq,
    #[token(")")] TParenDir,
    #[token("{")] TChaveEsq,
    #[token("}")] TChaveDir,
    #[token(";")] TPontoVirgula,
    #[token(",")] TVirgula,
    #[token(".")] TPonto,
    #[token(":")] TDoisPontos,
    #[token("=>")]TSeta,

    /* literais */
    #[regex(r#"\$\"([^"\\]|\\.)*\""#, |lex| {
    // slice = $" … "
    let s = lex.slice();
    s[2..s.len() - 1].to_string()          // devolve só o miolo
    })]
    TStringInterpolada(String),
    #[regex(r#""([^"\\]|\\.)*""#, |lex| lex.slice()[1..lex.slice().len()-1].to_string())]
    TString(String),
    #[regex(r"[0-9]+", |lex| lex.slice().parse::<i64>().unwrap())]
    TInteiro(i64),
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice().to_string())]
    TIdentificador(String),

    /* comentários / espaço */
    #[regex(r"//[^\n]*", logos::skip)]
    ComentarioLinha,
    #[regex(r"[ \t\r\n]+", logos::skip)]
    Whitespace,
}