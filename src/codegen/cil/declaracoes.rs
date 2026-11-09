// src/codegen/cil/declaracoes.rs

use crate::ast;
use super::tipos::{sanitizar_identificador, tipo_para_cil};
use super::CilGenerator;

impl<'a> CilGenerator<'a> {
    /// Gera a declaração de uma classe em CIL.
    pub fn generate_classe(&mut self, cl: &ast::DeclaracaoClasse, code: &mut String) {
        let fqn = if self.namespace_path.is_empty() {
            cl.nome.clone()
        } else {
            format!("{}.{}", self.namespace_path, cl.nome)
        };

        let old_classe = self.classe_atual.clone();
        self.classe_atual = Some(fqn.clone());

        self.generic_params_atuais = cl.generic_params.clone();
        let class_cil_name = if cl.generic_params.is_empty() {
            sanitizar_identificador(&fqn)
        } else {
            let sanitized = sanitizar_identificador(&fqn);
            format!("'{}`{}'<{}>", sanitized, cl.generic_params.len(), cl.generic_params.join(", "))
        };
        let mut class_header = String::new();

        let visibilidade = match cl.modificador {
            ast::ModificadorAcesso::Publico => "public",
            ast::ModificadorAcesso::Privado => "private",
            ast::ModificadorAcesso::Protegido => "family",
        };

        let abstrata_estatica = if cl.eh_abstrata {
            "abstract "
        } else if cl.eh_estatica {
            "abstract sealed "
        } else {
            ""
        };

        let resolved_info = self.type_checker.resolved_classes.get(&fqn);

        let classe_pai_cil = if let Some(ref pai) = cl.classe_pai {
            match pai {
                ast::Tipo::Classe(ref n) => {
                    let pai_fqn = self.type_checker.resolver_nome_classe(n, &self.namespace_path);
                    if pai_fqn == "objeto" || pai_fqn == "object" || pai_fqn == "System.Object" || self.type_checker.interfaces.contains_key(&pai_fqn) {
                        "extends [System.Runtime]System.Object".to_string()
                    } else {
                        format!("extends {}", sanitizar_identificador(&pai_fqn))
                    }
                }
                ast::Tipo::Aplicado { ref nome, ref args } => {
                    let iface_fqn = self.type_checker.resolver_nome_interface(nome, &self.namespace_path);
                    if self.type_checker.interfaces.contains_key(&iface_fqn) {
                        "extends [System.Runtime]System.Object".to_string()
                    } else {
                        let pai_fqn = self.type_checker.resolver_nome_classe(nome, &self.namespace_path);
                        let args_cil: Vec<String> = args.iter().map(|a| tipo_para_cil(&self.resolver_tipo(a))).collect();
                        format!("extends class '{}`{}'<{}>", sanitizar_identificador(&pai_fqn), args.len(), args_cil.join(", "))
                    }
                }
                _ => "extends [System.Runtime]System.Object".to_string(),
            }
        } else if let Some(ci) = resolved_info {
            if let Some(ref pai) = ci.parent_name {
                if pai == "objeto" || pai == "object" || pai == "System.Object" {
                    "extends [System.Runtime]System.Object".to_string()
                } else {
                    format!("extends {}", sanitizar_identificador(pai))
                }
            } else {
                "extends [System.Runtime]System.Object".to_string()
            }
        } else {
            "extends [System.Runtime]System.Object".to_string()
        };

        let mut all_ifaces: Vec<String> = Vec::new();
        for iface in &cl.interfaces {
            match iface {
                ast::Tipo::Classe(ref n) => {
                    let iface_fqn = self.type_checker.resolver_nome_interface(n, &self.namespace_path);
                    all_ifaces.push(sanitizar_identificador(&iface_fqn));
                }
                ast::Tipo::Aplicado { ref nome, ref args } => {
                    let iface_fqn = self.type_checker.resolver_nome_interface(nome, &self.namespace_path);
                    let args_cil: Vec<String> = args.iter().map(|a| tipo_para_cil(&self.resolver_tipo(a))).collect();
                    all_ifaces.push(format!("class '{}`{}'<{}>", sanitizar_identificador(&iface_fqn), args.len(), args_cil.join(", ")));
                }
                _ => {}
            }
        }
        if let Some(ref pai) = cl.classe_pai {
            match pai {
                ast::Tipo::Classe(ref n) => {
                    let pai_fqn = self.type_checker.resolver_nome_interface(n, &self.namespace_path);
                    if self.type_checker.interfaces.contains_key(&pai_fqn) && !all_ifaces.contains(&sanitizar_identificador(&pai_fqn)) {
                        all_ifaces.push(sanitizar_identificador(&pai_fqn));
                    }
                }
                ast::Tipo::Aplicado { ref nome, ref args } => {
                    let pai_fqn = self.type_checker.resolver_nome_interface(nome, &self.namespace_path);
                    if self.type_checker.interfaces.contains_key(&pai_fqn) {
                        let args_cil: Vec<String> = args.iter().map(|a| tipo_para_cil(&self.resolver_tipo(a))).collect();
                        let iface_str = format!("class '{}`{}'<{}>", sanitizar_identificador(&pai_fqn), args.len(), args_cil.join(", "));
                        if !all_ifaces.contains(&iface_str) {
                            all_ifaces.push(iface_str);
                        }
                    }
                }
                _ => {}
            }
        }

        let mut implements_clause = String::new();
        if !all_ifaces.is_empty() {
            implements_clause = format!(" implements {}", all_ifaces.join(", "));
        }

        class_header.push_str(&format!(
            ".class {} auto ansi beforefieldinit {}{}\n  {}\n{{\n",
            visibilidade,
            abstrata_estatica,
            class_cil_name,
            format!("{} {}", classe_pai_cil, implements_clause).trim()
        ));

        code.push_str(&class_header);

        // 1. Campos
        for campo in &cl.campos {
            let campo_vis = match campo.modificador {
                ast::ModificadorAcesso::Publico => "public",
                ast::ModificadorAcesso::Privado => "private",
                ast::ModificadorAcesso::Protegido => "family",
            };
            let static_kw = if campo.eh_estatica { "static " } else { "" };
            let tipo_res = self.resolver_tipo(&campo.tipo);
            let tipo_cil = tipo_para_cil(&tipo_res);
            code.push_str(&format!(
                "  .field {} {}{} {}\n",
                campo_vis,
                static_kw,
                tipo_cil,
                sanitizar_identificador(&campo.nome)
            ));
        }

        // 2. Propriedades
        for prop in &cl.propriedades {
            let tipo_res = self.resolver_tipo(&prop.tipo);
            let tipo_cil = tipo_para_cil(&tipo_res);
            let prop_cil = sanitizar_identificador(&prop.nome);
            let backing_field = format!("'_{}_k__BackingField'", prop.nome);

            let tem_obter_customizado = prop.obter.as_ref().and_then(|o| o.corpo.as_ref()).is_some();
            let tem_definir_customizado = prop.definir.as_ref().and_then(|d| d.corpo.as_ref()).is_some();
            let precisa_backing_field = !tem_obter_customizado || !tem_definir_customizado;

            if precisa_backing_field {
                code.push_str(&format!("  .field private {} {}\n", tipo_cil, backing_field));
            }

            // Definição da propriedade
            code.push_str(&format!(
                "  .property instance {} {}()\n  {{\n    .get instance {} {}::get_{}()\n    .set instance void {}::set_{}({})\n  }}\n",
                tipo_cil,
                prop_cil,
                tipo_cil,
                class_cil_name,
                prop_cil,
                class_cil_name,
                prop_cil,
                tipo_cil
            ));

            // Getter
            if let Some(corpo) = prop.obter.as_ref().and_then(|o| o.corpo.as_ref()) {
                self.limpar_contexto_metodo(prop.eh_estatica);
                let mut getter_body = String::new();
                for cmd in corpo {
                    self.generate_comando(cmd, &mut getter_body);
                }
                if !getter_body.trim_end().ends_with("ret") {
                    getter_body.push_str("    ret\n");
                }
                code.push_str(&format!(
                    "  .method public hidebysig specialname instance {} get_{}() cil managed\n  {{\n    .maxstack 64\n",
                    tipo_cil,
                    prop_cil
                ));
                self.emitir_locals(code);
                code.push_str(&getter_body);
                code.push_str("  }\n");
            } else {
                code.push_str(&format!(
                    "  .method public hidebysig specialname instance {} get_{}() cil managed\n  {{\n    .maxstack 64\n    ldarg.0\n    ldfld {} {}::{}\n    ret\n  }}\n",
                    tipo_cil,
                    prop_cil,
                    tipo_cil,
                    class_cil_name,
                    backing_field
                ));
            }

            // Setter
            if let Some(corpo) = prop.definir.as_ref().and_then(|d| d.corpo.as_ref()) {
                self.limpar_contexto_metodo(prop.eh_estatica);
                self.registrar_parametro("value", prop.tipo.clone());
                self.registrar_parametro("valor", prop.tipo.clone());
                let mut setter_body = String::new();
                for cmd in corpo {
                    self.generate_comando(cmd, &mut setter_body);
                }
                if !setter_body.trim_end().ends_with("ret") {
                    setter_body.push_str("    ret\n");
                }
                code.push_str(&format!(
                    "  .method public hidebysig specialname instance void set_{}({} 'value') cil managed\n  {{\n    .maxstack 64\n",
                    prop_cil,
                    tipo_cil
                ));
                self.emitir_locals(code);
                code.push_str(&setter_body);
                code.push_str("  }\n");
            } else {
                code.push_str(&format!(
                    "  .method public hidebysig specialname instance void set_{}({} 'value') cil managed\n  {{\n    .maxstack 64\n    ldarg.0\n    ldarg.1\n    stfld {} {}::{}\n    ret\n  }}\n",
                    prop_cil,
                    tipo_cil,
                    tipo_cil,
                    class_cil_name,
                    backing_field
                ));
            }
        }

        // 3. Construtores
        if cl.construtores.is_empty() && !cl.eh_estatica {
            // Emite construtor padrão chamando a base
            let base_ctor = if let Some(ref pai) = cl.classe_pai {
                match pai {
                    ast::Tipo::Classe(ref n) => {
                        let pai_fqn = self.type_checker.resolver_nome_classe(n, &self.namespace_path);
                        if pai_fqn == "objeto" || pai_fqn == "object" || pai_fqn == "System.Object" || self.type_checker.interfaces.contains_key(&pai_fqn) {
                            "[System.Runtime]System.Object::.ctor()".to_string()
                        } else {
                            format!("{}::.ctor()", sanitizar_identificador(&pai_fqn))
                        }
                    }
                    ast::Tipo::Aplicado { ref nome, ref args } => {
                        let iface_fqn = self.type_checker.resolver_nome_interface(nome, &self.namespace_path);
                        if self.type_checker.interfaces.contains_key(&iface_fqn) {
                            "[System.Runtime]System.Object::.ctor()".to_string()
                        } else {
                            let pai_fqn = self.type_checker.resolver_nome_classe(nome, &self.namespace_path);
                            let args_cil: Vec<String> = args.iter().map(|a| tipo_para_cil(&self.resolver_tipo(a))).collect();
                            format!("class '{}`{}'<{}>::.ctor()", sanitizar_identificador(&pai_fqn), args.len(), args_cil.join(", "))
                        }
                    }
                    _ => "[System.Runtime]System.Object::.ctor()".to_string(),
                }
            } else if let Some(ci) = self.type_checker.resolved_classes.get(&fqn) {
                if let Some(ref pai) = ci.parent_name {
                    if pai == "objeto" || pai == "object" || pai == "System.Object" {
                        "[System.Runtime]System.Object::.ctor()".to_string()
                    } else {
                        format!("{}::.ctor()", sanitizar_identificador(pai))
                    }
                } else {
                    "[System.Runtime]System.Object::.ctor()".to_string()
                }
            } else {
                "[System.Runtime]System.Object::.ctor()".to_string()
            };

            code.push_str(&format!(
                "  .method public hidebysig specialname rtspecialname instance void .ctor() cil managed\n  {{\n    .maxstack 8\n    ldarg.0\n    call instance void {}\n    ret\n  }}\n",
                base_ctor
            ));
        } else {
            for ctor in &cl.construtores {
                self.generate_construtor(ctor, &class_cil_name, cl, code);
            }
        }

        // 4. Métodos
        for metodo in &cl.metodos {
            self.generate_metodo(metodo, &class_cil_name, code);
        }

        code.push_str("}\n\n");
        self.generic_params_atuais.clear();
        self.classe_atual = old_classe;
    }

