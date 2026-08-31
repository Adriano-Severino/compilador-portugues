use crate::ast;
use crate::ast::*;
use crate::error::{ErroCompilador, TipoErro};
use crate::library_loader::{self, LibSimbolo};
use std::collections::{HashMap, HashSet};
use super::{ResolvedClassInfo, string_para_tipo};
use super::VerificadorTipos;

impl<'a> VerificadorTipos<'a> {
    pub fn inferir_tipo_expressao(
        &mut self,
        expressao: &Expressao,
        namespace_atual: &str,
        classe_atual: Option<&String>,
        escopo_vars: &HashMap<String, Tipo>,
    ) -> Tipo {
        match expressao {
            Expressao::Inteiro(_) => Tipo::Inteiro,
            Expressao::Texto(_) => Tipo::Texto,
            Expressao::Booleano(_) => Tipo::Booleano,
            Expressao::FlutuanteLiteral(_) => Tipo::Flutuante,
            Expressao::DuploLiteral(_) => Tipo::Duplo,
            Expressao::Decimal(_) => Tipo::Decimal,
            Expressao::Nulo => Tipo::Classe("objeto".to_string()),
            Expressao::Aguarde(inner) => match inner.as_ref() {
                Expressao::Chamada(nome, argumentos) => {
                    for argumento in argumentos {
                        self.inferir_tipo_expressao(
                            argumento,
                            namespace_atual,
                            classe_atual,
                            escopo_vars,
                        );
                    }
                    match nome.as_str() {
                        "LerArquivoAssíncrono" => Tipo::Texto,
                        "EscreverArquivoAssíncrono" | "VerificarArquivoAssíncrono" => {
                            Tipo::Booleano
                        }
                        _ => {
                            let fqn = self.resolver_nome_funcao(nome, namespace_atual);
                            match self.simbolos_namespaces.get(&fqn) {
                                Some(Declaracao::DeclaracaoFuncao(function))
                                    if function.eh_assincrona =>
                                {
                                    function.tipo_retorno.clone().unwrap_or(Tipo::Vazio)
                                }
                                _ => {
                                    self.erros.push(ErroCompilador::novo(
                                        TipoErro::Semântico,
                                        format!(
                                        "aguarde requer uma função assíncrona; '{}' não foi encontrada ou não foi marcada como assíncrona.",
                                        nome
                                    )));
                                    Tipo::Inferido
                                }
                            }
                        }
                    }
                }
                Expressao::ChamadaMetodo(objeto, metodo, argumentos) if matches!(objeto.as_ref(), Expressao::Identificador(nome) if nome == "Arquivo") =>
                {
                    for argumento in argumentos {
                        self.inferir_tipo_expressao(
                            argumento,
                            namespace_atual,
                            classe_atual,
                            escopo_vars,
                        );
                    }
                    match metodo.as_str() {
                        "LerTextoAssíncrono" => Tipo::Texto,
                        "EscreverTextoAssíncrono" | "ExisteAssíncrono" => Tipo::Booleano,
                        _ => Tipo::Inferido,
                    }
                }
                _ => {
                    self.erros.push(ErroCompilador::novo(
                        TipoErro::Semântico,
                        "aguarde requer uma chamada assíncrona.".to_string(),
                    ));
                    Tipo::Inferido
                }
            },
            Expressao::Este => {
                classe_atual.map_or(Tipo::Inferido, |nome| Tipo::Classe(nome.clone()))
            }
            Expressao::Identificador(nome) => {
                if escopo_vars.contains_key(nome) {
                    return escopo_vars.get(nome).unwrap().clone();
                }
                if let Some(class_name) = classe_atual {
                    if let Some(class_info) = self.resolved_classes.get(class_name) {
                        if class_info.properties.iter().any(|p| p.nome == *nome)
                            || class_info.fields.iter().any(|f| f.nome == *nome)
                        {
                            return self.inferir_tipo_expressao(
                                &Expressao::AcessoMembro(Box::new(Expressao::Este), nome.clone()),
                                namespace_atual,
                                classe_atual,
                                escopo_vars,
                            );
                        }
                    }
                }
                // Se o nome é um namespace stdlib (ex: "Sistema"), trata como classe para acesso a membros
                if self.eh_classe_stdlib(nome) {
                    return Tipo::Classe(nome.to_string());
                }
                // Classe?
                let fqn_class = self.resolver_nome_classe(nome, namespace_atual);
                if self.classes.contains_key(&fqn_class) {
                    return Tipo::Classe(fqn_class);
                }
                // NOVO: Verificar se a classe está na biblioteca externa
                if let Some(bib) = &self.biblioteca_externa {
                    if bib.simbolos.contains_key(&fqn_class) {
                        return Tipo::Classe(fqn_class);
                    }
                }
                // Se a classe está em um namespace stdlib, confia que existe (evita erro)
                let namespace_do_identificador = if fqn_class.contains('.') {
                    fqn_class.rfind('.').map(|i| &fqn_class[..i]).unwrap_or("")
                } else {
                    ""
                };
                if self.eh_classe_stdlib(&namespace_do_identificador) {
                    return Tipo::Classe(fqn_class);
                }
                // Enum?
                let fqn_enum = self.resolver_nome_enum(nome, namespace_atual);
                if self.enums.contains_key(&fqn_enum) {
                    return Tipo::Enum(fqn_enum);
                }
                self.erros.push(ErroCompilador::novo(
                    TipoErro::Semântico,
                    format!("Identificador \"{}\" não encontrado.", nome),
                ));
                Tipo::Inferido
            }
            Expressao::AcessoMembro(obj_expr, membro_nome) => {
                let obj_tipo = self.inferir_tipo_expressao(
                    obj_expr,
                    namespace_atual,
                    classe_atual,
                    escopo_vars,
                );

                let lookup_class_name = match &obj_tipo {
                    Tipo::Classe(nome) => Some(nome.clone()),
                    Tipo::Aplicado { nome, .. } => Some(nome.clone()),
                    _ => None,
                };

                if let Some(nome_classe) = lookup_class_name {
                    let fqn = self.resolver_nome_classe(&nome_classe, namespace_atual);

                    // NOVO: Consultar biblioteca externa para métodos
                    if let Some(bib) = &self.biblioteca_externa {
                        if let Some(LibSimbolo::Classe(lib_classe)) = bib.simbolos.get(&fqn) {
                            if let Some(metodo) = lib_classe.metodos.get(membro_nome) {
                                return string_para_tipo(&metodo.tipo_retorno);
                            }
                        }
                    }

                    if let Some(class_info) = self.resolved_classes.get(&fqn) {
                        if let Some(prop) = class_info
                            .properties
                            .iter()
                            .find(|p| p.nome == *membro_nome)
                        {
                            // Verificar modificador de acesso para propriedades (semântica C#)
                            let is_private = prop.modificador == ModificadorAcesso::Privado;
                            let is_protected = prop.modificador == ModificadorAcesso::Protegido;
                            let inside_same = classe_atual.map_or(false, |c| c == &fqn);
                            let inside_sub =
                                classe_atual.map_or(false, |c| self.is_subclass_of(c, &fqn));

                            if is_private && !inside_same {
                                self.erros.push(ErroCompilador::novo(
                                    TipoErro::Semântico,
                                    format!(
                                    "A propriedade '{}' de '{}' é inacessível: é privada e só pode ser acessada dentro da própria classe.",
                                    membro_nome, fqn
                                )));
                            } else if is_protected && !inside_same && !inside_sub {
                                self.erros.push(ErroCompilador::novo(
                                    TipoErro::Semântico,
                                    format!(
                                    "A propriedade '{}' de '{}' é inacessível: é protegida e só pode ser acessada dentro da classe ou de subclasses.",
                                    membro_nome, fqn
                                )));
                            }
                            return prop.tipo.clone();
                        }
                        if let Some(field) =
                            class_info.fields.iter().find(|f| f.nome == *membro_nome)
                        {
                            // Verificar modificador de acesso para campos (semântica C#)
                            let is_private = field.modificador == ModificadorAcesso::Privado;
                            let is_protected = field.modificador == ModificadorAcesso::Protegido;
                            let inside_same = classe_atual.map_or(false, |c| c == &fqn);
                            let inside_sub =
                                classe_atual.map_or(false, |c| self.is_subclass_of(c, &fqn));

                            if is_private && !inside_same {
                                self.erros.push(ErroCompilador::novo(
                                    TipoErro::Semântico,
                                    format!(
                                    "O campo '{}' de '{}' é inacessível: é privado e só pode ser acessado dentro da própria classe.",
                                    membro_nome, fqn
                                )));
                            } else if is_protected && !inside_same && !inside_sub {
                                self.erros.push(ErroCompilador::novo(
                                    TipoErro::Semântico,
                                    format!(
                                    "O campo '{}' de '{}' é inacessível: é protegido e só pode ser acessado dentro da classe ou de subclasses.",
                                    membro_nome, fqn
                                )));
                            }
                            return field.tipo.clone();
                        }
                    }
                }
                // Propriedade especial de arrays e textos
                if membro_nome == "tamanho" {
                    if matches!(obj_tipo, Tipo::Lista(_) | Tipo::Texto) {
                        return Tipo::Inteiro;
                    }
                }
                // Enum membro? O membro possui o tipo do próprio enum
                if let Tipo::Enum(ref fqn_enum) = obj_tipo {
                    if let Some(en) = self.enums.get(fqn_enum) {
                        if en.valores.iter().any(|v| v == membro_nome) {
                            return Tipo::Enum(fqn_enum.clone());
                        } else {
                            self.erros.push(ErroCompilador::novo(
                                TipoErro::Semântico,
                                format!(
                                    "Membro \"{}\" não existe no enum \"{}\".",
                                    membro_nome, fqn_enum
                                ),
                            ));
                        }
                    } else {
                        self.erros.push(ErroCompilador::novo(
                            TipoErro::Semântico,
                            format!(
                                "Enum \"{}\" não encontrado ao acessar membro \"{}\".",
                                fqn_enum, membro_nome
                            ),
                        ));
                    }
                }

                // Fallback: if the object is a class, we return Inferido but avoid immediate error
                if let Tipo::Classe(_) = obj_tipo {
                    return Tipo::Inferido;
                }

                Tipo::Inferido
            }
            Expressao::ListaLiteral(items) => {
                // Inferência de tipo para listas: tenta encontrar tipo comum
                if items.is_empty() {
                    return Tipo::Lista(Box::new(Tipo::Inferido));
                }
                // Coletar tipos de todos os itens
                let tipos: Vec<Tipo> = items
                    .iter()
                    .map(|e| {
                        self.inferir_tipo_expressao(e, namespace_atual, classe_atual, escopo_vars)
                    })
                    .collect();
                // 1) Se todos compatíveis com o primeiro (e vice-versa), use o primeiro
                let first = tipos[0].clone();
                let mut todos_compat = true;
                for t in &tipos[1..] {
                    if !self.tipos_compativeis_atribuicao(&first, t)
                        || !self.tipos_compativeis_atribuicao(t, &first)
                    {
                        todos_compat = false;
                        break;
                    }
                }
                if todos_compat {
                    return Tipo::Lista(Box::new(first));
                }
                // 2) Se todos forem classes, tentar achar interface comum

                let classes: Option<Vec<String>> = tipos
                    .iter()
                    .map(|t| {
                        if let Tipo::Classe(c) = t {
                            Some(c.clone())
                        } else {
                            None
                        }
                    })
                    .collect();
                if let Some(cls_vec) = classes {
                    if !cls_vec.is_empty() {
                        use std::collections::HashSet;
                        // Começa com interfaces do primeiro e intersecta com os demais
                        let mut intersec: HashSet<String> =
                            self.get_all_interfaces_of_class(&cls_vec[0]);
                        for c in &cls_vec[1..] {
                            let si = self.get_all_interfaces_of_class(c);
                            intersec = intersec.intersection(&si).cloned().collect::<HashSet<_>>();
                            if intersec.is_empty() {
                                break;
                            }
                        }
                        if let Some(iface_fqn) = intersec.into_iter().next() {
                            return Tipo::Lista(Box::new(Tipo::Classe(iface_fqn)));
                        }
                    }
                }
                // 3) Falha — tipos heterogêneos sem supertipo comum
                self.erros.push(ErroCompilador::novo(
                    TipoErro::Semântico,
                    "Elementos do array devem ter tipos compatíveis".into(),
                ));
                Tipo::Lista(Box::new(Tipo::Inferido))
            }
            Expressao::AcessoIndice(obj, idx) => {
                let t_obj =
                    self.inferir_tipo_expressao(obj, namespace_atual, classe_atual, escopo_vars);
                let t_idx =
                    self.inferir_tipo_expressao(idx, namespace_atual, classe_atual, escopo_vars);
                if t_idx != Tipo::Inteiro {
                    self.erros.push(ErroCompilador::novo(
                        TipoErro::Semântico,
                        "Índice de acesso deve ser inteiro".into(),
                    ));
                }
                if let Tipo::Lista(elem) = t_obj {
                    return *elem;
                }
                self.erros.push(ErroCompilador::novo(
                    TipoErro::Semântico,
                    "Acesso por índice requer lista".into(),
                ));
                Tipo::Inferido
            }
            Expressao::NovoObjeto(t, args) => {
                let (t_norm, mut errs) = self.normalize_tipo_ro(t, namespace_atual);
                self.erros.append(&mut errs);

                if let Tipo::Classe(nome_classe) = &t_norm {
                    if let Some(classe_decl) = self.classes.get(nome_classe) {
                        // Se não houver construtores definidos e a chamada é sem argumentos,
                        // é uma chamada ao construtor padrão implícito, que é público.
                        if classe_decl.construtores.is_empty() && args.is_empty() {
                            // Construtor padrão, acesso permitido.
                        } else if let Some(construtor) = classe_decl.construtores.iter().find(|c| {
                            // Aceita se o número de args está entre
                            // os parâmetros obrigatórios e o total (C# semantics)
                            let total = c.parametros.len();
                            let obrigatorios = c
                                .parametros
                                .iter()
                                .filter(|p| p.valor_padrao.is_none())
                                .count();
                            args.len() >= obrigatorios && args.len() <= total
                        }) {
                            let is_private = construtor.modificador == ModificadorAcesso::Privado;
                            let is_protected =
                                construtor.modificador == ModificadorAcesso::Protegido;

                            let is_inside_same_class = classe_atual
                                .map_or(false, |current_class_fqn| {
                                    current_class_fqn == nome_classe
                                });

                            let is_inside_subclass = if let Some(current_class_fqn) = classe_atual {
                                self.is_subclass_of(current_class_fqn, nome_classe)
                            } else {
                                false
                            };

                            if (is_private && !is_inside_same_class)
                                || (is_protected && !is_inside_subclass)
                            {
                                self.erros.push(ErroCompilador::novo(
                                    TipoErro::Semântico,
                                    format!(
                                    "O construtor da classe '{}' é inacessível devido ao seu nível de proteção.",
                                    nome_classe
                                )));
                            }
                        } else {
                            self.erros.push(ErroCompilador::novo(
                                TipoErro::Semântico,
                                format!(
                                "A classe '{}' não contém um construtor que receba {} argumentos.",
                                nome_classe,
                                args.len()
                            ),
                            ));
                        }
                    }
                }

                t_norm
            }
            Expressao::NovoArray(t, _) => {
                let (t_norm, mut errs) = self.normalize_tipo_ro(t, namespace_atual);
                self.erros.append(&mut errs);
                Tipo::Lista(Box::new(t_norm))
            }
            Expressao::Aritmetica(_, esq, dir) => {
                let te =
                    self.inferir_tipo_expressao(esq, namespace_atual, classe_atual, escopo_vars);
                let td =
                    self.inferir_tipo_expressao(dir, namespace_atual, classe_atual, escopo_vars);
                // Promoção numérica simples: Duplo > Flutuante > Inteiro; Decimal tratado a parte
                use Tipo::*;
                match (te, td) {
                    (Decimal, _) | (_, Decimal) => Decimal,
                    (Duplo, _) | (_, Duplo) => Duplo,
                    (Flutuante, _) | (_, Flutuante) => Flutuante,
                    (Inteiro, Inteiro) => Inteiro,
                    _ => Inteiro,
                }
            }
            Expressao::Comparacao(_, _, _) => Tipo::Booleano,
            Expressao::Logica(_, _, _) => Tipo::Booleano,
            Expressao::ChamadaMetodo(obj_expr, metodo_nome, _args) => {
                let obj_tipo = self.inferir_tipo_expressao(
                    obj_expr,
                    namespace_atual,
                    classe_atual,
                    escopo_vars,
                );
                if let Tipo::Classe(nome_classe) = obj_tipo {
                    // NOVO: Consultar biblioteca externa primeiro
                    if let Some(bib) = &self.biblioteca_externa {
                        if let Some(LibSimbolo::Classe(lib_classe)) = bib.simbolos.get(&nome_classe)
                        {
                            if let Some(metodo) = lib_classe.metodos.get(metodo_nome) {
                                return string_para_tipo(&metodo.tipo_retorno);
                            }
                        }
                    }
                    if let Some(class_info) = self.resolved_classes.get(&nome_classe) {
                        if let Some(metodo) = class_info.methods.get(metodo_nome) {
                            let mut is_static_access = false;
                            if let Expressao::Identificador(nome_id) = obj_expr.as_ref() {
                                if !escopo_vars.contains_key(nome_id) {
                                    let is_member = classe_atual
                                        .and_then(|c| self.resolved_classes.get(c))
                                        .map_or(false, |info| {
                                            info.properties.iter().any(|p| p.nome == *nome_id)
                                                || info.fields.iter().any(|f| f.nome == *nome_id)
                                        });
                                    if !is_member {
                                        is_static_access = true;
                                    }
                                }
                            }

                            if metodo.eh_estatica && !is_static_access {
                                self.erros.push(ErroCompilador::novo(
                                    TipoErro::Semântico,
                                    format!(
                                        "O método '{}' de '{}' é estático e não pode ser chamado a partir de uma instância.",
                                        metodo_nome, nome_classe
                                    )
                                ));
                            } else if !metodo.eh_estatica && is_static_access {
                                self.erros.push(ErroCompilador::novo(
                                    TipoErro::Semântico,
                                    format!(
                                        "O método '{}' de '{}' não é estático e não pode ser chamado diretamente pela classe.",
                                        metodo_nome, nome_classe
                                    )
                                ));
                            }

                            // Verificar modificador de acesso (semântica C#)
                            let is_private = metodo.modificador == ModificadorAcesso::Privado;
                            let is_protected = metodo.modificador == ModificadorAcesso::Protegido;
                            let inside_same = classe_atual.map_or(false, |c| c == &nome_classe);
                            let inside_sub = classe_atual
                                .map_or(false, |c| self.is_subclass_of(c, &nome_classe));

                            if is_private && !inside_same {
                                self.erros.push(ErroCompilador::novo(
                                    TipoErro::Semântico,
                                    format!(
                                        "O método '{}' de '{}' é inacessível: é privado e só pode ser chamado dentro da própria classe.",
                                        metodo_nome, nome_classe
                                    )
                                ));
                            } else if is_protected && !inside_same && !inside_sub {
                                self.erros.push(ErroCompilador::novo(
                                    TipoErro::Semântico,
                                    format!(
                                        "O método '{}' de '{}' é inacessível: é protegido e só pode ser chamado dentro da classe ou de subclasses.",
                                        metodo_nome, nome_classe
                                    )
                                ));
                            }
                            return metodo.tipo_retorno.clone().unwrap_or(Tipo::Vazio);
                        }
                    }
                }
                Tipo::Inferido
            }
            _ => Tipo::Inferido,
        }
    }

