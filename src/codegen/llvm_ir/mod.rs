pub mod util;
pub mod tipos_e_structs;
pub mod declaracoes;
pub mod comandos;
pub mod expressoes;
pub mod async_gen;
pub mod conversoes;

use crate::ast;
use std::collections::{HashMap, HashSet};
use crate::library_loader::LibSimbolo;
use crate::type_checker;


/// O gerador de código para o alvo LLVM IR.
pub struct LlvmGenerator<'a> {
    programa: &'a ast::Programa,
    type_checker: &'a type_checker::VerificadorTipos<'a>,
    resolved_classes: &'a HashMap<String, type_checker::ResolvedClassInfo<'a>>,
    header: String,
    body: String,
    string_counter: usize,
    temp_counter: usize,
    /// Mantém o controle de variáveis locais e seus ponteiros de memória.
    variables: HashMap<String, (String, ast::Tipo)>,
    namespace_path: String,
    /// Mantém o controle da classe que está sendo processada no momento.
    classe_atual: Option<String>,
    // Mapa: FQN da classe -> lista ordenada de (nome_metodo, FQN_declarante)
    vtables: HashMap<String, Vec<(String, String)>>,
    // Índices rápidos: FQN -> (metodo -> índice)
    vtable_index: HashMap<String, HashMap<String, usize>>,
    // Instanciações genéricas coletadas no programa
    // classe_fqn -> lista de args (cada args é uma lista de Tipos já normalizados)
    applied_class_insts: HashMap<String, Vec<Vec<ast::Tipo>>>,
    // interface_fqn -> lista de args
    applied_iface_insts: HashMap<String, Vec<Vec<ast::Tipo>>>,
    /// Símbolos de trampolins que adaptam funções assíncronas da linguagem à
    /// assinatura `void* (void*)` exigida pelo runtime C.
    async_wrapper_symbols: HashMap<String, String>,
}

impl<'a> LlvmGenerator<'a> {
    pub fn new(
        programa: &'a ast::Programa,
        type_checker: &'a type_checker::VerificadorTipos<'a>,
        resolved_classes: &'a HashMap<String, type_checker::ResolvedClassInfo<'a>>,
    ) -> Self {
        Self {
            programa,
            type_checker,
            resolved_classes,
            header: String::new(),
            body: String::new(),
            string_counter: 0,
            temp_counter: 0,
            variables: HashMap::new(),
            namespace_path: String::new(),
            classe_atual: None,
            vtables: HashMap::new(),
            vtable_index: HashMap::new(),
            applied_class_insts: HashMap::new(),
            applied_iface_insts: HashMap::new(),
            async_wrapper_symbols: HashMap::new(),
        }
    }

    pub fn generate(&mut self) -> String {
        // Coleta instâncias genéricas (Aplicado) usadas no programa, antes de gerar tipos
        self.collect_applied_instantiations();
        self.prepare_header();
        self.declare_external_functions();
        // Constrói vtables antes de definir structs
        self.build_all_vtables();
        self.define_all_structs();
        // Define tipos para interfaces como structs mínimos para uso em assinaturas
        self.define_all_interface_structs();
        // Define tipos especializados para interfaces aplicadas (I<T> -> %class.I$T)
        self.define_all_applied_interface_structs();
        self.define_all_vtable_globals();
        self.define_static_globals();

        // Gera definições de funções e classes.
        for declaracao in &self.programa.declaracoes {
            match declaracao {
                ast::Declaracao::DeclaracaoFuncao(func) => {
                    self.generate_funcao(func, "");
                }
                ast::Declaracao::DeclaracaoClasse(class) => {
                    self.generate_classe_definitions(class, "");
                }
                _ => {}
            }
        }
        for ns in &self.programa.namespaces {
            self.generate_namespace_definitions(ns);
        }

        // Gera métodos para classes genéricas aplicadas (monomorfização)
        self.generate_applied_class_methods();

        // Gera a função `main`: executa comandos globais e, se existir, chama `Principal`.
        let mut old_body = self.body.clone();
        let old_vars = self.variables.clone();
        self.body = String::new();
        self.variables.clear();

        self.body.push_str("define i32 @main() {\n");
        self.body.push_str("entry:\n");

        // Comandos globais (top-level) no namespace raiz
        for decl in &self.programa.declaracoes {
            if let ast::Declaracao::Comando(cmd) = decl {
                self.generate_comando(cmd);
            }
        }

        // Se existir uma função Principal, chama-a ao final
        if let Some(fqn) = self.find_principal_function_fqn() {
            self.body
                .push_str(&format!("  call void @\"{}\"()\n", fqn.replace(".", "_")));
        }

        self.body.push_str("  ret i32 0\n");
        self.body.push_str("}\n");

        // Anexa e restaura
        old_body.push_str(&self.body);
        self.body = old_body;
        self.variables = old_vars;

        format!("{}{}", self.header, self.body)
    }

