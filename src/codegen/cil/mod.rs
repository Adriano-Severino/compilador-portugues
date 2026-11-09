// src/codegen/cil/mod.rs

pub mod compilador_il;
pub mod declaracoes;
pub mod comandos;
pub mod expressoes;
pub mod tipos;

use crate::ast;
use tipos::sanitizar_identificador;

/// Gerador de código para o alvo CIL (Common Intermediate Language - ECMA-335) do .NET.
pub struct CilGenerator<'a> {
    pub(crate) programa: &'a ast::Programa,
    pub(crate) type_checker: &'a crate::type_checker::VerificadorTipos<'a>,
    pub(crate) assembly_name: String,
    pub(crate) namespace_path: String,
    pub(crate) classe_atual: Option<String>,
    pub(crate) label_counter: usize,
    pub(crate) local_vars: Vec<(String, ast::Tipo)>,
    pub(crate) params: Vec<(String, ast::Tipo)>,
    pub(crate) eh_metodo_estatico: bool,
    pub(crate) generic_params_atuais: Vec<String>,
}

impl<'a> CilGenerator<'a> {
    pub fn new(
        programa: &'a ast::Programa,
        type_checker: &'a crate::type_checker::VerificadorTipos<'a>,
        assembly_name: String,
    ) -> Self {
        Self {
            programa,
            type_checker,
            assembly_name,
            namespace_path: String::new(),
            classe_atual: None,
            label_counter: 0,
            local_vars: Vec::new(),
            params: Vec::new(),
            eh_metodo_estatico: true,
            generic_params_atuais: Vec::new(),
        }
    }

    /// Gera o código CIL completo para o programa Pôr do Sol.
    pub fn generate(&mut self) -> String {
        let mut code = String::new();

        // 1. Cabeçalho do Assembly CIL
        code.push_str("// Código CIL gerado automaticamente pelo Compilador Pôr do Sol\n");
        code.push_str(".assembly extern System.Runtime {}\n");
        code.push_str(".assembly extern System.Console {}\n");
        code.push_str(".assembly extern System.Collections {}\n");
        code.push_str(".assembly extern System.IO.FileSystem {}\n");
        code.push_str(&format!(".assembly {} {{}}\n\n", sanitizar_identificador(&self.assembly_name)));

        // 2. Processa Enums globais e de namespaces
        self.processar_enums(&mut code);

        // 3. Processa Interfaces globais e de namespaces
        self.processar_interfaces(&mut code);

        // 4. Processa Classes globais e de namespaces
        self.processar_classes(&mut code);

        // 5. Emite a classe Principal contendo Main() e comandos top-level
        self.emitir_classe_principal(&mut code);

        code
    }

    fn processar_enums(&mut self, code: &mut String) {
        for decl in &self.programa.declaracoes {
            if let ast::Declaracao::DeclaracaoEnum(en) = decl {
                self.generate_enum(en, code);
            }
        }
        for ns in &self.programa.namespaces {
            self.namespace_path = ns.nome.clone();
            for decl in &ns.declaracoes {
                if let ast::Declaracao::DeclaracaoEnum(en) = decl {
                    self.generate_enum(en, code);
                }
            }
        }
        self.namespace_path.clear();
    }

    fn processar_interfaces(&mut self, code: &mut String) {
        for decl in &self.programa.declaracoes {
            if let ast::Declaracao::DeclaracaoInterface(iface) = decl {
                self.generate_interface(iface, code);
            }
        }
        for ns in &self.programa.namespaces {
            self.namespace_path = ns.nome.clone();
            for decl in &ns.declaracoes {
                if let ast::Declaracao::DeclaracaoInterface(iface) = decl {
                    self.generate_interface(iface, code);
                }
            }
        }
        self.namespace_path.clear();
    }