    /// Gera o código CIL de um construtor de classe.
    fn generate_construtor(
        &mut self,
        ctor: &ast::ConstrutorClasse,
        class_cil_name: &str,
        cl: &ast::DeclaracaoClasse,
        code: &mut String,
    ) {
        self.limpar_contexto_metodo(false);

        let mut params_cil = Vec::new();
        for param in &ctor.parametros {
            let tipo_res = self.resolver_tipo(&param.tipo);
            let t = tipo_para_cil(&tipo_res);
            let n = sanitizar_identificador(&param.nome);
            params_cil.push(format!("{} {}", t, n));
            self.registrar_parametro(&param.nome, tipo_res);
        }

        let mut body_code = String::new();

        // 1. Chamada ao construtor pai
        let mut tipos_args_pai_cil = Vec::new();
        body_code.push_str("    ldarg.0\n");
        if let Some(ref args_pai) = ctor.chamada_pai {
            for arg in args_pai {
                let t = self.generate_expressao(arg, &mut body_code);
                tipos_args_pai_cil.push(tipo_para_cil(&self.resolver_tipo(&t)));
            }
        }
        let args_pai_sig = tipos_args_pai_cil.join(", ");

        let fqn = self.classe_atual.clone().unwrap_or_default();
        let base_class_name = if let Some(ref pai) = cl.classe_pai {
            match pai {
                ast::Tipo::Classe(ref n) => {
                    let pai_fqn = self.type_checker.resolver_nome_classe(n, &self.namespace_path);
                    if pai_fqn == "objeto" || pai_fqn == "object" || pai_fqn == "System.Object" || self.type_checker.interfaces.contains_key(&pai_fqn) {
                        "[System.Runtime]System.Object".to_string()
                    } else {
                        sanitizar_identificador(&pai_fqn)
                    }
                }
                ast::Tipo::Aplicado { ref nome, ref args } => {
                    let iface_fqn = self.type_checker.resolver_nome_interface(nome, &self.namespace_path);
                    if self.type_checker.interfaces.contains_key(&iface_fqn) {
                        "[System.Runtime]System.Object".to_string()
                    } else {
                        let pai_fqn = self.type_checker.resolver_nome_classe(nome, &self.namespace_path);
                        let args_cil: Vec<String> = args.iter().map(|a| tipo_para_cil(&self.resolver_tipo(a))).collect();
                        format!("class '{}`{}'<{}>", sanitizar_identificador(&pai_fqn), args.len(), args_cil.join(", "))
                    }
                }
                _ => "[System.Runtime]System.Object".to_string(),
            }
        } else if let Some(ci) = self.type_checker.resolved_classes.get(&fqn) {
            if let Some(ref pai) = ci.parent_name {
                if pai == "objeto" || pai == "object" || pai == "System.Object" {
                    "[System.Runtime]System.Object".to_string()
                } else {
                    sanitizar_identificador(pai)
                }
            } else {
                "[System.Runtime]System.Object".to_string()
            }
        } else {
            "[System.Runtime]System.Object".to_string()
        };

        body_code.push_str(&format!("    call instance void {}::.ctor({})\n", base_class_name, args_pai_sig));

        // 2. Inicialização de campos com valor padrão
        for campo in &cl.campos {
            if let Some(ref init_expr) = campo.valor_inicial {
                if !campo.eh_estatica {
                    body_code.push_str("    ldarg.0\n");
                    let _ = self.generate_expressao(init_expr, &mut body_code);
                    let tipo_cil = tipo_para_cil(&campo.tipo);
                    body_code.push_str(&format!(
                        "    stfld {} {}::{}\n",
                        tipo_cil,
                        class_cil_name,
                        sanitizar_identificador(&campo.nome)
                    ));
                }
            }
        }

        // 3. Corpo do construtor
        for cmd in &ctor.corpo {
            self.generate_comando(cmd, &mut body_code);
        }

        if !body_code.trim_end().ends_with("ret") {
            body_code.push_str("    ret\n");
        }

        code.push_str(&format!(
            "  .method public hidebysig specialname rtspecialname instance void .ctor({}) cil managed\n  {{\n    .maxstack 64\n",
            params_cil.join(", ")
        ));

        self.emitir_locals(code);
        code.push_str(&body_code);
        code.push_str("  }\n");
    }

