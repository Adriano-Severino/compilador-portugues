use compilador_portugues::ast::{Comando, Declaracao, Expressao};
use compilador_portugues::{CompiladorPortugues, Token, VerificadorTipos};
use logos::Logos;

fn parse(codigo: &str) -> compilador_portugues::Programa {
    let mut compilador = CompiladorPortugues::new();
    compilador
        .compilar_codigo(codigo)
        .unwrap_or_else(|erro| panic!("parse falhou:\n{}\n\ncodigo:\n{}", erro, codigo))
}

fn assert_typecheck_ok(codigo: &str) -> compilador_portugues::Programa {
    let programa = parse(codigo);
    let mut verificador = VerificadorTipos::new();
    verificador
        .verificar_programa(&programa)
        .unwrap_or_else(|erros| {
            panic!(
                "verificacao semantica falhou:\n{}\n\ncodigo:\n{}",
                erros.join("\n"),
                codigo
            )
        });
    programa
}

fn typecheck_errors(codigo: &str) -> Vec<String> {
    let programa = parse(codigo);
    let mut verificador = VerificadorTipos::new();
    verificador
        .verificar_programa(&programa)
        .expect_err("esperava erro semantico, mas o programa foi aceito")
}

fn tokens(codigo: &str) -> Vec<Token> {
    Token::lexer(codigo)
        .map(|token| token.expect("token invalido"))
        .collect()
}

#[test]
fn lexer_reconhece_sintaxe_em_portugues() {
    let tokens = tokens(
        r#"usando espaco classe interface enumeração função retorne se senão enquanto para
           publico privado protegido estática abstrata redefinível sobrescreve
           novo este base obter definir verdadeiro falso inteiro texto booleano flutuante duplo decimal vazio
           "abc" $"ola {nome}" 10 1.5 2.5f 3.50m"#,
    );

    assert!(tokens.contains(&Token::TUsando));
    assert!(tokens.contains(&Token::TEspaco));
    assert!(tokens.contains(&Token::TClasse));
    assert!(tokens.contains(&Token::TInterface));
    assert!(tokens.contains(&Token::TEnumeracao));
    assert!(tokens.contains(&Token::TFuncao));
    assert!(tokens.contains(&Token::TSe));
    assert!(tokens.contains(&Token::TSenao));
    assert!(tokens.contains(&Token::TEnquanto));
    assert!(tokens.contains(&Token::TPara));
    assert!(tokens.contains(&Token::TPublico));
    assert!(tokens.contains(&Token::TPrivado));
    assert!(tokens.contains(&Token::TProtegido));
    assert!(tokens.contains(&Token::TEstatica));
    assert!(tokens.contains(&Token::TAbstrato));
    assert!(tokens.contains(&Token::TRedefinivel));
    assert!(tokens.contains(&Token::TSobrescreve));
    assert!(tokens.contains(&Token::TNovo));
    assert!(tokens.contains(&Token::TEste));
    assert!(tokens.contains(&Token::TBase));
    assert!(tokens.contains(&Token::TObter));
    assert!(tokens.contains(&Token::TDefinir));
    assert!(tokens.contains(&Token::TVerdadeiro));
    assert!(tokens.contains(&Token::TFalso));
    assert!(tokens.contains(&Token::TTipoInteiro));
    assert!(tokens.contains(&Token::TTipoTexto));
    assert!(tokens.contains(&Token::TTipoBooleano));
    assert!(tokens.contains(&Token::TTipoFlutuante));
    assert!(tokens.contains(&Token::TTipoDuplo));
    assert!(tokens.contains(&Token::TTipoDecimal));
    assert!(tokens.contains(&Token::TTipoVazio));
    assert!(tokens.contains(&Token::TString("abc".into())));
    assert!(tokens.contains(&Token::TStringInterpolada("ola {nome}".into())));
    assert!(tokens.contains(&Token::TInteiro(10)));
    assert!(tokens.contains(&Token::TDuploLiteral("1.5".into())));
    assert!(tokens.contains(&Token::TFlutuanteLiteral("2.5f".into())));
    assert!(tokens.contains(&Token::TDecimal("3.50m".into())));
}

#[test]
fn parser_modela_controle_de_fluxo_incluindo_para() {
    let programa = parse(
        r#"
função vazio Principal() {
    inteiro total = 0;
    para (var i = 0; i < 3;) {
        total = total + 1;
    }
}
"#,
    );

    let Declaracao::DeclaracaoFuncao(funcao) = &programa.declaracoes[0] else {
        panic!("esperava funcao top-level");
    };
    assert_eq!(funcao.nome, "Principal");
    assert!(matches!(funcao.corpo[1], Comando::Para(_, _, _, _)));
}

#[test]
fn typecheck_aceita_primitivos_operadores_var_condicional_enquanto_e_interpolacao() {
    assert_typecheck_ok(
        r#"
função vazio Principal() {
    inteiro a = 10;
    inteiro b = 5;
    flutuante f = 2.5f;
    duplo d = 3.5;
    decimal preco = 10.50m;
    booleano ativo = verdadeiro;
    texto nome = "Ana";
    var total = a + b * 2;

    se (total >= 20 && ativo) {
        imprima($"Nome: {nome}");
    } senão {
        imprima("menor");
    }

    enquanto (a < 12) {
        a = a + 1;
    }
}
"#,
    );
}