    fn processar_classes(&mut self, code: &mut String) {
        for decl in &self.programa.declaracoes {
            if let ast::Declaracao::DeclaracaoClasse(cl) = decl {
                self.generate_classe(cl, code);
            }
        }
        for ns in &self.programa.namespaces {
            self.namespace_path = ns.nome.clone();
            for decl in &ns.declaracoes {
                if let ast::Declaracao::DeclaracaoClasse(cl) = decl {
                    self.generate_classe(cl, code);
                }
            }
        }
        self.namespace_path.clear();
    }

    fn emitir_classe_principal(&mut self, code: &mut String) {
        self.classe_atual = Some("Principal".to_string());
        code.push_str(".class public auto ansi beforefieldinit Principal extends [System.Runtime]System.Object\n{\n");

        // Funções globais viram métodos estáticos de Principal
        for decl in &self.programa.declaracoes {
            if let ast::Declaracao::DeclaracaoFuncao(func) = decl {
                self.generate_funcao_global(func, code);
            }
        }

        // Funções declaradas dentro de namespaces também viram métodos estáticos acessíveis
        for ns in &self.programa.namespaces {
            self.namespace_path = ns.nome.clone();
            for decl in &ns.declaracoes {
                if let ast::Declaracao::DeclaracaoFuncao(func) = decl {
                    self.generate_funcao_global(func, code);
                }
            }
        }
        self.namespace_path.clear();

        // Ponto de entrada Main()
        self.limpar_contexto_metodo(true);
        let mut main_body = String::new();

        let mut tem_comandos = false;
        for decl in &self.programa.declaracoes {
            if let ast::Declaracao::Comando(cmd) = decl {
                tem_comandos = true;
                self.generate_comando(cmd, &mut main_body);
            }
        }

        // Se não houver comandos top-level, verifica se existe função Principal() ou principal() definida pelo usuário
        if !tem_comandos {
            let func_principal = self.programa.declaracoes.iter().find_map(|d| match d {
                ast::Declaracao::DeclaracaoFuncao(f) if f.nome.eq_ignore_ascii_case("Principal") => Some(f),
                _ => None,
            }).or_else(|| {
                self.programa.namespaces.iter().find_map(|ns| {
                    ns.declaracoes.iter().find_map(|d| match d {
                        ast::Declaracao::DeclaracaoFuncao(f) if f.nome.eq_ignore_ascii_case("Principal") => Some(f),
                        _ => None,
                    })
                })
            });

            if let Some(func) = func_principal {
                let ret_ty = func.tipo_retorno.as_ref().map(tipos::tipo_para_cil).unwrap_or_else(|| "void".to_string());
                main_body.push_str(&format!("    call {} Principal::{}()\n", ret_ty, sanitizar_identificador(&func.nome)));
                if ret_ty != "void" {
                    main_body.push_str("    pop\n");
                }
            }
        }

        if !main_body.trim_end().ends_with("ret") {
            main_body.push_str("    ret\n");
        }

        code.push_str("  .method public hidebysig static void Main() cil managed\n  {\n");
        code.push_str("    .entrypoint\n");
        code.push_str("    .maxstack 64\n");

        self.emitir_locals(code);
        code.push_str(&main_body);
        code.push_str("  }\n\n");

        // Construtor padrão de Principal
        code.push_str("  .method public hidebysig specialname rtspecialname instance void .ctor() cil managed\n  {\n    ldarg.0\n    call instance void [System.Runtime]System.Object::.ctor()\n    ret\n  }\n");
        code.push_str("}\n");
        self.classe_atual = None;
    }

