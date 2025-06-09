use crate::ast::*;

pub fn criar_biblioteca_padrao() -> Vec<Declaracao> {
    let mut stdlib = Vec::new();
    
    // Funções matemáticas básicas
    stdlib.extend(criar_funcoes_matematicas());
    
    stdlib
}

fn criar_funcoes_matematicas() -> Vec<Declaracao> {
    vec![
        Declaracao::DeclaracaoFuncao(DeclaracaoFuncao {
            nome: "abs".to_string(),
            parametros: vec![
                Parametro {
                    nome: "valor".to_string(),
                    tipo: Tipo::Inteiro,
                    valor_padrao: None,
                }
            ],
            tipo_retorno: Some(Tipo::Inteiro),
            modificador: ModificadorAcesso::Publico,
            corpo: vec![],
        }),
    ]
}