    pub fn generate_for_library(&mut self) -> String {
        // Coleta instâncias genéricas (Aplicado) usadas no programa
        self.collect_applied_instantiations();
        self.prepare_header();
        // Constrói vtables antes de definir structs
        self.build_all_vtables();
        self.define_all_structs();
        // Define tipos para interfaces como structs mínimas
        self.define_all_interface_structs();
        // Define tipos especializados para interfaces aplicadas
        self.define_all_applied_interface_structs();
        self.define_all_vtable_globals();
        self.define_static_globals();

        // Gera definições de funções e classes (mas SEM função main)
        for declaracao in &self.programa.declaracoes {
            match declaracao {
                ast::Declaracao::DeclaracaoFuncao(func) => {
                    self.generate_funcao(func, "");
                }
                ast::Declaracao::DeclaracaoClasse(class) => {
                    self.generate_classe_definitions(class, "");
                }
                _ => {}
            }
        }
        for ns in &self.programa.namespaces {
            self.generate_namespace_definitions(ns);
        }

        // Gera métodos para classes genéricas aplicadas (monomorfização)
        self.generate_applied_class_methods();

        format!("{}{}", self.header, self.body)
    }

    // Nome canônico e estável para tipos em mangling
    fn prepare_header(&mut self) {
        self.header
            .push_str("target triple = \"x86_64-pc-windows-msvc\"\n");
        self.header.push_str("declare i32 @printf(i8*, ...)\n");
        self.header.push_str("declare i32 @scanf(i8*, ...)\n");
        self.header.push_str("declare i8* @malloc(i64)\n");
        self.header
            .push_str("declare i32 @sprintf(i8*, i8*, ...)\n");
        self.header.push_str("declare i64 @strlen(i8*)\n");
        // ABI do runtime nativo de async/await. `task` permanece opaco para
        // que o layout com mutex/condition variable seja exclusivo do C.
        self.header.push_str("%task = type opaque\n");
        self.header.push_str("declare i32 @next_task_id()\n");
        self.header.push_str("declare %task* @task_create(i32)\n");
        self.header
            .push_str("declare void @task_submit_to_pool(%task*, i8* (i8*)*, i8*)\n");
        self.header.push_str("declare i8* @task_await(%task*)\n");
        self.header
            .push_str("declare %task* @task_create_read_file(i8*)\n");
        self.header
            .push_str("declare %task* @task_create_write_file(i8*, i8*)\n");
        self.header
            .push_str("declare %task* @task_create_file_exists(i8*)\n");
        self.header.push_str("declare void @llvm.memcpy.p0i8.p0i8.i64(i8* nocapture writeonly, i8* nocapture readonly, i64, i1 immarg)\n");
        self.header
            .push_str("declare void @llvm.memset.p0i8.i64(i8*, i8, i64, i1)\n");
        // Estrutura genérica de array: { i32 len, i8* data }
        self.header.push_str("%array = type { i32, i8* }\n");
        self.header.push_str(
            "@.println_fmt = private unnamed_addr constant [4 x i8] c\"%s\\0A\\00\", align 1\n",
        );
        self.header
            .push_str("@.int_fmt = private unnamed_addr constant [3 x i8] c\"%d\\00\", align 1\n");
        self.header.push_str(
            "@.float_fmt = private unnamed_addr constant [3 x i8] c\"%f\\00\", align 1\n",
        );
        self.header.push_str(
            "@.double_fmt = private unnamed_addr constant [3 x i8] c\"%f\\00\", align 1\n",
        );
        self.header
            .push_str("@.empty_str = private unnamed_addr constant [1 x i8] c\"\\00\", align 1\n");
        // Formato para ler uma linha inteira (até CR/LF), consumindo finais de linha
        // "%255[^\r\n]%*[\r\n]" em C; em IR usamos escapes hex: \0D (CR) e \0A (LF)
        self.header.push_str("@.scanline_fmt = private unnamed_addr constant [16 x i8] c\"%255[^\\0D\\0A]%*[\\0D\\0A]\\00\", align 1\n");
        self.header.push_str("@.oob_msg = private unnamed_addr constant [23 x i8] c\"Indice fora dos limites\", align 1\n");
    }

