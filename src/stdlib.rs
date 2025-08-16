use crate::ast::*;

pub fn criar_biblioteca_padrao() -> Vec<Declaracao> {
    let mut stdlib = Vec::new();

    // Funções matemáticas básicas
    stdlib.extend(criar_funcoes_matematicas());

    // Funções de I/O básicas em português: EscreverLinha e LerLinha
    stdlib.push(Declaracao::DeclaracaoFuncao(DeclaracaoFuncao {
        nome: "EscreverLinha".to_string(),
        parametros: vec![Parametro {
            nome: "texto".to_string(),
            tipo: Tipo::Texto,
            valor_padrao: None,
        }],
        tipo_retorno: Some(Tipo::Vazio),
        modificador: ModificadorAcesso::Publico,
        corpo: vec![],
        eh_estatica: false,
    }));
    stdlib.push(Declaracao::DeclaracaoFuncao(DeclaracaoFuncao {
        nome: "LerLinha".to_string(),
        parametros: vec![],
        tipo_retorno: Some(Tipo::Texto),
        modificador: ModificadorAcesso::Publico,
        corpo: vec![],
        eh_estatica: false,
    }));

    stdlib
}

fn criar_funcoes_matematicas() -> Vec<Declaracao> {
    vec![Declaracao::DeclaracaoFuncao(DeclaracaoFuncao {
        nome: "abs".to_string(),
        parametros: vec![Parametro {
            nome: "valor".to_string(),
            tipo: Tipo::Inteiro,
            valor_padrao: None,
        }],
        tipo_retorno: Some(Tipo::Inteiro),
        modificador: ModificadorAcesso::Publico,
        corpo: vec![],
        eh_estatica: false,
    })]
}
