use crate::{ast, lexer::Token};
use logos::Logos;

pub fn parse_string_interpolada(input: &str) -> Result<ast::Expressao, String> {
    let mut parts = Vec::<ast::PartStringInterpolada>::new();
    let mut last_end = 0;

    for (start, _) in input.match_indices('{') {
        let mut balance = 1;
        let mut end_brace = None;

        for (i, c) in input[start + 1..].char_indices() {
            match c {
                '{' => balance += 1,
                '}' => {
                    balance -= 1;
                    if balance == 0 {
                        end_brace = Some(start + 1 + i);
                        break;
                    }
                }
                _ => {}
            }
        }

        if let Some(end) = end_brace {
            if start > last_end {
                parts.push(ast::PartStringInterpolada::Texto(
                    input[last_end..start].into(),
                ));
            }

            let codigo = &input[start + 1..end];
            let lexer = Token::lexer(codigo);
            let tokens: Vec<_> = lexer
                .spanned()
                .filter_map(|(ok, span)| ok.ok().map(|t| (span.start, t, span.end)))
                .collect();

            let expr = crate::parser::ExpressaoParser::new()
                .parse(tokens.iter().cloned())
                .map_err(|e| format!("Erro na expressão interpolada: {:?}", e))?;

            parts.push(ast::PartStringInterpolada::Expressao(expr));
            last_end = end + 1;
        }
    }

    if last_end < input.len() {
        parts.push(ast::PartStringInterpolada::Texto(
            input[last_end..].into(),
        ));
    }

    Ok(ast::Expressao::StringInterpolada(parts))
}

pub fn planificar_interpolada(expr: ast::Expressao) -> ast::Expressao {
    if let ast::Expressao::StringInterpolada(parts) = expr {
        let mut iter = parts.into_iter();
        let mut acc = parte_para_expr(iter.next().unwrap());
        for p in iter {
            acc = ast::Expressao::Aritmetica(
                ast::OperadorAritmetico::Soma,
                Box::new(acc),
                Box::new(parte_para_expr(p)),
            );
        }
        acc
    } else {
        expr
    }
}

pub fn walk_programa<F: FnMut(&mut ast::Expressao)>(p: &mut ast::Programa, mut f: F) {
    fn visita_cmd<F: FnMut(&mut ast::Expressao)>(c: &mut ast::Comando, f:&mut F){
        match c {
            ast::Comando::Imprima(e)
          | ast::Comando::Expressao(e) => f(e),
            ast::Comando::Bloco(cmds) => cmds.iter_mut().for_each(|c|visita_cmd(c,f)),
            _ => {}
        }
    }
    for d in &mut p.declaracoes {
        if let ast::Declaracao::Comando(c) = d { visita_cmd(c, &mut f); }
    }
}

fn parte_para_expr(p: ast::PartStringInterpolada) -> ast::Expressao {
    match p {
        ast::PartStringInterpolada::Texto(t)      => ast::Expressao::Texto(t),
        ast::PartStringInterpolada::Expressao(e)  => e,
    }
}