    fn declare_external_functions(&mut self) {
        if self.type_checker.biblioteca_externa.is_none() {
            return;
        }
        let ext_lib = self.type_checker.biblioteca_externa.as_ref().unwrap();
        let mut classes: Vec<_> = ext_lib
            .simbolos
            .values()
            .filter_map(|s| {
                if let crate::library_loader::LibSimbolo::Classe(c) = s {
                    Some(c)
                } else {
                    None
                }
            })
            .collect();
        classes.sort_by(|a, b| a.fqn.cmp(&b.fqn));

        let mut undefined_structs = std::collections::HashSet::new();

        for lib_classe in &classes {
            let fqn = &lib_classe.fqn;
            let sanitized = fqn.replace('.', "_");
            let struct_name = format!("%class.{}", sanitized);
            undefined_structs.insert(struct_name);

            for m in lib_classe.metodos.values() {
                let ret_llvm = self.map_string_to_llvm_type(&m.tipo_retorno);
                if ret_llvm.starts_with("%class.") {
                    undefined_structs.insert(ret_llvm.trim_end_matches('*').to_string());
                }
                for (p_tipo, _) in &m.parametros {
                    let p_llvm = self.map_string_to_llvm_type(p_tipo);
                    if p_llvm.starts_with("%class.") {
                        undefined_structs.insert(p_llvm.trim_end_matches('*').to_string());
                    }
                }
            }
        }

        let mut fqns_defined_locally = std::collections::HashSet::new();
        for fqn in self.resolved_classes.keys() {
            fqns_defined_locally.insert(format!("%class.{}", fqn.replace('.', "_")));
        }
        for (base_fqn, insts) in &self.applied_class_insts {
            for args in insts {
                let mangled = self.mangle_aplicado_name(base_fqn, args);
                fqns_defined_locally.insert(format!("%class.{}", mangled));
            }
        }

        let mut undefined_structs_vec: Vec<_> = undefined_structs.into_iter().collect();
        undefined_structs_vec.sort();
        for struct_name in undefined_structs_vec {
            if !fqns_defined_locally.contains(&struct_name) {
                self.header
                    .push_str(&format!("{} = type opaque\n", struct_name));
            }
        }

        for lib_classe in classes {
            let fqn = &lib_classe.fqn;
            let sanitized = fqn.replace('.', "_");
            let struct_name = format!("%class.{}", sanitized);
            let self_ptr_ty = format!("{}*", struct_name);

            let mut metodos: Vec<_> = lib_classe.metodos.values().collect();
            metodos.sort_by(|a, b| a.nome.cmp(&b.nome));

            for m in metodos {
                let fun_sym = format!("{}::{}", fqn, m.nome).replace('.', "_");
                let ret_llvm = self.map_string_to_llvm_type(&m.tipo_retorno);

                let mut params_llvm: Vec<String> = if m.eh_estatica {
                    vec![]
                } else {
                    vec![self_ptr_ty.clone()]
                };

                for (p_tipo, _p_nome) in &m.parametros {
                    params_llvm.push(self.map_string_to_llvm_type(p_tipo));
                }

                self.header.push_str(&format!(
                    "declare {0} @\"{1}\"({2})\n",
                    ret_llvm,
                    fun_sym,
                    params_llvm.join(", ")
                ));
            }
        }
    }

}