    /// Gera a declaração de um método dentro de uma classe.
    fn generate_metodo(&mut self, metodo: &ast::MetodoClasse, _class_cil_name: &str, code: &mut String) {
        if metodo.eh_abstrato {
            let vis = match metodo.modificador {
                ast::ModificadorAcesso::Publico => "public",
                ast::ModificadorAcesso::Privado => "private",
                ast::ModificadorAcesso::Protegido => "family",
            };
            let tipo_retorno = metodo.tipo_retorno.as_ref().map(tipo_para_cil).unwrap_or_else(|| "void".to_string());
            let params_cil: Vec<String> = metodo.parametros.iter().map(|p| format!("{} {}", tipo_para_cil(&p.tipo), sanitizar_identificador(&p.nome))).collect();

            code.push_str(&format!(
                "  .method {} hidebysig newslot abstract virtual instance {} {}({}) cil managed {{}}\n",
                vis,
                tipo_retorno,
                sanitizar_identificador(&metodo.nome),
                params_cil.join(", ")
            ));
            return;
        }

        self.limpar_contexto_metodo(metodo.eh_estatica);

        let vis = match metodo.modificador {
            ast::ModificadorAcesso::Publico => "public",
            ast::ModificadorAcesso::Privado => "private",
            ast::ModificadorAcesso::Protegido => "family",
        };

        let fqn = self.classe_atual.clone().unwrap_or_default();
        let implementa_interface_metodo = if let Some(ci) = self.type_checker.resolved_classes.get(&fqn) {
            ci.interfaces.iter().any(|iface_fqn| {
                if let Some(iface) = self.type_checker.interfaces.get(iface_fqn) {
                    iface.metodos.iter().any(|m| m.nome == metodo.nome)
                } else {
                    false
                }
            })
        } else {
            false
        };

        let virtual_flags = if metodo.eh_virtual || implementa_interface_metodo {
            "newslot virtual "
        } else if metodo.eh_override {
            "virtual "
        } else {
            ""
        };

        let static_or_instance = if metodo.eh_estatica {
            "static "
        } else {
            "instance "
        };

        let tipo_retorno = metodo.tipo_retorno.as_ref()
            .map(|t| tipo_para_cil(&self.resolver_tipo(t)))
            .unwrap_or_else(|| "void".to_string());

        let mut params_cil = Vec::new();
        for param in &metodo.parametros {
            let tipo_res = self.resolver_tipo(&param.tipo);
            let t = tipo_para_cil(&tipo_res);
            let n = sanitizar_identificador(&param.nome);
            params_cil.push(format!("{} {}", t, n));
            self.registrar_parametro(&param.nome, tipo_res);
        }

        let mut body_code = String::new();
        for cmd in &metodo.corpo {
            self.generate_comando(cmd, &mut body_code);
        }

        if !body_code.trim_end().ends_with("ret") {
            body_code.push_str("    ret\n");
        }

        code.push_str(&format!(
            "  .method {} hidebysig {}{}{} {}({}) cil managed\n  {{\n    .maxstack 64\n",
            vis,
            virtual_flags,
            static_or_instance,
            tipo_retorno,
            sanitizar_identificador(&metodo.nome),
            params_cil.join(", ")
        ));

        self.emitir_locals(code);
        code.push_str(&body_code);
        code.push_str("  }\n");
    }