    fn generate_funcao_global(&mut self, func: &ast::DeclaracaoFuncao, code: &mut String) {
        self.limpar_contexto_metodo(true);

        let vis = match func.modificador {
            ast::ModificadorAcesso::Publico => "public",
            ast::ModificadorAcesso::Privado => "private",
            ast::ModificadorAcesso::Protegido => "family",
        };

        let tipo_retorno = func.tipo_retorno.as_ref()
            .map(|t| tipos::tipo_para_cil(&self.resolver_tipo(t)))
            .unwrap_or_else(|| "void".to_string());

        let mut params_cil = Vec::new();
        for param in &func.parametros {
            let tipo_res = self.resolver_tipo(&param.tipo);
            let t = tipos::tipo_para_cil(&tipo_res);
            let n = sanitizar_identificador(&param.nome);
            params_cil.push(format!("{} {}", t, n));
            self.registrar_parametro(&param.nome, tipo_res);
        }

        let mut body_code = String::new();
        for cmd in &func.corpo {
            self.generate_comando(cmd, &mut body_code);
        }

        if !body_code.trim_end().ends_with("ret") {
            body_code.push_str("    ret\n");
        }

        code.push_str(&format!(
            "  .method {} hidebysig static {} {}({}) cil managed\n  {{\n    .maxstack 64\n",
            vis,
            tipo_retorno,
            sanitizar_identificador(&func.nome),
            params_cil.join(", ")
        ));

        self.emitir_locals(code);
        code.push_str(&body_code);
        code.push_str("  }\n\n");
    }

    // Auxiliares de contexto e resolução
    pub(crate) fn novo_label(&mut self, prefix: &str) -> String {
        self.label_counter += 1;
        format!("{}_{}", prefix, self.label_counter)
    }

    pub(crate) fn limpar_contexto_metodo(&mut self, eh_estatico: bool) {
        self.local_vars.clear();
        self.params.clear();
        self.eh_metodo_estatico = eh_estatico;
    }

    pub(crate) fn resolver_nome_tipo(&self, nome: &str) -> String {
        // 1. Tenta resolver como enum
        if !self.namespace_path.is_empty() {
            let fqn = format!("{}.{}", self.namespace_path, nome);
            if self.type_checker.enums.contains_key(&fqn) {
                return fqn;
            }
        }
        for u in &self.type_checker.usings {
            let fqn = format!("{}.{}", u, nome);
            if self.type_checker.enums.contains_key(&fqn) {
                return fqn;
            }
        }
        if self.type_checker.enums.contains_key(nome) {
            return nome.to_string();
        }

        // 2. Tenta resolver como classe
        let fqn_classe = self.type_checker.resolver_nome_classe(nome, &self.namespace_path);
        if fqn_classe != nome || self.type_checker.classes.contains_key(nome) {
            return fqn_classe;
        }

        // 3. Tenta resolver como interface
        let fqn_iface = self.type_checker.resolver_nome_interface(nome, &self.namespace_path);
        if fqn_iface != nome || self.type_checker.interfaces.contains_key(nome) {
            return fqn_iface;
        }

        nome.to_string()
    }

    pub(crate) fn resolver_tipo(&self, tipo: &ast::Tipo) -> ast::Tipo {
        match tipo {
            ast::Tipo::Classe(nome) => {
                if self.generic_params_atuais.contains(nome) {
                    return ast::Tipo::Generico(nome.clone());
                }
                let fqn = self.resolver_nome_tipo(nome);
                if self.type_checker.enums.contains_key(&fqn) {
                    ast::Tipo::Enum(fqn)
                } else {
                    ast::Tipo::Classe(fqn)
                }
            }
            ast::Tipo::Enum(nome) => {
                let fqn = self.resolver_nome_tipo(nome);
                ast::Tipo::Enum(fqn)
            }
            ast::Tipo::Lista(inner) => {
                ast::Tipo::Lista(Box::new(self.resolver_tipo(inner)))
            }
            ast::Tipo::Opcional(inner) => {
                ast::Tipo::Opcional(Box::new(self.resolver_tipo(inner)))
            }
            ast::Tipo::Aplicado { nome, args } => {
                let fqn = self.resolver_nome_tipo(nome);
                let resolved_args = args.iter().map(|a| self.resolver_tipo(a)).collect();
                ast::Tipo::Aplicado { nome: fqn, args: resolved_args }
            }
            _ => tipo.clone(),
        }
    }

    pub(crate) fn registrar_var_local(&mut self, nome: &str, tipo: ast::Tipo) -> usize {
        let tipo = self.resolver_tipo(&tipo);
        if let Some(pos) = self.local_vars.iter().position(|(n, _)| n == nome) {
            self.local_vars[pos].1 = tipo;
            pos
        } else {
            let idx = self.local_vars.len();
            self.local_vars.push((nome.to_string(), tipo));
            idx
        }
    }