    pub fn get_expr_type(
        &self,
        expressao: &Expressao,
        namespace_atual: &str,
        classe_atual: Option<&String>,
        escopo_vars: &HashMap<String, Tipo>,
    ) -> Tipo {
        match expressao {
            Expressao::Inteiro(_) => Tipo::Inteiro,
            Expressao::Texto(_) => Tipo::Texto,
            Expressao::Booleano(_) => Tipo::Booleano,
            Expressao::Decimal(_) => Tipo::Decimal,
            Expressao::Nulo => Tipo::Classe("objeto".to_string()),
            Expressao::Este => {
                classe_atual.map_or(Tipo::Inferido, |nome| Tipo::Classe(nome.clone()))
            }
            Expressao::Identificador(nome) => {
                if escopo_vars.contains_key(nome) {
                    return escopo_vars.get(nome).unwrap().clone();
                }
                if let Some(class_name) = classe_atual {
                    if let Some(class_info) = self.resolved_classes.get(class_name) {
                        if class_info.properties.iter().any(|p| p.nome == *nome)
                            || class_info.fields.iter().any(|f| f.nome == *nome)
                        {
                            return self.get_expr_type(
                                &Expressao::AcessoMembro(Box::new(Expressao::Este), nome.clone()),
                                namespace_atual,
                                classe_atual,
                                escopo_vars,
                            );
                        }
                    }
                }
                let fqn_class = self.resolver_nome_classe(nome, namespace_atual);
                if self.classes.contains_key(&fqn_class) {
                    return Tipo::Classe(fqn_class);
                }
                // NOVO: Verificar se a classe está na biblioteca externa
                if let Some(bib) = &self.biblioteca_externa {
                    if bib.simbolos.contains_key(&fqn_class) {
                        return Tipo::Classe(fqn_class);
                    }
                }
                let fqn_enum = self.resolver_nome_enum(nome, namespace_atual);
                if self.enums.contains_key(&fqn_enum) {
                    return Tipo::Enum(fqn_enum);
                }
                Tipo::Inferido
            }
            Expressao::AcessoMembro(obj_expr, membro_nome) => {
                let obj_tipo =
                    self.get_expr_type(obj_expr, namespace_atual, classe_atual, escopo_vars);
                if let Tipo::Classe(ref nome_classe) = obj_tipo {
                    // NOVO: Consultar biblioteca externa primeiro
                    if let Some(bib) = &self.biblioteca_externa {
                        if let Some(LibSimbolo::Classe(lib_classe)) = bib.simbolos.get(nome_classe)
                        {
                            if let Some(metodo) = lib_classe.metodos.get(membro_nome) {
                                return string_para_tipo(&metodo.tipo_retorno);
                            }
                        }
                    }
                    if let Some(class_info) = self.resolved_classes.get(nome_classe) {
                        if let Some(prop) = class_info
                            .properties
                            .iter()
                            .find(|p| p.nome == *membro_nome)
                        {
                            return prop.tipo.clone();
                        }
                        if let Some(field) =
                            class_info.fields.iter().find(|f| f.nome == *membro_nome)
                        {
                            return field.tipo.clone();
                        }
                    }
                }
                if membro_nome == "tamanho" {
                    if matches!(obj_tipo, Tipo::Lista(_) | Tipo::Texto) {
                        return Tipo::Inteiro;
                    }
                }
                if let Tipo::Enum(ref fqn_enum) = obj_tipo {
                    if let Some(en) = self.enums.get(fqn_enum) {
                        if en.valores.iter().any(|v| v == membro_nome) {
                            return Tipo::Enum(fqn_enum.clone());
                        }
                    }
                }
                Tipo::Inferido
            }
            Expressao::ListaLiteral(items) => {
                if items.is_empty() {
                    return Tipo::Lista(Box::new(Tipo::Inferido));
                }
                let first =
                    self.get_expr_type(&items[0], namespace_atual, classe_atual, escopo_vars);
                return Tipo::Lista(Box::new(first));
            }
            Expressao::AcessoIndice(obj, _idx) => {
                let t_obj = self.get_expr_type(obj, namespace_atual, classe_atual, escopo_vars);
                if let Tipo::Lista(elem) = t_obj {
                    return *elem;
                }
                Tipo::Inferido
            }
            Expressao::NovoObjeto(t, _) => {
                let (t_norm, _) = self.normalize_tipo_ro(t, namespace_atual);
                t_norm
            }
            Expressao::NovoArray(t, _) => {
                let (t_norm, _) = self.normalize_tipo_ro(t, namespace_atual);
                Tipo::Lista(Box::new(t_norm))
            }
            Expressao::Aritmetica(_, esq, dir) => {
                let te = self.get_expr_type(esq, namespace_atual, classe_atual, escopo_vars);
                let td = self.get_expr_type(dir, namespace_atual, classe_atual, escopo_vars);
                use Tipo::*;
                match (te, td) {
                    (Decimal, _) | (_, Decimal) => Decimal,
                    (Duplo, _) | (_, Duplo) => Duplo,
                    (Flutuante, _) | (_, Flutuante) => Flutuante,
                    (Inteiro, Inteiro) => Inteiro,
                    _ => Inteiro,
                }
            }
            Expressao::Comparacao(_, _, _) => Tipo::Booleano,
            Expressao::Logica(_, _, _) => Tipo::Booleano,
            _ => Tipo::Inferido,
        }
    }