    /// Gera uma interface em CIL.
    pub fn generate_interface(&mut self, iface: &ast::DeclaracaoInterface, code: &mut String) {
        self.generic_params_atuais = iface.generic_params.clone();
        let fqn = if self.namespace_path.is_empty() {
            iface.nome.clone()
        } else {
            format!("{}.{}", self.namespace_path, iface.nome)
        };

        let iface_cil = if iface.generic_params.is_empty() {
            sanitizar_identificador(&fqn)
        } else {
            let sanitized = sanitizar_identificador(&fqn);
            format!("'{}`{}'<{}>", sanitized, iface.generic_params.len(), iface.generic_params.join(", "))
        };
        code.push_str(&format!(
            ".class interface public abstract auto ansi {}\n{{\n",
            iface_cil
        ));

        for metodo in &iface.metodos {
            let tipo_retorno = metodo.tipo_retorno.as_ref().map(|t| tipo_para_cil(&self.resolver_tipo(t))).unwrap_or_else(|| "void".to_string());
            let params_cil: Vec<String> = metodo.parametros.iter().map(|p| format!("{} {}", tipo_para_cil(&self.resolver_tipo(&p.tipo)), sanitizar_identificador(&p.nome))).collect();

            code.push_str(&format!(
                "  .method public hidebysig newslot abstract virtual instance {} {}({}) cil managed {{}}\n",
                tipo_retorno,
                sanitizar_identificador(&metodo.nome),
                params_cil.join(", ")
            ));
        }

        code.push_str("}\n\n");
        self.generic_params_atuais.clear();
    }

