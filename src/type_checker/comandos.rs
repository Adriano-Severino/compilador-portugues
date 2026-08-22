use crate::ast;
use crate::ast::*;
use crate::error::{ErroCompilador, TipoErro};
use crate::library_loader::{self, LibSimbolo};
use std::collections::{HashMap, HashSet};
use super::ResolvedClassInfo;
use super::VerificadorTipos;

impl<'a> VerificadorTipos<'a> {
    pub(crate) fn verificar_comando(
        &mut self,
        comando: &Comando,
        namespace_atual: &str,
        classe_atual: Option<&String>,
        escopo_vars: &mut HashMap<String, Tipo>,
    ) {
        match comando {
            Comando::DeclaracaoVariavel(tipo, nome, expr) => {
                let (tipo_resolvido, mut errs) = self.normalize_tipo_ro(tipo, namespace_atual);
                self.erros.append(&mut errs);
                if let Some(e) = expr {
                    let tipo_expr =
                        self.inferir_tipo_expressao(e, namespace_atual, classe_atual, escopo_vars);
                    if tipo_expr != Tipo::Inferido
                        && !self.tipos_compativeis_atribuicao(&tipo_resolvido, &tipo_expr)
                    {
                        self.erros.push(ErroCompilador::novo(
                            TipoErro::Semântico,
                            format!(
                            "Tipo da expressão ({:?}) não corresponde ao tipo da variável \"{}\" ({:?}).",
                            tipo_expr, nome, tipo_resolvido
                        )));
                    }
                }
                escopo_vars.insert(nome.clone(), tipo_resolvido.clone());
            }
            Comando::AtribuirIndice(alvo, idx, valor) => {
                let t_alvo =
                    self.inferir_tipo_expressao(alvo, namespace_atual, classe_atual, escopo_vars);
                let t_idx =
                    self.inferir_tipo_expressao(idx, namespace_atual, classe_atual, escopo_vars);
                if t_idx != Tipo::Inteiro {
                    self.erros.push(ErroCompilador::novo(
                        TipoErro::Semântico,
                        "Índice de array deve ser inteiro".into(),
                    ));
                }
                if let Tipo::Lista(elem) = t_alvo {
                    let t_val = self.inferir_tipo_expressao(
                        valor,
                        namespace_atual,
                        classe_atual,
                        escopo_vars,
                    );
                    if !self.tipos_compativeis_atribuicao(&elem, &t_val) {
                        self.erros.push(ErroCompilador::novo(
                            TipoErro::Semântico,
                            format!(
                                "Atribuição de elemento incompatível: esperado {:?}, recebido {:?}",
                                elem, t_val
                            ),
                        ));
                    }
                } else {
                    self.erros.push(ErroCompilador::novo(
                        TipoErro::Semântico,
                        "Atribuição por índice requer alvo do tipo lista".into(),
                    ));
                }
            }
            Comando::AtribuirPropriedade(obj_expr, prop_nome, val_expr) => {
                let obj_tipo = self.inferir_tipo_expressao(
                    obj_expr,
                    namespace_atual,
                    classe_atual,
                    escopo_vars,
                );
                if let Tipo::Classe(nome_classe) = obj_tipo {
                    if let Some(class_info) = self.resolved_classes.get(&nome_classe) {
                        let prop = class_info.properties.iter().find(|p| p.nome == *prop_nome);
                        let prop_tipo = prop.map(|p| p.tipo.clone()).or_else(|| {
                            class_info
                                .fields
                                .iter()
                                .find(|f| f.nome == *prop_nome)
                                .map(|f| f.tipo.clone())
                        });

                        if let Some(p) = prop {
                            if p.definir.is_none() {
                                self.erros.push(ErroCompilador::novo(
                                    TipoErro::Semântico,
                                    format!(
                                    "A propriedade \"{}\" é somente leitura (read-only) e não pode ser atribuída.",
                                    prop_nome
                                )));
                            }
                        }

                        if let Some(p_tipo) = prop_tipo {
                            let val_tipo = self.inferir_tipo_expressao(
                                val_expr,
                                namespace_atual,
                                classe_atual,
                                escopo_vars,
                            );
                            if val_tipo != Tipo::Inferido
                                && !self.tipos_compativeis_atribuicao(&p_tipo, &val_tipo)
                            {
                                self.erros.push(ErroCompilador::novo(
                                    TipoErro::Semântico,
                                    format!(
                                    "Atribuição de tipo inválido para propriedade \"{}\". Esperado {:?}, recebido {:?}.",
                                    prop_nome, p_tipo, val_tipo
                                )));
                            }
                        } else {
                            self.erros.push(ErroCompilador::novo(
                                TipoErro::Semântico,
                                format!(
                                    "Propriedade \"{}\" não encontrada na classe \"{}\".",
                                    prop_nome, nome_classe
                                ),
                            ));
                        }
                    } else {
                        self.erros.push(ErroCompilador::novo(
                            TipoErro::Semântico,
                            format!(
                                "Classe \"{}\" não encontrada para atribuição de propriedade.",
                                nome_classe
                            ),
                        ));
                    }
                } else if obj_tipo == Tipo::Inferido || obj_tipo == Tipo::Objeto {
                    // Treat Inferido or Objeto as a dynamic object to avoid blocking compilation of library code
                    // that might have complex late-bound types.
                    let val_tipo = self.inferir_tipo_expressao(
                        val_expr,
                        namespace_atual,
                        classe_atual,
                        escopo_vars,
                    );
                } else {
                    self.erros.push(ErroCompilador::novo(
                        TipoErro::Semântico,
                        "Atribuição de propriedade em algo que não é um objeto.".to_string(),
                    ));
                }
            }
            Comando::Bloco(comandos) => {
                let mut bloco_vars = escopo_vars.clone();
                for cmd in comandos {
                    self.verificar_comando(cmd, namespace_atual, classe_atual, &mut bloco_vars);
                }
            }
            Comando::DeclaracaoVar(nome, expr) => {
                let tipo_expr =
                    self.inferir_tipo_expressao(expr, namespace_atual, classe_atual, escopo_vars);
                escopo_vars.insert(nome.clone(), tipo_expr);
            }
            Comando::Imprima(expr) => {
                self.inferir_tipo_expressao(expr, namespace_atual, classe_atual, escopo_vars);
            }
            Comando::Retorne(expr) => {
                if let Some(e) = expr {
                    self.inferir_tipo_expressao(e, namespace_atual, classe_atual, escopo_vars);
                }
            }
            Comando::Se(cond, corpo, senao) => {
                self.inferir_tipo_expressao(cond, namespace_atual, classe_atual, escopo_vars);
                self.verificar_comando(corpo, namespace_atual, classe_atual, escopo_vars);
                if let Some(s) = senao {
                    self.verificar_comando(s, namespace_atual, classe_atual, escopo_vars);
                }
            }
            Comando::Enquanto(cond, corpo) => {
                self.inferir_tipo_expressao(cond, namespace_atual, classe_atual, escopo_vars);
                self.verificar_comando(corpo, namespace_atual, classe_atual, escopo_vars);
            }
            Comando::Expressao(expr) => {
                self.inferir_tipo_expressao(expr, namespace_atual, classe_atual, escopo_vars);
            }
            Comando::Atribuicao(nome, expr) => {
                if let Some(class_name) = classe_atual {
                    if let Some(class_info) = self.resolved_classes.get(class_name) {
                        if class_info.properties.iter().any(|p| p.nome == *nome)
                            || class_info.fields.iter().any(|f| f.nome == *nome)
                        {
                            self.verificar_comando(
                                &Comando::AtribuirPropriedade(
                                    Box::new(Expressao::Este),
                                    nome.clone(),
                                    expr.clone(),
                                ),
                                namespace_atual,
                                classe_atual,
                                escopo_vars,
                            );
                            return;
                        }
                    }
                }
                let tipo_expr =
                    self.inferir_tipo_expressao(expr, namespace_atual, classe_atual, escopo_vars);
                if let Some(tipo_var) = escopo_vars.get(nome) {
                    if tipo_expr != Tipo::Inferido
                        && !self.tipos_compativeis_atribuicao(tipo_var, &tipo_expr)
                    {
                        self.erros.push(ErroCompilador::novo(
                            TipoErro::Semântico,
                            format!(
                            "Atribuição de tipo inválido para variável \"{}\". Esperado {:?}, recebido {:?}.",
                            nome, tipo_var, tipo_expr
                        )));
                    }
                } else {
                    self.erros.push(ErroCompilador::novo(
                        TipoErro::Semântico,
                        format!("Variável \"{}\" não declarada.", nome),
                    ));
                }
            }
            Comando::ChamarMetodo(obj_expr, _, args) => {
                // Verifica tipo do objeto e existência do método no tipo estático
                let obj_tipo = self.inferir_tipo_expressao(
                    obj_expr,
                    namespace_atual,
                    classe_atual,
                    escopo_vars,
                );
                for arg in args {
                    self.inferir_tipo_expressao(arg, namespace_atual, classe_atual, escopo_vars);
                }
                // Descobre o nome do método a partir do comando
                let metodo_nome = match comando {
                    Comando::ChamarMetodo(_, m, _) => m,
                    _ => unreachable!(),
                };

                match obj_tipo {
                    Tipo::Classe(ref nome) => {
                        // Pode ser interface ou classe
                        if self.interfaces.contains_key(nome) {
                            // Método deve existir na interface
                            if let Some(iface) = self.interfaces.get(nome) {
                                if !iface.metodos.iter().any(|s| &s.nome == metodo_nome) {
                                    self.erros.push(ErroCompilador::novo(
                                        TipoErro::Semântico,
                                        format!(
                                            "Método '{}' não existe na interface '{}'.",
                                            metodo_nome, nome
                                        ),
                                    ));
                                }
                            }
                        } else if let Some(class_info) = self.resolved_classes.get(nome) {
                            if !class_info.methods.contains_key(metodo_nome) {
                                // Pode existir em declaração bruta, mas resolved já inclui herdados
                                self.erros.push(ErroCompilador::novo(
                                    TipoErro::Semântico,
                                    format!(
                                        "Método '{}' não existe na classe '{}'.",
                                        metodo_nome, nome
                                    ),
                                ));
                            } else {
                                // Verificar modificador de acesso (semântica C#)
                                let metodo = class_info.methods.get(metodo_nome).unwrap();

                                let mut is_static_access = false;
                                if let Expressao::Identificador(nome_id) = obj_expr.as_ref() {
                                    if !escopo_vars.contains_key(nome_id) {
                                        // Verifica se não é um campo ou propriedade da classe atual (implicit this)
                                        let is_member = classe_atual
                                            .and_then(|c| self.resolved_classes.get(c))
                                            .map_or(false, |info| {
                                                info.properties.iter().any(|p| p.nome == *nome_id)
                                                    || info
                                                        .fields
                                                        .iter()
                                                        .any(|f| f.nome == *nome_id)
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
                                        metodo_nome, nome
                                    )));
                                } else if !metodo.eh_estatica && is_static_access {
                                    self.erros.push(ErroCompilador::novo(
                                        TipoErro::Semântico,
                                        format!(
                                        "O método '{}' de '{}' não é estático e não pode ser chamado diretamente pela classe.",
                                        metodo_nome, nome
                                    )));
                                }

                                let is_private = metodo.modificador == ModificadorAcesso::Privado;
                                let is_protected =
                                    metodo.modificador == ModificadorAcesso::Protegido;
                                let inside_same = classe_atual.map_or(false, |c| c == nome);
                                let inside_sub =
                                    classe_atual.map_or(false, |c| self.is_subclass_of(c, nome));

                                if is_private && !inside_same {
                                    self.erros.push(ErroCompilador::novo(
                                        TipoErro::Semântico,
                                        format!(
                                        "O método '{}' de '{}' é inacessível: é privado e só pode ser chamado dentro da própria classe.",
                                        metodo_nome, nome
                                    )));
                                } else if is_protected && !inside_same && !inside_sub {
                                    self.erros.push(ErroCompilador::novo(
                                        TipoErro::Semântico,
                                        format!(
                                        "O método '{}' de '{}' é inacessível: é protegido e só pode ser chamado dentro da classe ou de subclasses.",
                                        metodo_nome, nome
                                    )));
                                }
                            }
                        }
                    }
                    _ => {
                        // outros tipos por ora não têm métodos
                        self.erros.push(ErroCompilador::novo(
                            TipoErro::Semântico,
                            format!(
                                "Chamando método '{}' em tipo que não é objeto: {:?}",
                                metodo_nome, obj_tipo
                            ),
                        ));
                    }
                }
            }
            Comando::AcessarCampo(obj, _campo) => {
                let _obj_tipo = self.inferir_tipo_expressao(
                    &Expressao::Identificador(obj.clone()),
                    namespace_atual,
                    classe_atual,
                    escopo_vars,
                );
            }
            _ => {}
        }
    }

}
