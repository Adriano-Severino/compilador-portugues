use logos::Logos;

#[derive(Logos, Debug, PartialEq, Clone)]
pub enum Token {
    /* palavras-chave básicas */
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
    #[token("função")]
    TFuncao,
    #[token("retorne")]
    TRetorne,
    #[token("imprima")]
    TImprima,
    #[token("var")]
    TVar,
    #[token("espaco")]
    TEspaco,
    #[token("usando")]
    TUsando,

    /* tipos */
    #[token("inteiro")]
    TTipoInteiro,
    #[token("texto")]
    TTipoTexto,
    #[token("booleano")]
    TTipoBooleano,
    #[token("flutuante")]
    TTipoFlutuante,
    #[token("duplo")]
    TTipoDuplo,
    #[token("decimal")]
    TTipoDecimal,
    #[token("vazio")]
    TTipoVazio,
    #[token("verdadeiro")]
    TVerdadeiro,
    #[token("falso")]
    TFalso,

    /* OOP */
    #[token("classe")]
    TClasse,
    #[token("enumeração")]
    TEnumeracao,
    #[token("construtor")]
    TConstrutor,
    #[token("publico")]
    TPublico,
    #[token("privado")]
    TPrivado,
    #[token("protegido")]
    TProtegido,
    #[token("base")]
    TBase,
    #[token("redefinível")]
    TRedefinivel,
    #[token("sobrescreve")]
    TSobrescreve,
    #[token("abstrata")]
    TAbstrato,
    #[token("novo")]
    TNovo,
    #[token("este")]
    TEste,
    #[token("obter")]
    TObter,
    #[token("definir")]
    TDefinir,
    #[token("estática")]
    TEstatica,

    /* operadores */
    #[token("==")]
    TIgual,
    #[token("!=")]
    TDiferente,
    #[token(">=")]
    TMaiorIgual,
    #[token("<=")]
    TMenorIgual,
    #[token(">")]
    TMaiorQue,
    #[token("<")]
    TMenor,
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
    #[token("=")]
    TAtribuicao,

    /* delimitadores */
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
    #[token(",")]
    TVirgula,
    #[token(".")]
    TPonto,
    #[token(":")]
    TDoisPontos,
    #[token("=>")]
    TSeta,

    /* literais */
    #[regex(r#"\$\"([^"\\]|\\.)*\""#, |lex| {
    // slice = $" … "
    let s = lex.slice();
    s[2..s.len() - 1].to_string()          // devolve só o miolo
    })]
    TStringInterpolada(String),
    #[regex(r#""([^"\\]|\\.)*""#, |lex| lex.slice()[1..lex.slice().len()-1].to_string())]
    TString(String),
    #[regex(r"[0-9]+\.[0-9]+[mM]", |lex| lex.slice().to_string())]
    TDecimal(String),
    #[regex(r"[0-9]+\.[0-9]+[fF]", |lex| lex.slice().to_string())]
    TFlutuanteLiteral(String),
    #[regex(r"[0-9]+\.[0-9]+", |lex| lex.slice().to_string())]
    TDuploLiteral(String),
    #[regex(r"[0-9]+", |lex| {
        let s = lex.slice();
        s.parse::<i64>().expect(&format!("Literal inteiro inválido: '{s}'"))
    })]
    TInteiro(i64),
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice().to_string())]
    TIdentificador(String),

    /* comentários / espaço */
    #[regex(r"//[^\n]*", logos::skip)]
    ComentarioLinha,
    #[regex(r"[ \t\r\n]+", logos::skip)]
    Whitespace,
}

#[cfg(test)]
mod tests {
    use super::*;
    use logos::Logos;

    #[test]
    fn test_palavras_chave() {
        let codigo = "se então senão enquanto para classe publico";
        let mut lex = Token::lexer(codigo);

        assert_eq!(lex.next(), Some(Ok(Token::TSe)));
        assert_eq!(lex.next(), Some(Ok(Token::TEntao)));
        assert_eq!(lex.next(), Some(Ok(Token::TSenao)));
        assert_eq!(lex.next(), Some(Ok(Token::TEnquanto)));
        assert_eq!(lex.next(), Some(Ok(Token::TPara)));
        assert_eq!(lex.next(), Some(Ok(Token::TClasse)));
        assert_eq!(lex.next(), Some(Ok(Token::TPublico)));
    }
    #[test]
    fn test_literais() {
        let codigo = r#"123 "hello" verdadeiro falso"#;
        let mut lex = Token::lexer(codigo);

        assert_eq!(lex.next(), Some(Ok(Token::TInteiro(123))));
        assert_eq!(lex.next(), Some(Ok(Token::TString("hello".to_string()))));
        assert_eq!(lex.next(), Some(Ok(Token::TVerdadeiro)));
        assert_eq!(lex.next(), Some(Ok(Token::TFalso)));
    }

    #[test]
    fn test_string_interpolada() {
        let codigo = r#"$"Olá {nome}, você tem {idade} anos""#;
        let mut lex = Token::lexer(codigo);

        if let Some(Ok(Token::TStringInterpolada(conteudo))) = lex.next() {
            assert_eq!(conteudo, "Olá {nome}, você tem {idade} anos");
        } else {
            panic!("String interpolada não reconhecida");
        }
    }

    #[test]
    fn test_palavras_chave_oop() {
        let codigo = "classe construtor publico privado protegido base redefinível sobrescreve abstrata novo este obter definir estática";
        let mut lex = Token::lexer(codigo);

        assert_eq!(lex.next(), Some(Ok(Token::TClasse)));
        assert_eq!(lex.next(), Some(Ok(Token::TConstrutor)));
        assert_eq!(lex.next(), Some(Ok(Token::TPublico)));
        assert_eq!(lex.next(), Some(Ok(Token::TPrivado)));
        assert_eq!(lex.next(), Some(Ok(Token::TProtegido)));
        assert_eq!(lex.next(), Some(Ok(Token::TBase)));
        assert_eq!(lex.next(), Some(Ok(Token::TRedefinivel)));
        assert_eq!(lex.next(), Some(Ok(Token::TSobrescreve)));
        assert_eq!(lex.next(), Some(Ok(Token::TAbstrato)));
        assert_eq!(lex.next(), Some(Ok(Token::TNovo)));
        assert_eq!(lex.next(), Some(Ok(Token::TEste)));
        assert_eq!(lex.next(), Some(Ok(Token::TObter)));
        assert_eq!(lex.next(), Some(Ok(Token::TDefinir)));
        assert_eq!(lex.next(), Some(Ok(Token::TEstatica)));
    }

    #[test]
    fn test_decimal_type() {
        let codigo = "decimal flutuante duplo";
        let mut lex = Token::lexer(codigo);
        assert_eq!(lex.next(), Some(Ok(Token::TTipoDecimal)));
        assert_eq!(lex.next(), Some(Ok(Token::TTipoFlutuante)));
        assert_eq!(lex.next(), Some(Ok(Token::TTipoDuplo)));
    }
}