#[test]
fn typecheck_aceita_funcoes_metodos_propriedades_e_parametros_opcionais() {
    assert_typecheck_ok(
        r#"
publico classe Pessoa {
    publico texto Nome { obter; definir; }
    publico inteiro Idade { obter; definir; }

    publico Pessoa(texto nome, inteiro idade = 18) {
        Nome = nome;
        Idade = idade;
    }

    publico texto Apresentar(texto prefixo = "Nome") {
        retorne $"{prefixo}: {Nome}";
    }
}

publico função texto saudacao(texto nome = "mundo") {
    retorne "Ola " + nome;
}

função vazio Principal() {
    var p = novo Pessoa("Ana");
    p.Nome = "Maria";
    imprima(p.Apresentar());
    imprima(saudacao());
}
"#,
    );
}

#[test]
fn typecheck_aceita_classes_heranca_base_e_override() {
    assert_typecheck_ok(
        r#"
publico classe Animal {
    publico texto Nome { obter; definir; }

    publico Animal(texto nome) {
        Nome = nome;
    }

    publico redefinível texto Falar() {
        retorne "som";
    }
}

publico classe Cachorro : Animal {
    publico Cachorro(texto nome) : base(nome) {
    }

    publico sobrescreve texto Falar() {
        retorne "au";
    }
}

função vazio Principal() {
    Animal a = novo Cachorro("Tobi");
    imprima(a.Falar());
}
"#,
    );
}

#[test]
fn typecheck_aceita_interfaces_arrays_indexacao_e_tamanho() {
    assert_typecheck_ok(
        r#"
publico interface IFalante {
    publico vazio Falar();
}

publico classe Pessoa : IFalante {
    publico vazio Falar() {
        imprima("oi");
    }
}

publico classe Robo : IFalante {
    publico vazio Falar() {
        imprima("beep");
    }
}

função vazio Principal() {
    var falantes = [novo Pessoa(), novo Robo()];
    inteiro quantidade = falantes.tamanho;
    falantes[0].Falar();
    imprima(quantidade);
}
"#,
    );
}

#[test]
fn parser_preserva_declaracoes_genericas() {
    let programa = parse(
        r#"
publico interface IRepositorio<T> {
    publico T Obter(inteiro id);
}

publico classe Caixa<T> {
    privado T valor;

    publico Caixa(T inicial) {
        valor = inicial;
    }

    publico T Obter() {
        retorne valor;
    }
}
"#,
    );

    let Declaracao::DeclaracaoInterface(interface) = &programa.declaracoes[0] else {
        panic!("esperava interface generica");
    };
    assert_eq!(interface.nome, "IRepositorio");
    assert_eq!(interface.generic_params, ["T"]);

    let Declaracao::DeclaracaoClasse(classe) = &programa.declaracoes[1] else {
        panic!("esperava classe generica");
    };
    assert_eq!(classe.nome, "Caixa");
    assert_eq!(classe.generic_params, ["T"]);
}

#[test]
fn typecheck_aceita_enumeracoes_e_rejeita_mistura_de_enums() {
    assert_typecheck_ok(
        r#"
enumeração Cor {
    Vermelho, Verde
}

função vazio Principal() {
    Cor cor = Cor.Verde;
    se (cor == Cor.Verde) {
        imprima("ok");
    }
}
"#,
    );

    let erros = typecheck_errors(
        r#"
enumeração Cor {
    Vermelho, Verde
}

enumeração Status {
    Aberto, Fechado
}

função vazio Principal() {
    Cor cor = Status.Aberto;
}
"#,
    );
    assert!(
        erros
            .iter()
            .any(|erro| erro.contains("não corresponde") || erro.contains("Tipo da expressão")),
        "erro inesperado: {erros:?}"
    );
}

#[test]
fn typecheck_rejeita_classe_que_nao_implementa_interface() {
    let erros = typecheck_errors(
        r#"
publico interface IFalante {
    publico vazio Falar();
}

publico classe Pessoa : IFalante {
}
"#,
    );

    assert!(
        erros
            .iter()
            .any(|erro| erro.contains("não implementa") || erro.contains("Falar")),
        "erro inesperado: {erros:?}"
    );
}

#[test]
fn typecheck_rejeita_atribuicao_com_tipo_incompativel() {
    let erros = typecheck_errors(
        r#"
função vazio Principal() {
    inteiro idade = "trinta";
}
"#,
    );

    assert!(
        erros
            .iter()
            .any(|erro| erro.contains("não corresponde") || erro.contains("Tipo da expressão")),
        "erro inesperado: {erros:?}"
    );
}

#[test]
fn ast_representa_arrays_com_var_literal_e_indexacao() {
    let programa = assert_typecheck_ok(
        r#"
função vazio Principal() {
    var numeros = [1, 2, 3];
    inteiro primeiro = numeros[0];
}
"#,
    );

    let Declaracao::DeclaracaoFuncao(funcao) = &programa.declaracoes[0] else {
        panic!("esperava funcao top-level");
    };
    let Comando::DeclaracaoVar(_, Expressao::ListaLiteral(itens)) = &funcao.corpo[0] else {
        panic!("esperava declaracao de array com literal");
    };

    assert_eq!(itens.len(), 3);
}
