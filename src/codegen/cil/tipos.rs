// src/codegen/cil/tipos.rs

use crate::ast;

/// Converte um tipo da linguagem Pôr do Sol em uma assinatura de tipo CIL (ECMA-335).
pub fn tipo_para_cil(tipo: &ast::Tipo) -> String {
    match tipo {
        ast::Tipo::Inteiro => "int64".to_string(),
        ast::Tipo::Texto => "string".to_string(),
        ast::Tipo::Booleano => "bool".to_string(),
        ast::Tipo::Flutuante => "float32".to_string(),
        ast::Tipo::Duplo => "float64".to_string(),
        ast::Tipo::Decimal => "valuetype [System.Runtime]System.Decimal".to_string(),
        ast::Tipo::Vazio => "void".to_string(),
        ast::Tipo::Objeto => "object".to_string(),
        ast::Tipo::Lista(inner) => format!("{}[]", tipo_para_cil(inner)),
        ast::Tipo::Classe(nome) => {
            if nome == "objeto" || nome == "object" || nome == "System.Object" {
                "object".to_string()
            } else {
                let sanitized = sanitizar_identificador(nome);
                format!("class {}", sanitized)
            }
        }
        ast::Tipo::Enum(nome) => {
            let sanitized = sanitizar_identificador(nome);
            format!("valuetype {}", sanitized)
        }
        ast::Tipo::Aplicado { nome, args } => {
            let sanitized = sanitizar_identificador(nome);
            let args_cil: Vec<String> = args.iter().map(tipo_para_cil).collect();
            format!("class '{}`{}'<{}>", sanitized, args.len(), args_cil.join(", "))
        }
        ast::Tipo::Generico(param) => format!("!{}", param),
        ast::Tipo::Opcional(inner) => {
            format!(
                "valuetype [System.Runtime]System.Nullable`1<{}>",
                tipo_para_cil(inner)
            )
        }
        ast::Tipo::Funcao(_, _) => "object".to_string(),
        ast::Tipo::Inferido => "object".to_string(),
    }
}

const CIL_KEYWORDS: &[&str] = &[
    "add", "sub", "mul", "div", "rem", "and", "or", "xor", "not", "neg",
    "shl", "shr", "ret", "call", "callvirt", "newobj", "newarr", "ldnull",
    "ldstr", "ldc", "ldarg", "starg", "ldloc", "stloc", "ldfld", "stfld",
    "ldsfld", "stsfld", "ldelem", "stelem", "ldlen", "box", "unbox",
    "castclass", "isinst", "throw", "rethrow", "leave", "br", "brtrue",
    "brfalse", "beq", "bne", "bge", "bgt", "ble", "blt", "dup", "pop", "nop",
    "class", "interface", "value", "enum", "struct", "extends", "implements",
    "public", "private", "family", "assembly", "famandassem", "famorassem",
    "static", "instance", "virtual", "abstract", "final", "hidebysig",
    "newslot", "specialname", "rtspecialname", "managed", "unmanaged", "cil",
    "auto", "ansi", "beforefieldinit", "method", "field", "property",
    "void", "bool", "char", "int8", "int16", "int32", "int64", "float32",
    "float64", "string", "object", "valuetype", "native", "extern",
    "entrypoint", "maxstack", "locals", "init", "true", "false", "null",
];

/// Sanitiza nomes de tipos e identificadores para CIL, preservando namespaces com ponto.
pub fn sanitizar_identificador(nome: &str) -> String {
    let partes: Vec<&str> = nome.split('.').collect();
    let sanitizadas: Vec<String> = partes
        .iter()
        .map(|p| {
            if p.is_empty() {
                return String::new();
            }
            let is_kw = CIL_KEYWORDS.contains(&p.to_ascii_lowercase().as_str());
            if !is_kw && p.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                p.to_string()
            } else {
                format!("'{}'", p)
            }
        })
        .collect();
    sanitizadas.join(".")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Tipo;

    #[test]
    fn test_tipo_primitivos() {
        assert_eq!(tipo_para_cil(&Tipo::Inteiro), "int64");
        assert_eq!(tipo_para_cil(&Tipo::Texto), "string");
        assert_eq!(tipo_para_cil(&Tipo::Booleano), "bool");
        assert_eq!(tipo_para_cil(&Tipo::Flutuante), "float32");
        assert_eq!(tipo_para_cil(&Tipo::Duplo), "float64");
        assert_eq!(tipo_para_cil(&Tipo::Vazio), "void");
        assert_eq!(tipo_para_cil(&Tipo::Objeto), "object");
    }

    #[test]
    fn test_tipo_arrays_e_genericos() {
        assert_eq!(tipo_para_cil(&Tipo::Lista(Box::new(Tipo::Inteiro))), "int64[]");
        assert_eq!(tipo_para_cil(&Tipo::Generico("T".to_string())), "!T");
        assert_eq!(
            tipo_para_cil(&Tipo::Aplicado {
                nome: "Caixa".to_string(),
                args: vec![Tipo::Texto],
            }),
            "class 'Caixa`1'<string>"
        );
    }

    #[test]
    fn test_sanitizar_palavras_chave_cil() {
        assert_eq!(sanitizar_identificador("add"), "'add'");
        assert_eq!(sanitizar_identificador("ret"), "'ret'");
        assert_eq!(sanitizar_identificador("MeuNamespace.add"), "MeuNamespace.'add'");
        assert_eq!(sanitizar_identificador("minha_funcao"), "minha_funcao");
    }
}


