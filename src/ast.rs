#[derive(Debug)]
pub struct Programa {
    pub comandos: Vec<Comando>,
}

#[derive(Debug)]
pub enum Comando {
    Se(Expressao, Box<Comando>),
    Imprima(String),
}

#[derive(Debug)]
pub enum Expressao {
    Inteiro(i64),
    Identificador(String),
    Comparacao(OperadorComparacao, Box<Expressao>, Box<Expressao>),
}

#[derive(Debug)]
pub enum OperadorComparacao {
    MaiorQue,
}