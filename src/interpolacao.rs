use crate::ast;
use crate::lexer::Token;
use logos::Logos;

pub fn parse_string_interpolada(input: &str) -> Result<ast::Expressao, String> {
    let mut parts = Vec::new();
    let mut last_end = 0;
    
    for (start, _) in input.match_indices('{') {
        let mut balance = 1;
        let mut end_brace_pos = None;
        
        for (i, c) in input[start + 1..].char_indices() {
            if c == '{' {
                balance += 1;
            } else if c == '}' {
                balance -= 1;
                if balance == 0 {
                    end_brace_pos = Some(start + 1 + i);
                    break;
                }
            }
        }
        
        if let Some(end) = end_brace_pos {
            if start > last_end {
                parts.push(ast::PartStringInterpolada::Texto(
                    input[last_end..start].to_string(),
                ));
            }
            
            let expr_code = &input[start + 1..end];
            let lexer = Token::lexer(expr_code);
            let tokens: Vec<_> = lexer
                .spanned()
                .filter_map(|(tok_res, span)| {
                    tok_res.ok().map(|tok| (span.start, tok, span.end))
                })
                .collect();
            
            let expr_parser = crate::parser::ExpressaoParser::new();
            let parsed_expr = expr_parser
                .parse(tokens.iter().cloned())
                .map_err(|e| format!("Erro ao analisar expressão interpolada: {:?}", e))?;
            
            parts.push(ast::PartStringInterpolada::Expressao(parsed_expr));
            last_end = end + 1;
        }
    }
    
    if last_end < input.len() {
        parts.push(ast::PartStringInterpolada::Texto(
            input[last_end..].to_string(),
        ));
    }
    
    Ok(ast::Expressao::StringInterpolada(parts))
}