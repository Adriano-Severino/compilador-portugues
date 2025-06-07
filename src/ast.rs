#[derive(Debug)]
pub enum Comando {
    Se(Expressao, Box<Comando>),
    Imprima(String),
}

#[derive(Debug)]
pub enum Expressao {
    Identificador(String),
    Inteiro(i64),
    Comparacao(OperadorComparacao, Box<Expressao>, Box<Expressao>),
}

#[derive(Debug)]
pub enum OperadorComparacao {
    MaiorQue,
}