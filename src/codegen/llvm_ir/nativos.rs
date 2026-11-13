use crate::ast;
use crate::library_loader::{LibClasse, LibMetodo, LibSimbolo};

use super::LlvmGenerator;

/* PorsolKind — deve permanecer idêntico a porsol_runtime.h */
const P_NULO: i32 = 0;
const P_INTEIRO: i32 = 1;
const P_DOUBLE: i32 = 2;
const P_BOOL: i32 = 3;
const P_TEXTO: i32 = 4;
const P_OBJETO: i32 = 5;
const P_ARRAY: i32 = 6;

impl<'a> LlvmGenerator<'a> {
    /* ------------------------------------------------------------------ */
    /* Detecção de tipos da biblioteca padrão                              */
    /* ------------------------------------------------------------------ */

    pub(crate) fn lib_classe_by_fqn(&self, fqn: &str) -> Option<&'a LibClasse> {
        let bib = self.type_checker.biblioteca_externa.as_ref()?;
        match bib.simbolos.get(fqn) {
            Some(LibSimbolo::Classe(c)) => Some(c),
            _ => None,
        }
    }

    /// Resolve um nome (simples, FQN ou mangled "FQN$args") para o FQN de uma
    /// classe da biblioteca padrão. Retorna None se não for stdlib.
    pub(crate) fn resolve_stdlib_base(&self, nome: &str) -> Option<String> {
        let base = nome.split('$').next().unwrap_or(nome);
        if base.is_empty() {
            return None;
        }
        if self.lib_classe_by_fqn(base).is_some() {
            return Some(base.to_string());
        }
        let fqn = self
            .type_checker
            .resolver_nome_classe(base, &self.namespace_path);
        if self.lib_classe_by_fqn(&fqn).is_some() {
            return Some(fqn);
        }
        // Fallback: casa por nome simples ou sufixo de FQN. O último caso
        // cobre FQNs resolvidos erroneamente pelo type checker (ex.:
        // "Sistema.Colecoes.Data" quando a classe real é "Sistema.Data.Data").
        let bib = self.type_checker.biblioteca_externa.as_ref()?;
        let simple = base.rsplit('.').next().unwrap_or(base);
        for simbolo in bib.simbolos.values() {
            if let LibSimbolo::Classe(c) = simbolo {
                if c.fqn == base
                    || c.fqn.ends_with(&format!(".{}", base))
                    || c.fqn.ends_with(&format!(".{}", simple))
                    || c.nome == base
                    || c.nome == simple
                {
                    return Some(c.fqn.clone());
                }
            }
        }
        None
    }

    /// Se `tipo` é uma classe da stdlib, retorna (fqn_base, args_genéricos).
    pub(crate) fn stdlib_tipo_info(&self, tipo: &ast::Tipo) -> Option<(String, Vec<ast::Tipo>)> {
        match tipo {
            ast::Tipo::Classe(n) => self.resolve_stdlib_base(n).map(|f| (f, Vec::new())),
            ast::Tipo::Aplicado { nome, args } => {
                let f = self.resolve_stdlib_base(nome)?;
                let recv_args = if args.is_empty() {
                    self.type_args_from_mangled(nome)
                } else {
                    args.iter()
                        .map(|a| self.resolve_type(a, &self.namespace_path))
                        .collect()
                };
                Some((f, recv_args))
            }
            _ => None,
        }
    }

    /// Recupera argumentos genéricos do sufixo mangled ("FQN$int_texto").
    fn type_args_from_mangled(&self, mangled: &str) -> Vec<ast::Tipo> {
        let Some((_, suffix)) = mangled.split_once('$') else {
            return Vec::new();
        };
        if suffix.is_empty() {
            return Vec::new();
        }
        fn prim(t: &str) -> Option<ast::Tipo> {
            Some(match t {
                "int" => ast::Tipo::Inteiro,
                "texto" => ast::Tipo::Texto,
                "bool" => ast::Tipo::Booleano,
                "f32" => ast::Tipo::Flutuante,
                "f64" => ast::Tipo::Duplo,
                "dec" => ast::Tipo::Decimal,
                "vazio" => ast::Tipo::Vazio,
                _ => return None,
            })
        }
        let tokens: Vec<&str> = suffix.split('_').collect();
        let mut args = Vec::new();
        let mut i = 0;
        while i < tokens.len() {
            if let Some(t) = prim(tokens[i]) {
                args.push(t);
                i += 1;
            } else {
                // Tokens desconhecidos são unidos como nome de classe
                // (o mangling troca '.' por '_', então não há como separar
                // namespaces de argumentos múltiplos — aproximação aceita).
                let mut j = i;
                while j < tokens.len() && prim(tokens[j]).is_none() {
                    j += 1;
                }
                args.push(ast::Tipo::Classe(tokens[i..j].join("_")));
                i = j;
            }
        }
        args
    }

    /// Chave de despacho: usa a chave explícita do [Nativo] quando existe,
    /// senão "<ClasseSimples>::<Metodo>".
    fn native_key(&self, lib: &LibClasse, m: &LibMetodo) -> String {
        if let Some(k) = &m.chave_nativa {
            return k.clone();
        }
        let simples = lib.fqn.rsplit('.').next().unwrap_or(&lib.fqn);
        format!("{}::{}", simples, m.nome)
    }

    /// Converte a string de tipo do manifesto .pbl em ast::Tipo, aplicando
    /// substituição de parâmetros genéricos pelos args do receptor.
    pub(crate) fn stdlib_tipo_from_str(
        &self,
        s: &str,
        class_fqn: &str,
        recv_args: &[ast::Tipo],
    ) -> ast::Tipo {
        let simples = class_fqn.rsplit('.').next().unwrap_or(class_fqn);
        let params: &[&str] = match simples {
            "Dicionario" => &["TChave", "TValor"],
            _ => &["T"],
        };
        for (i, p) in params.iter().enumerate() {
            if s == *p {
                if let Some(a) = recv_args.get(i) {
                    return a.clone();
                }
                return ast::Tipo::Objeto;
            }
        }
        let s = s.trim();
        match s {
            "inteiro" | "i32" => ast::Tipo::Inteiro,
            "texto" => ast::Tipo::Texto,
            "booleano" => ast::Tipo::Booleano,
            "flutuante" => ast::Tipo::Flutuante,
            "duplo" => ast::Tipo::Duplo,
            "decimal" => ast::Tipo::Decimal,
            "vazio" => ast::Tipo::Vazio,
            "objeto" => ast::Tipo::Objeto,
            _ => {
                // O manifesto usa "lista<T>" (minúsculo) para arrays.
                if let Some(inner) = s
                    .strip_prefix("lista<")
                    .and_then(|x| x.strip_suffix('>'))
                {
                    return ast::Tipo::Lista(Box::new(
                        self.stdlib_tipo_from_str(inner, class_fqn, recv_args),
                    ));
                }
                if let Some(elem) = s.strip_suffix("[]") {
                    return ast::Tipo::Lista(Box::new(
                        self.stdlib_tipo_from_str(elem, class_fqn, recv_args),
                    ));
                }
                // Remove argumentos genéricos do nome ("Lista<inteiro>").
                let base = s.split('<').next().unwrap_or(s);
                if let Some(f) = self.resolve_stdlib_base(base) {
                    return ast::Tipo::Classe(f);
                }
                let fqn = self
                    .type_checker
                    .resolver_nome_classe(base, &self.namespace_path);
                ast::Tipo::Classe(fqn)
            }
        }
    }

    /* ------------------------------------------------------------------ */
    /* Interceptação de chamadas                                           */
    /* ------------------------------------------------------------------ */

    /// Chamadas estáticas: `Classe.Metodo(args)` onde Classe é stdlib.
    /// Retorna None se `obj_expr` não for identificador de classe stdlib.
    pub(crate) fn try_stdlib_static_call(
        &mut self,
        obj_expr: &ast::Expressao,
        metodo_nome: &str,
        argumentos: &[ast::Expressao],
    ) -> Option<(String, ast::Tipo)> {
        let ast::Expressao::Identificador(nome) = obj_expr else {
            return None;
        };
        let fqn = self.resolve_stdlib_base(nome)?;
        let lib = self.lib_classe_by_fqn(&fqn)?;
        let m = lib.metodos.get(metodo_nome)?;
        if !(m.eh_estatica || lib.eh_estatica) {
            panic!(
                "'{}' é um método de instância da biblioteca padrão; chame-o sobre um objeto",
                metodo_nome
            );
        }
        if m.eh_assincrona {
            panic!(
                "'{}::{}' é assíncrono e requer 'aguarde'",
                fqn, metodo_nome
            );
        }
        let chave = self.native_key(lib, m);
        let mut values = Vec::new();
        for a in argumentos {
            values.push(self.generate_expressao(a));
        }
        let ret = self.stdlib_tipo_from_str(&m.tipo_retorno, &fqn, &[]);
        Some(self.emit_native_call(&chave, &values, &ret))
    }

    /// Chamadas de instância: `obj.Metodo(args)` onde obj é handle stdlib.
    /// `obj_reg`/`obj_tipo` já foram gerados pelo chamador.
    pub(crate) fn try_stdlib_instance_call(
        &mut self,
        obj_reg: &str,
        obj_tipo: &ast::Tipo,
        metodo_nome: &str,
        argumentos: &[ast::Expressao],
    ) -> Option<(String, ast::Tipo)> {
        let (fqn, recv_args) = self.stdlib_tipo_info(obj_tipo)?;
        let lib = self.lib_classe_by_fqn(&fqn)?;
        let m = lib.metodos.get(metodo_nome)?;
        if m.eh_assincrona {
            panic!(
                "'{}::{}' é assíncrono e requer 'aguarde'",
                fqn, metodo_nome
            );
        }
        let chave = self.native_key(lib, m);
        // Método estático chamado via instância: não passa self.
        let mut values: Vec<(String, ast::Tipo)> = if m.eh_estatica {
            Vec::new()
        } else {
            vec![(obj_reg.to_string(), ast::Tipo::Objeto)]
        };
        for a in argumentos {
            values.push(self.generate_expressao(a));
        }
        let ret = self.stdlib_tipo_from_str(&m.tipo_retorno, &fqn, &recv_args);
        Some(self.emit_native_call(&chave, &values, &ret))
    }

    /// Propriedade/campo de instância stdlib: `obj.Nome`.
    pub(crate) fn try_stdlib_property_get(
        &mut self,
        obj_reg: &str,
        obj_tipo: &ast::Tipo,
        membro_nome: &str,
    ) -> Option<(String, ast::Tipo)> {
        let (fqn, recv_args) = self.stdlib_tipo_info(obj_tipo)?;
        let lib = self.lib_classe_by_fqn(&fqn)?;
        let tipo_str = lib
            .propriedades
            .iter()
            .find(|p| p.nome == membro_nome)
            .map(|p| p.tipo.clone())
            .or_else(|| {
                lib.campos
                    .iter()
                    .find(|c| c.nome == membro_nome)
                    .map(|c| c.tipo.clone())
            })?;
        let ret = self.stdlib_tipo_from_str(&tipo_str, &fqn, &recv_args);
        let simples = fqn.rsplit('.').next().unwrap_or(&fqn);
        let chave = format!("{}::{}", simples, membro_nome);
        let values = vec![(obj_reg.to_string(), ast::Tipo::Objeto)];
        Some(self.emit_native_call(&chave, &values, &ret))
    }

    /// Atribuição de propriedade stdlib: `obj.Nome = valor`.
    /// Retorna true se o tipo for stdlib (o dispatch é emitido aqui).
    pub(crate) fn try_stdlib_property_set(
        &mut self,
        obj_reg: &str,
        obj_tipo: &ast::Tipo,
        prop_nome: &str,
        val_reg: &str,
        val_tipo: &ast::Tipo,
    ) -> bool {
        let Some((fqn, _)) = self.stdlib_tipo_info(obj_tipo) else {
            return false;
        };
        let simples = fqn.rsplit('.').next().unwrap_or(&fqn);
        let chave = format!("{}::set_{}", simples, prop_nome);
        let values = vec![
            (obj_reg.to_string(), ast::Tipo::Objeto),
            (val_reg.to_string(), val_tipo.clone()),
        ];
        self.emit_native_call(&chave, &values, &ast::Tipo::Vazio);
        true
    }

    /// `aguarde Classe.MetodoAssíncrono(args)` da stdlib. Mapeia para as
    /// tasks do async_runtime.c; DeResultado é avaliado imediatamente.
    /// `obj_reg` é o registrador do receptor para métodos de instância
    /// (ex.: `http.ObterAssincrono`); `None` para chamadas estáticas.
    pub(crate) fn await_stdlib_async(
        &mut self,
        class_fqn: &str,
        metodo: &str,
        arguments: &[ast::Expressao],
        obj_reg: Option<&str>,
    ) -> (String, ast::Tipo) {
        let simples = class_fqn.rsplit('.').next().unwrap_or(class_fqn);
        if simples == "Tarefa" && metodo == "DeResultado" {
            if arguments.len() != 1 {
                panic!("Tarefa::DeResultado requer 1 argumento");
            }
            return self.generate_expressao(&arguments[0]);
        }
        let key = format!("{}::{}", simples, metodo);
        // ClienteHttp.*Assincrono — método de instância executado como task.
        let http = match key.as_str() {
            "ClienteHttp::ObterAssincrono" | "ClienteHttp::GetAsync" => Some((0, false)),
            "ClienteHttp::EnviarAssincrono" | "ClienteHttp::PostAsync" => Some((1, true)),
            "ClienteHttp::AtualizarAssincrono" | "ClienteHttp::PutAsync" => Some((2, true)),
            "ClienteHttp::ExcluirAssincrono" | "ClienteHttp::DeleteAsync" => Some((3, false)),
            _ => None,
        };
        if let Some((codigo, tem_corpo)) = http {
            return self.await_http_async(&key, codigo, tem_corpo, obj_reg, arguments);
        }
        let (runtime_fn, ret) = match key.as_str() {
            "Arquivo::LerTextoAssincrono" | "Arquivo::LerTextoAssíncrono" => {
                ("task_create_read_file", ast::Tipo::Texto)
            }
            "Arquivo::EscreverTextoAssincrono" | "Arquivo::EscreverTextoAssíncrono" => {
                ("task_create_write_file", ast::Tipo::Vazio)
            }
            "Arquivo::AdicionarTextoAssincrono" | "Arquivo::AdicionarTextoAssíncrono" => {
                ("task_create_append_file", ast::Tipo::Vazio)
            }
            "Arquivo::ExisteAssincrono" | "Arquivo::ExisteAssíncrono" => {
                ("task_create_file_exists", ast::Tipo::Booleano)
            }
            "Tarefa::Atraso" | "Temporizador::Aguardar" => {
                ("task_create_sleep", ast::Tipo::Vazio)
            }
            _ => panic!(
                "Método assíncrono '{}' da biblioteca padrão não tem suporte no runtime LLVM",
                key
            ),
        };

        let task = self.get_unique_temp_name();
        if runtime_fn == "task_create_sleep" {
            if arguments.len() != 1 {
                panic!("{} requer 1 argumento", key);
            }
            let (v, t) = self.generate_expressao(&arguments[0]);
            let ms = match t {
                ast::Tipo::Inteiro | ast::Tipo::Enum(_) => {
                    let e = self.get_unique_temp_name();
                    self.body.push_str(&format!(
                        "  {} = sext i32 {} to i64\n",
                        e, v
                    ));
                    e
                }
                ast::Tipo::Duplo => {
                    let e = self.get_unique_temp_name();
                    self.body.push_str(&format!(
                        "  {} = fptosi double {} to i64\n",
                        e, v
                    ));
                    e
                }
                _ => panic!("'{}' requer argumento inteiro", key),
            };
            self.body.push_str(&format!(
                "  {} = call %task* @task_create_sleep(i64 {})\n",
                task, ms
            ));
        } else {
            let mut vals = Vec::new();
            for a in arguments {
                let (v, t) = self.generate_expressao(a);
                vals.push(self.ensure_string(&v, &t));
            }
            let sig = match vals.len() {
                1 => format!("i8* {}", vals[0]),
                2 => format!("i8* {}, i8* {}", vals[0], vals[1]),
                _ => panic!("'{}' requer 1 ou 2 argumentos de texto", key),
            };
            self.body.push_str(&format!(
                "  {} = call %task* @{}({})\n",
                task, runtime_fn, sig
            ));
        }
        self.await_task_result(&task, &ret)
    }

    /// `aguarde http.MetodoAssincrono(...)`: lê `BaseUrl` do objeto na
    /// thread principal e cria uma task HTTP no pool do async_runtime.
    /// `codigo`: 0=GET 1=POST 2=PUT 3=DELETE.
    fn await_http_async(
        &mut self,
        key: &str,
        codigo: i32,
        tem_corpo: bool,
        obj_reg: Option<&str>,
        arguments: &[ast::Expressao],
    ) -> (String, ast::Tipo) {
        let esperado = if tem_corpo { 2 } else { 1 };
        if arguments.len() != esperado {
            panic!("'{}' requer {} argumento(s), mas recebeu {}", key, esperado, arguments.len());
        }
        let Some(obj) = obj_reg else {
            panic!("'{}' requer uma instância de ClienteHttp", key);
        };
        // BaseUrl é lido aqui (thread principal) para não compartilhar o
        // PorsolObj com a thread do pool.
        let (base_reg, _) = self.emit_native_call(
            "ClienteHttp::BaseUrl",
            &[(obj.to_string(), ast::Tipo::Objeto)],
            &ast::Tipo::Texto,
        );
        let base = self.ensure_string(&base_reg, &ast::Tipo::Texto);
        let (url_reg, url_tipo) = self.generate_expressao(&arguments[0]);
        let url = self.ensure_string(&url_reg, &url_tipo);
        let corpo = if tem_corpo {
            let (corpo_reg, corpo_tipo) = self.generate_expressao(&arguments[1]);
            self.ensure_string(&corpo_reg, &corpo_tipo)
        } else {
            "null".to_string()
        };
        let task = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {} = call %task* @task_create_http(i32 {}, i8* {}, i8* {}, i8* {})\n",
            task, codigo, url, corpo, base
        ));
        self.await_task_result(&task, &ast::Tipo::Texto)
    }

    /// Cria objeto stdlib via `porsol_obj_new` e despacha o construtor.
    /// `ctor_args` são (reg, tipo) já avaliados; vazio = sem chamada de ctor.
    pub(crate) fn stdlib_obj_new(
        &mut self,
        fqn: &str,
        ctor_args: &[(String, ast::Tipo)],
    ) -> String {
        let key = self.create_global_string(fqn);
        let obj = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {} = call i8* @porsol_obj_new(i8* {})\n",
            obj, key
        ));
        if !ctor_args.is_empty() {
            let mut values = vec![(obj.clone(), ast::Tipo::Objeto)];
            values.extend(ctor_args.iter().cloned());
            let simples = fqn.rsplit('.').next().unwrap_or(fqn);
            self.emit_native_call(
                &format!("{}::construtor", simples),
                &values,
                &ast::Tipo::Vazio,
            );
        }
        obj
    }

    /* ------------------------------------------------------------------ */
    /* Emissão da chamada nativa                                           */
    /* ------------------------------------------------------------------ */

    fn porsol_kind(ty: &ast::Tipo) -> i32 {
        match ty {
            ast::Tipo::Inteiro | ast::Tipo::Enum(_) => P_INTEIRO,
            ast::Tipo::Duplo | ast::Tipo::Flutuante => P_DOUBLE,
            ast::Tipo::Booleano => P_BOOL,
            ast::Tipo::Texto | ast::Tipo::Decimal => P_TEXTO,
            ast::Tipo::Lista(_) => P_ARRAY,
            ast::Tipo::Classe(_)
            | ast::Tipo::Aplicado { .. }
            | ast::Tipo::Objeto
            | ast::Tipo::Generico(_) => P_OBJETO,
            _ => P_NULO,
        }
    }

    fn porsol_elem_kind(elem: &ast::Tipo) -> i32 {
        Self::porsol_kind(elem)
    }

    /// Emite `porsol_native_call` com os valores embrulhados em
    /// `%porsol.value` e desembrulha o resultado conforme `ret_tipo`.
    pub(crate) fn emit_native_call(
        &mut self,
        chave: &str,
        values: &[(String, ast::Tipo)],
        ret_tipo: &ast::Tipo,
    ) -> (String, ast::Tipo) {
        // alloca precisa de ao menos 1 elemento; argc reflete o real.
        let n_alloc = values.len().max(1);
        let n_args = values.len();
        let arr_ty = format!("[{} x %porsol.value]", n_alloc);
        let arr = self.get_unique_temp_name();
        self.body
            .push_str(&format!("  {} = alloca {}, align 8\n", arr, arr_ty));

        // Zera o array de argumentos (campos não usados ficam nulos).
        let sz_ptr = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {} = getelementptr %porsol.value, %porsol.value* null, i32 {}\n",
            sz_ptr, n_alloc
        ));
        let sz = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {} = ptrtoint %porsol.value* {} to i64\n",
            sz, sz_ptr
        ));
        let arr_i8 = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {} = bitcast {}* {} to i8*\n",
            arr_i8, arr_ty, arr
        ));
        self.body.push_str(&format!(
            "  call void @llvm.memset.p0i8.i64(i8* {}, i8 0, i64 {}, i1 false)\n",
            arr_i8, sz
        ));

        for (i, (reg, ty)) in values.iter().enumerate() {
            let slot = self.get_unique_temp_name();
            self.body.push_str(&format!(
                "  {} = getelementptr {}, {}* {}, i32 0, i32 {}\n",
                slot, arr_ty, arr_ty, arr, i
            ));
            self.emit_porsol_store(&slot, reg, ty);
        }

        let outv = self.get_unique_temp_name();
        self.body
            .push_str(&format!("  {} = alloca %porsol.value, align 8\n", outv));
        let key = self.create_global_string(chave);
        self.body.push_str(&format!(
            "  call void @porsol_native_call(%porsol.value* {}, i8* {}, i32 {}, {}* {})\n",
            outv, key, n_args, arr_ty, arr
        ));

        (self.unwrap_porsol(&outv, ret_tipo), ret_tipo.clone())
    }

    /// Escreve kind + payload de um argumento dentro do slot %porsol.value.
    fn emit_porsol_store(&mut self, slot: &str, reg: &str, ty: &ast::Tipo) {
        let kind = Self::porsol_kind(ty);
        let kf = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {} = getelementptr %porsol.value, %porsol.value* {}, i32 0, i32 0\n",
            kf, slot
        ));
        self.body
            .push_str(&format!("  store i32 {}, i32* {}\n", kind, kf));

        match kind {
            P_INTEIRO => {
                let ext = self.get_unique_temp_name();
                self.body.push_str(&format!(
                    "  {} = sext i32 {} to i64\n",
                    ext, reg
                ));
                let f = self.get_unique_temp_name();
                self.body.push_str(&format!(
                    "  {} = getelementptr %porsol.value, %porsol.value* {}, i32 0, i32 1\n",
                    f, slot
                ));
                self.body
                    .push_str(&format!("  store i64 {}, i64* {}\n", ext, f));
            }
            P_BOOL => {
                let ext = self.get_unique_temp_name();
                self.body.push_str(&format!(
                    "  {} = zext i1 {} to i64\n",
                    ext, reg
                ));
                let f = self.get_unique_temp_name();
                self.body.push_str(&format!(
                    "  {} = getelementptr %porsol.value, %porsol.value* {}, i32 0, i32 1\n",
                    f, slot
                ));
                self.body
                    .push_str(&format!("  store i64 {}, i64* {}\n", ext, f));
            }
            P_DOUBLE => {
                // Flutuante (float) é promovido a double; Duplo já é double.
                let val = if matches!(ty, ast::Tipo::Flutuante) {
                    let t = self.get_unique_temp_name();
                    self.body.push_str(&format!(
                        "  {} = fpext float {} to double\n",
                        t, reg
                    ));
                    t
                } else {
                    reg.to_string()
                };
                let f = self.get_unique_temp_name();
                self.body.push_str(&format!(
                    "  {} = getelementptr %porsol.value, %porsol.value* {}, i32 0, i32 2\n",
                    f, slot
                ));
                self.body
                    .push_str(&format!("  store double {}, double* {}\n", val, f));
            }
            P_TEXTO => {
                let f = self.get_unique_temp_name();
                self.body.push_str(&format!(
                    "  {} = getelementptr %porsol.value, %porsol.value* {}, i32 0, i32 3\n",
                    f, slot
                ));
                self.body
                    .push_str(&format!("  store i8* {}, i8** {}\n", reg, f));
            }
            P_ARRAY => {
                if let ast::Tipo::Lista(elem) = ty {
                    let hdr = self.emit_array_hdr(reg, elem);
                    let f = self.get_unique_temp_name();
                    self.body.push_str(&format!(
                        "  {} = getelementptr %porsol.value, %porsol.value* {}, i32 0, i32 4\n",
                        f, slot
                    ));
                    self.body
                        .push_str(&format!("  store i8* {}, i8** {}\n", hdr, f));
                }
            }
            P_OBJETO => {
                // reg pode ser %class.X* ou i8*; normaliza para i8*.
                let as_i8 = if self.map_type_to_llvm_arg(ty) == "i8*" {
                    reg.to_string()
                } else {
                    let t = self.get_unique_temp_name();
                    self.body.push_str(&format!(
                        "  {} = bitcast {} {} to i8*\n",
                        t,
                        self.map_type_to_llvm_arg(ty),
                        reg
                    ));
                    t
                };
                let f = self.get_unique_temp_name();
                self.body.push_str(&format!(
                    "  {} = getelementptr %porsol.value, %porsol.value* {}, i32 0, i32 4\n",
                    f, slot
                ));
                self.body
                    .push_str(&format!("  store i8* {}, i8** {}\n", as_i8, f));
            }
            _ => {}
        }
    }

    /// Constrói um PorsolArrayHdr ({i32 len, i32 kind, i8* data}) na stack a
    /// partir de um %array* do LLVM e retorna i8* para o campo .o.
    fn emit_array_hdr(&mut self, arr_reg: &str, elem: &ast::Tipo) -> String {
        let ah = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {} = alloca %porsol.arr, align 8\n",
            ah
        ));
        let (data, len) = self.get_array_data_and_len(arr_reg);

        let f0 = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {} = getelementptr %porsol.arr, %porsol.arr* {}, i32 0, i32 0\n",
            f0, ah
        ));
        self.body
            .push_str(&format!("  store i32 {}, i32* {}\n", len, f0));

        let f1 = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {} = getelementptr %porsol.arr, %porsol.arr* {}, i32 0, i32 1\n",
            f1, ah
        ));
        self.body.push_str(&format!(
            "  store i32 {}, i32* {}\n",
            Self::porsol_elem_kind(elem),
            f1
        ));

        let f2 = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {} = getelementptr %porsol.arr, %porsol.arr* {}, i32 0, i32 2\n",
            f2, ah
        ));
        self.body
            .push_str(&format!("  store i8* {}, i8** {}\n", data, f2));

        let p = self.get_unique_temp_name();
        self.body.push_str(&format!(
            "  {} = bitcast %porsol.arr* {} to i8*\n",
            p, ah
        ));
        p
    }

    /// Desembrulha o %porsol.value de saída no tipo LLVM esperado.
    fn unwrap_porsol(&mut self, out: &str, ret: &ast::Tipo) -> String {
        if matches!(ret, ast::Tipo::Vazio) {
            return String::new();
        }
        let field = |this: &mut Self, idx: i32| -> String {
            let f = this.get_unique_temp_name();
            this.body.push_str(&format!(
                "  {} = getelementptr %porsol.value, %porsol.value* {}, i32 0, i32 {}\n",
                f, out, idx
            ));
            f
        };
        match ret {
            ast::Tipo::Inteiro | ast::Tipo::Enum(_) => {
                let f = field(self, 1);
                let raw = self.get_unique_temp_name();
                self.body
                    .push_str(&format!("  {} = load i64, i64* {}\n", raw, f));
                let v = self.get_unique_temp_name();
                self.body.push_str(&format!(
                    "  {} = trunc i64 {} to i32\n",
                    v, raw
                ));
                v
            }
            ast::Tipo::Booleano => {
                let f = field(self, 1);
                let raw = self.get_unique_temp_name();
                self.body
                    .push_str(&format!("  {} = load i64, i64* {}\n", raw, f));
                let v = self.get_unique_temp_name();
                self.body.push_str(&format!(
                    "  {} = trunc i64 {} to i1\n",
                    v, raw
                ));
                v
            }
            ast::Tipo::Duplo => {
                let f = field(self, 2);
                let v = self.get_unique_temp_name();
                self.body
                    .push_str(&format!("  {} = load double, double* {}\n", v, f));
                v
            }
            ast::Tipo::Flutuante => {
                let f = field(self, 2);
                let raw = self.get_unique_temp_name();
                self.body
                    .push_str(&format!("  {} = load double, double* {}\n", raw, f));
                let v = self.get_unique_temp_name();
                self.body.push_str(&format!(
                    "  {} = fptrunc double {} to float\n",
                    v, raw
                ));
                v
            }
            ast::Tipo::Texto | ast::Tipo::Decimal => {
                let f = field(self, 3);
                let v = self.get_unique_temp_name();
                self.body
                    .push_str(&format!("  {} = load i8*, i8** {}\n", v, f));
                v
            }
            ast::Tipo::Lista(_) => {
                // O runtime devolve PorsolArrayHdr* cujo layout
                // {i32 len, i32 kind, i8* data} é compatível com %array
                // {i32 len, i8* data} nos offsets 0 e 8.
                let f = field(self, 4);
                let raw = self.get_unique_temp_name();
                self.body
                    .push_str(&format!("  {} = load i8*, i8** {}\n", raw, f));
                let v = self.get_unique_temp_name();
                self.body.push_str(&format!(
                    "  {} = bitcast i8* {} to %array*\n",
                    v, raw
                ));
                v
            }
            _ => {
                let f = field(self, 4);
                let raw = self.get_unique_temp_name();
                self.body
                    .push_str(&format!("  {} = load i8*, i8** {}\n", raw, f));
                let target = self.map_type_to_llvm_arg(ret);
                if target == "i8*" {
                    raw
                } else {
                    let v = self.get_unique_temp_name();
                    self.body.push_str(&format!(
                        "  {} = bitcast i8* {} to {}\n",
                        v, raw, target
                    ));
                    v
                }
            }
        }
    }
}