    /// Gera um enum em CIL.
    pub fn generate_enum(&mut self, en: &ast::DeclaracaoEnum, code: &mut String) {
        let fqn = if self.namespace_path.is_empty() {
            en.nome.clone()
        } else {
            format!("{}.{}", self.namespace_path, en.nome)
        };

        let enum_cil = sanitizar_identificador(&fqn);
        code.push_str(&format!(
            ".class public auto ansi sealed {} extends [System.Runtime]System.Enum\n{{\n",
            enum_cil
        ));
        code.push_str("  .field public specialname rtspecialname int32 value__\n");

        for (i, val) in en.valores.iter().enumerate() {
            code.push_str(&format!(
                "  .field public static literal valuetype {} {} = int32({})\n",
                enum_cil,
                sanitizar_identificador(val),
                i
            ));
        }

        code.push_str("}\n\n");
    }

    /// Emite a seção `.locals init (...)` se houver variáveis locais registradas.
    pub(crate) fn emitir_locals(&self, code: &mut String) {
        if self.local_vars.is_empty() {
            return;
        }

        code.push_str("    .locals init (\n");
        let count = self.local_vars.len();
        for (i, (nome, tipo)) in self.local_vars.iter().enumerate() {
            let virgula = if i + 1 < count { "," } else { "" };
            let t = tipo_para_cil(tipo);
            let n = sanitizar_identificador(nome);
            code.push_str(&format!("      [{}] {} {}{}\n", i, t, n, virgula));
        }
        code.push_str("    )\n");
    }
}
