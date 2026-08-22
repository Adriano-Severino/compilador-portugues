use crate::ast;
use crate::ast::*;
use crate::error::{ErroCompilador, TipoErro};
use crate::library_loader::{self, LibSimbolo};
use std::collections::{HashMap, HashSet};
use super::ResolvedClassInfo;
use super::VerificadorTipos;

impl<'a> VerificadorTipos<'a> {
    pub(crate) fn verificar_declaracao(
        &mut self,
        declaracao: &'a Declaracao,
        namespace_atual: &str,
        escopo_vars: &mut HashMap<String, Tipo>,
    ) {
        match declaracao {
            Declaracao::DeclaracaoClasse(classe) => {
                let params: std::collections::HashSet<String> =
                    classe.generic_params.iter().cloned().collect();
                self.generic_scope.push(params);

                let fqn = if namespace_atual.is_empty() {
                    classe.nome.clone()
                } else {
                    format!("{}.{}", namespace_atual, classe.nome)
                };
                // Regras de abstracao
                // 1) Nao pode haver metodo abstrato em classe nao-abstrata
                for m in &classe.metodos {
                    if m.eh_abstrato && !classe.eh_abstrata {
                        self.erros.push(ErroCompilador::novo(
                            TipoErro::Semântico,
                            format!(
                                "Método abstrato '{}' em classe não abstrata '{}'",
                                m.nome, fqn
                            ),
                        ));
                    }
                    // 2) método abstrato não pode ter corpo
                    if m.eh_abstrato && !m.corpo.is_empty() {
                        self.erros.push(ErroCompilador::novo(
                            TipoErro::Semântico,
                            format!(
                                "Método abstrato '{}' não pode ter corpo em '{}'",
                                m.nome, fqn
                            ),
                        ));
                    }
                    // 3) método abstrato não pode ser estático
                    if m.eh_abstrato && m.eh_estatica {
                        self.erros.push(ErroCompilador::novo(
                            TipoErro::Semântico,
                            format!(
                                "Método abstrato '{}' não pode ser estático em '{}'",
                                m.nome, fqn
                            ),
                        ));
                    }
                }
                // 4) Classe estática não pode ser abstrata (como em C#)
                if classe.eh_abstrata && classe.eh_estatica {
                    self.erros.push(ErroCompilador::novo(
                        TipoErro::Semântico,
                        format!(
                            "Classe '{}' não pode ser 'abstrata' e 'estática' ao mesmo tempo",
                            fqn
                        ),
                    ));
                }
                for metodo in &classe.metodos {
                    let is_nativo = metodo.attributes.iter().any(|a| a.name == "Nativo");
                    // Um método é "externo" quando tem corpo vazio, não é abstrato, e tem o atributo [Nativo]
                    // (parseado via palavra-chave `externo` no parser).
                    let is_externo = !metodo.eh_abstrato
                        && !is_nativo
                        && metodo.corpo.is_empty()
                        && !metodo.eh_virtual
                        && !metodo.eh_override;
                    let _ = is_externo; // será usado em validações futuras

                    if is_nativo && !metodo.corpo.is_empty() {
                        self.erros.push(ErroCompilador::novo(
                            TipoErro::Semântico,
                            format!(
                                "Método nativo '{}' não pode ter corpo em '{}'",
                                metodo.nome, fqn
                            ),
                        ));
                    }

                    let mut metodo_vars = escopo_vars.clone();
                    // Validação de override/virtual
                    if let Some(parent_simple) = &classe.classe_pai {
                        let base = match parent_simple {
                            Tipo::Classe(n) => n.as_str(),
                            Tipo::Aplicado { nome, .. } => nome.as_str(),
                            _ => "",
                        };
                        let parent_fqn = self.resolver_nome_classe(base, namespace_atual);
                        if metodo.eh_override {
                            if let Some(base_m) = self
                                .encontrar_metodo_na_base(Some(parent_fqn.clone()), &metodo.nome)
                            {
                                // Em C#, métodos abstratos são implicitamente virtuais (overridáveis)
                                if !(base_m.eh_virtual || base_m.eh_abstrato) {
                                    self.erros.push(ErroCompilador::novo(
                                        TipoErro::Semântico,
                                        format!(
                                        "Método '{}' em '{}' usa 'sobrescreve' mas o método da classe base não é 'redefinível'. Dica: marque o método da base como 'redefinível'.",
                                        metodo.nome, fqn
                                    )));
                                } else {
                                    let (ret_c, params_c) = self.assinatura_metodo(metodo);
                                    let (ret_b, params_b) = self.assinatura_metodo(base_m);
                                    if ret_c != ret_b || params_c != params_b {
                                        self.erros.push(ErroCompilador::novo(
                                            TipoErro::Semântico,
                                            format!(
                                            "Assinatura incompatível no override de '{}.{}'. Dica: a assinatura deve ser exatamente a mesma da base (retorno e parâmetros).",
                                            fqn, metodo.nome
                                        )));
                                    }
                                }
                            } else {
                                self.erros.push(ErroCompilador::novo(
                                    TipoErro::Semântico,
                                    format!(
                                    "Método '{}' marcado como 'sobrescreve' mas não existe método correspondente na classe base de '{}'. Dica: verifique nome, parâmetros e se o método da base está visível.",
                                    metodo.nome, fqn
                                )));
                            }
                        }
                    }
                    if let Some(return_type) = &metodo.tipo_retorno {
                        let (resolved_return_type, mut errs) =
                            self.normalize_tipo_ro(return_type, namespace_atual);
                        self.erros.append(&mut errs);
                        self.validar_tipo_conhecido(
                            &resolved_return_type,
                            namespace_atual,
                            format!("o tipo de retorno do método '{}'", metodo.nome),
                        );
                    }

                    let mut metodo_vars = escopo_vars.clone();
                    for param in &metodo.parametros {
                        let (resolved_param_type, mut e) =
                            self.normalize_tipo_ro(&param.tipo, namespace_atual);
                        self.erros.append(&mut e);
                        self.validar_tipo_conhecido(
                            &resolved_param_type,
                            namespace_atual,
                            format!("o parâmetro '{}' do método '{}'", param.nome, metodo.nome),
                        );
                        metodo_vars.insert(param.nome.clone(), resolved_param_type);
                    }
                    println!(
                        "DEBUG: Verificando método \"{}\". Parâmetros no escopo: {:?}",
                        metodo.nome, metodo_vars
                    );

                    let eh_stdlib = self.eh_classe_stdlib(&fqn);

                    if !metodo.eh_abstrato && !is_nativo && !eh_stdlib {
                        for comando in &metodo.corpo {
                            self.verificar_comando(
                                comando,
                                namespace_atual,
                                Some(&fqn),
                                &mut metodo_vars,
                            );
                        }
                    }
                }
                self.generic_scope.pop();
            }
            Declaracao::DeclaracaoFuncao(funcao) => {
                let is_nativo = funcao.attributes.iter().any(|a| a.name == "Nativo");

                if is_nativo && !funcao.corpo.is_empty() {
                    self.erros.push(ErroCompilador::novo(
                        TipoErro::Semântico,
                        format!("Função nativa '{}' não pode ter corpo.", funcao.nome),
                    ));
                }

                let mut func_vars = escopo_vars.clone();
                for param in &funcao.parametros {
                    func_vars.insert(param.nome.clone(), param.tipo.clone());
                }

                let eh_stdlib = self.stdlib_namespaces.contains(namespace_atual);

                if !is_nativo && !eh_stdlib {
                    for comando in &funcao.corpo {
                        self.verificar_comando(comando, namespace_atual, None, &mut func_vars);
                    }
                }
            }
            Declaracao::Comando(cmd) => {
                self.verificar_comando(cmd, namespace_atual, None, escopo_vars);
            }
            _ => {}
        }
    }

}