    pub(crate) fn obter_var_local(&self, nome: &str) -> Option<(usize, ast::Tipo)> {
        self.local_vars.iter().position(|(n, _)| n == nome).map(|idx| (idx, self.local_vars[idx].1.clone()))
    }

    pub(crate) fn registrar_parametro(&mut self, nome: &str, tipo: ast::Tipo) {
        let tipo = self.resolver_tipo(&tipo);
        self.params.push((nome.to_string(), tipo));
    }

    pub(crate) fn obter_parametro(&self, nome: &str) -> Option<(usize, ast::Tipo)> {
        self.params.iter().position(|(n, _)| n == nome).map(|idx| {
            let real_idx = if self.eh_metodo_estatico { idx } else { idx + 1 };
            (real_idx, self.params[idx].1.clone())
        })
    }

    pub(crate) fn obter_valor_enum(&self, nome: &str) -> Option<usize> {
        for (enum_fqn, en_decl) in &self.type_checker.enums {
            if let Some(pos) = en_decl.valores.iter().position(|v| v == nome) {
                let short_name = enum_fqn.split('.').last().unwrap_or(enum_fqn);
                if short_name == nome || en_decl.valores.contains(&nome.to_string()) {
                    return Some(pos);
                }
            }
        }
        None
    }

    pub(crate) fn obter_valor_enum_membro(&self, fqn_enum: &str, membro: &str) -> Option<usize> {
        if let Some(en) = self.type_checker.enums.get(fqn_enum) {
            return en.valores.iter().position(|v| v == membro);
        }
        // Fallback: se não encontrar por FQN exato, tenta por nome simples ou sufixo
        for (k, en) in &self.type_checker.enums {
            let short = k.split('.').last().unwrap_or(k);
            if (k == fqn_enum || short == fqn_enum) && en.valores.contains(&membro.to_string()) {
                return en.valores.iter().position(|v| v == membro);
            }
        }
        None
    }

    pub(crate) fn obter_tipo_campo_e_dono(&self, class_fqn: &str, field_name: &str) -> Option<(ast::Tipo, String)> {
        let fqn_resolved = self.type_checker.resolver_nome_classe(class_fqn, &self.namespace_path);
        let mut curr = Some(fqn_resolved);
        while let Some(c) = curr {
            if let Some(resolved) = self.type_checker.resolved_classes.get(&c) {
                if let Some(field) = resolved.fields.iter().find(|f| f.nome == field_name) {
                    return Some((field.tipo.clone(), c));
                }
                curr = resolved.parent_name.clone();
            } else if let Some(class_decl) = self.type_checker.classes.get(&c) {
                if let Some(field) = class_decl.campos.iter().find(|f| f.nome == field_name) {
                    return Some((field.tipo.clone(), c));
                }
                curr = match class_decl.classe_pai.as_ref() {
                    Some(ast::Tipo::Classe(name)) => Some(name.clone()),
                    _ => None,
                };
            } else {
                curr = None;
            }
        }
        None
    }

    pub(crate) fn obter_tipo_campo(&self, class_fqn: &str, field_name: &str) -> Option<ast::Tipo> {
        self.obter_tipo_campo_e_dono(class_fqn, field_name).map(|(t, _)| t)
    }

    pub(crate) fn obter_tipo_propriedade_e_dono(&self, class_fqn: &str, prop_name: &str) -> Option<(ast::Tipo, String)> {
        let fqn_resolved = self.type_checker.resolver_nome_classe(class_fqn, &self.namespace_path);
        let mut curr = Some(fqn_resolved);
        while let Some(c) = curr {
            if let Some(resolved) = self.type_checker.resolved_classes.get(&c) {
                if let Some(prop) = resolved.properties.iter().find(|p| p.nome == prop_name) {
                    return Some((prop.tipo.clone(), c));
                }
                curr = resolved.parent_name.clone();
            } else if let Some(class_decl) = self.type_checker.classes.get(&c) {
                if let Some(prop) = class_decl.propriedades.iter().find(|p| p.nome == prop_name) {
                    return Some((prop.tipo.clone(), c));
                }
                curr = match class_decl.classe_pai.as_ref() {
                    Some(ast::Tipo::Classe(name)) => Some(name.clone()),
                    _ => None,
                };
            } else {
                curr = None;
            }
        }
        None
    }

