#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Programa {
    pub comandos: Vec<Comando>,
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
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum Tipo {
    Inteiro,
    Texto,
    Booleano,
    Vazio,
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
    Modulo,  // Adicionado operador módulo
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