    pub(crate) fn normalize_tipo_ro(&self, t: &Tipo, namespace_atual: &str) -> (Tipo, Vec<ErroCompilador>) {
        use Tipo::*;
        match t {
            Lista(inner) => {
                let (norm, errs) = self.normalize_tipo_ro(inner, namespace_atual);
                (Lista(Box::new(norm)), { errs })
            }
            Classe(n) => {
                // Check if it's a generic parameter
                for scope in self.generic_scope.iter().rev() {
                    if scope.contains(n) {
                        return (Generico(n.clone()), vec![]);
                    }
                }
                let fqn_enum = self.resolver_nome_enum(n, namespace_atual);
                if self.enums.contains_key(&fqn_enum) {
                    return (Enum(fqn_enum), vec![]);
                }
                (
                    Classe(self.resolver_nome_classe(n, namespace_atual)),
                    vec![],
                )
            }
            Enum(n) => (Enum(self.resolver_nome_enum(n, namespace_atual)), vec![]),
            Aplicado { nome, args } => {
                // Check if it's a generic parameter
                for scope in self.generic_scope.iter().rev() {
                    if scope.contains(nome) {
                        return (Generico(nome.clone()), vec![]);
                    }
                }
                let fqn_cls = self.resolver_nome_classe(nome, namespace_atual);
                let fqn_iface = self.resolver_nome_interface(nome, namespace_atual);
                let (is_class, is_iface, resolved_name) = (
                    self.classes.contains_key(&fqn_cls),
                    self.interfaces.contains_key(&fqn_iface),
                    if self.classes.contains_key(&fqn_cls) {
                        fqn_cls.clone()
                    } else if self.interfaces.contains_key(&fqn_iface) {
                        fqn_iface.clone()
                    } else {
                        nome.clone()
                    },
                );
                let mut erros: Vec<ErroCompilador> = Vec::new();
                if is_class {
                    if let Some(decl) = self.classes.get(&fqn_cls) {
                        let expected = decl.generic_params.len();
                        if expected == 0 {
                            erros.push(ErroCompilador::novo(
                                TipoErro::Semântico,
                                format!(
                                "Tipo '{}' não é genérico, mas foi usado como '{}' com argumentos.",
                                fqn_cls, nome
                            ),
                            ));
                        } else if expected != args.len() {
                            erros.push(ErroCompilador::novo(
                                TipoErro::Semântico,
                                format!(
                                "Aridade genérica incorreta para '{}': esperados {}, recebidos {}.",
                                fqn_cls,
                                expected,
                                args.len()
                            ),
                            ));
                        }
                    }
                } else if is_iface {
                    if let Some(decl) = self.interfaces.get(&fqn_iface) {
                        let expected = decl.generic_params.len();
                        if expected == 0 {
                            erros.push(ErroCompilador::novo(
                                TipoErro::Semântico,
                                format!(
                                "Interface '{}' não é genérica, mas foi usada como '{}' com argumentos.",
                                fqn_iface, nome
                            )));
                        } else if expected != args.len() {
                            erros.push(ErroCompilador::novo(
                                TipoErro::Semântico,
                                format!(
                                "Aridade genérica incorreta para interface '{}': esperados {}, recebidos {}.",
                                fqn_iface, expected, args.len()
                            )));
                        }
                    }
                }

                let mut norm_args: Vec<Tipo> = Vec::new();
                for a in args.iter() {
                    let (na, mut e) = self.normalize_tipo_ro(a, namespace_atual);
                    norm_args.push(na);
                    erros.append(&mut e);
                }

                (
                    Aplicado {
                        nome: resolved_name,
                        args: norm_args,
                    },
                    erros,
                )
            }
            Funcao(params, ret) => {
                let mut erros = Vec::new();
                let mut norm_params: Vec<Tipo> = Vec::new();
                for p in params.iter() {
                    let (np, mut e) = self.normalize_tipo_ro(p, namespace_atual);
                    norm_params.push(np);
                    erros.append(&mut e);
                }
                let (nr, mut e2) = self.normalize_tipo_ro(ret, namespace_atual);
                erros.append(&mut e2);
                (Funcao(norm_params, Box::new(nr)), erros)
            }
            Opcional(inner) => {
                let (norm, errs) = self.normalize_tipo_ro(inner, namespace_atual);
                (Opcional(Box::new(norm)), { errs })
            }
            other => (other.clone(), vec![]),
        }
    }

}