    pub(crate) fn obter_tipo_propriedade(&self, class_fqn: &str, prop_name: &str) -> Option<ast::Tipo> {
        self.obter_tipo_propriedade_e_dono(class_fqn, prop_name).map(|(t, _)| t)
    }

    pub(crate) fn obter_retorno_metodo(&self, class_fqn: &str, method_name: &str) -> ast::Tipo {
        if let Some(resolved) = self.type_checker.resolved_classes.get(class_fqn) {
            if let Some(metodo) = resolved.methods.get(method_name) {
                return metodo.tipo_retorno.clone().unwrap_or(ast::Tipo::Vazio);
            }
        }
        if let Some(class_decl) = self.type_checker.classes.get(class_fqn) {
            if let Some(metodo) = class_decl.metodos.iter().find(|m| m.nome == method_name) {
                return metodo.tipo_retorno.clone().unwrap_or(ast::Tipo::Vazio);
            }
        }
        if let Some(iface) = self.type_checker.interfaces.get(class_fqn) {
            if let Some(metodo) = iface.metodos.iter().find(|m| m.nome == method_name) {
                return metodo.tipo_retorno.clone().unwrap_or(ast::Tipo::Vazio);
            }
        }
        let simple = class_fqn.split('.').last().unwrap_or(class_fqn);
        if let Some(iface) = self.type_checker.interfaces.get(simple) {
            if let Some(metodo) = iface.metodos.iter().find(|m| m.nome == method_name) {
                return metodo.tipo_retorno.clone().unwrap_or(ast::Tipo::Vazio);
            }
        }
        // Fallback: procura em qualquer interface registrada
        for (_, iface) in &self.type_checker.interfaces {
            if let Some(metodo) = iface.metodos.iter().find(|m| m.nome == method_name) {
                return metodo.tipo_retorno.clone().unwrap_or(ast::Tipo::Vazio);
            }
        }
        // Fallback: procura em qualquer classe resolvida
        for (_, rc) in &self.type_checker.resolved_classes {
            if let Some(metodo) = rc.methods.get(method_name) {
                return metodo.tipo_retorno.clone().unwrap_or(ast::Tipo::Vazio);
            }
        }
        ast::Tipo::Vazio
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::*;
    use crate::type_checker::VerificadorTipos;

    #[test]
    fn test_gerar_cil_programa_minimo() {
        let programa = Programa {
            usings: vec![],
            namespaces: vec![],
            declaracoes: vec![Declaracao::DeclaracaoFuncao(DeclaracaoFuncao {
                attributes: vec![],
                nome: "Principal".to_string(),
                generic_params: vec![],
                parametros: vec![],
                tipo_retorno: Some(Tipo::Vazio),
                modificador: ModificadorAcesso::Publico,
                corpo: vec![Comando::Expressao(Expressao::ChamadaMetodo(
                    Box::new(Expressao::Identificador("Console".to_string())),
                    "EscreverLinha".to_string(),
                    vec![Expressao::Texto("Olá CIL".to_string())],
                ))],
                eh_estatica: true,
                eh_assincrona: false,
            })],
        };

        let mut tc = VerificadorTipos::new();
        let _ = tc.verificar_programa(&programa);

        let mut gen = CilGenerator::new(&programa, &tc, "teste_min".to_string());
        let cil = gen.generate();

        assert!(cil.contains(".assembly teste_min {}"));
        assert!(cil.contains(".method public hidebysig static void Main()"));
        assert!(cil.contains("call void [System.Console]System.Console::WriteLine(string)"));
    }
}

