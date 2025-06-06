#[derive(Debug)]
pub enum Comando {
    Se(Expressao, Box<Comando>),
    Imprima(String),
}

#[derive(Debug)]
pub enum Expressao {
    Inteiro(i64),
    Identificador(String),
}
