use crate::ast::*;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

pub struct SistemaModulos {
    modulos: HashMap<String, Modulo>,
    dependencias: HashMap<String, Vec<String>>,
    namespaces: HashMap<String, String>,

}

#[derive(Debug, Clone)]
pub struct Modulo {
    pub nome: String,
    pub caminho: PathBuf,
    pub declaracoes: Vec<Declaracao>,
    pub importacoes: Vec<Importacao>,
    pub exportacoes: Vec<Exportacao>,
    pub dependencias: Vec<String>,
}

impl SistemaModulos {
    pub fn new() -> Self {
        Self {
            modulos: HashMap::new(),
            dependencias: HashMap::new(),
            namespaces: HashMap::new(),

        }
    }

    pub fn carregar_modulo(&mut self, caminho: &str) -> Result<String, String> {
        let caminho_path = PathBuf::from(caminho);
        let nome_modulo = caminho_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("anonimo")
            .to_string();

        if self.modulos.contains_key(&nome_modulo) {
            return Ok(nome_modulo); // Já carregado
        }

        let conteudo = std::fs::read_to_string(&caminho_path)
            .map_err(|e| format!("Erro ao ler módulo {}: {}", caminho, e))?;
        
        let modulo = self.parsear_modulo(&nome_modulo, caminho_path, &conteudo)?;
        
        // Verificar dependências circulares
        self.verificar_dependencias_circulares(&modulo)?;
        
        // Carregar dependências
        for dep in &modulo.dependencias {
            self.carregar_modulo(dep)?;
        }

        self.modulos.insert(nome_modulo.clone(), modulo);
        Ok(nome_modulo)
    }

    fn parsear_modulo(&self, nome: &str, caminho: PathBuf, _conteudo: &str) -> Result<Modulo, String> {
        // Por enquanto, criar um módulo vazio
        // Em implementação real, usaria o parser aqui
        Ok(Modulo {
            nome: nome.to_string(),
            caminho,
            declaracoes: Vec::new(),
            importacoes: Vec::new(),
            exportacoes: Vec::new(),
            dependencias: Vec::new(),
        })
    }

    fn verificar_dependencias_circulares(&self, modulo: &Modulo) -> Result<(), String> {
        let mut visitados = HashSet::new();
        let mut pilha = HashSet::new();
        
        self.dfs_dependencias(&modulo.nome, &mut visitados, &mut pilha)
    }

    fn dfs_dependencias(
        &self,
        modulo: &str,
        visitados: &mut HashSet<String>,
        pilha: &mut HashSet<String>,
    ) -> Result<(), String> {
        if pilha.contains(modulo) {
            return Err(format!("Dependência circular detectada envolvendo módulo '{}'", modulo));
        }

        if visitados.contains(modulo) {
            return Ok(());
        }

        pilha.insert(modulo.to_string());
        visitados.insert(modulo.to_string());

        if let Some(deps) = self.dependencias.get(modulo) {
            for dep in deps {
                self.dfs_dependencias(dep, visitados, pilha)?;
            }
        }

        pilha.remove(modulo);
        Ok(())
    }

    pub fn resolver_importacao(&self, importacao: &Importacao) -> Result<Vec<Declaracao>, String> {
        if let Some(modulo) = self.modulos.get(&importacao.caminho) {
            let mut declaracoes = Vec::new();
            
            if importacao.itens.is_empty() {
                // Importar tudo que é público
                for decl in &modulo.declaracoes {
                    if self.is_declaracao_publica(decl) {
                        declaracoes.push(decl.clone());
                    }
                }
            } else {
                // Importar itens específicos
                for item in &importacao.itens {
                    if let Some(decl) = self.encontrar_declaracao_por_nome(&modulo.declaracoes, item) {
                        if self.is_declaracao_publica(decl) {
                            declaracoes.push(decl.clone());
                        } else {
                            return Err(format!(
                                "Item '{}' não é público no módulo '{}'", 
                                item, importacao.caminho
                            ));
                        }
                    } else {
                        return Err(format!(
                            "Item '{}' não encontrado no módulo '{}'", 
                            item, importacao.caminho
                        ));
                    }
                }
            }
            
            Ok(declaracoes)
        } else {
            Err(format!("Módulo '{}' não encontrado", importacao.caminho))
        }
    }

    fn is_declaracao_publica(&self, declaracao: &Declaracao) -> bool {
        match declaracao {
            Declaracao::DeclaracaoClasse(classe) => {
                matches!(classe.modificador, ModificadorAcesso::Publico)
            },
            Declaracao::DeclaracaoFuncao(funcao) => {
                matches!(funcao.modificador, ModificadorAcesso::Publico)
            },
            Declaracao::Exportacao(exp) => exp.publico,
            _ => false,
        }
    }

   fn encontrar_declaracao_por_nome<'a>(&self, declaracoes: &'a [Declaracao], nome: &str) -> Option<&'a Declaracao> {
        declaracoes.iter().find(|decl| {
            match decl {
                Declaracao::DeclaracaoClasse(classe) => classe.nome == nome,
                Declaracao::DeclaracaoFuncao(funcao) => funcao.nome == nome,
                _ => false,
            }
        })
    }

    pub fn resolver_namespace(&self, caminho: &str) -> String {
        if let Some(namespace) = self.namespaces.get(caminho) {
            namespace.clone()
        } else {
            caminho.to_string()
        }
    }

    pub fn adicionar_namespace(&mut self, alias: String, caminho: String) {
        self.namespaces.insert(alias, caminho);
    }

    pub fn obter_modulo(&self, nome: &str) -> Option<&Modulo> {
        self.modulos.get(nome)
    }

    pub fn listar_modulos(&self) -> Vec<String> {
        self.modulos.keys().cloned().collect()
    }

    pub fn validar_exportacoes(&self, modulo: &str) -> Result<(), Vec<String>> {
        if let Some(mod_info) = self.modulos.get(modulo) {
            let mut erros = Vec::new();
            
            for exportacao in &mod_info.exportacoes {
                if !self.encontrar_declaracao_por_nome(&mod_info.declaracoes, &exportacao.nome).is_some() {
                    erros.push(format!(
                        "Exportação '{}' não corresponde a nenhuma declaração no módulo '{}'",
                        exportacao.nome, modulo
                    ));
                }
            }
            
            if erros.is_empty() {
                Ok(())
            } else {
                Err(erros)
            }
        } else {
            Err(vec![format!("Módulo '{}' não encontrado", modulo)])
        }
    }

    pub fn gerar_grafo_dependencias(&self) -> HashMap<String, Vec<String>> {
        self.dependencias.clone()
    }
}

// Utilitários para resolução de caminhos de módulos
pub struct ResolvedorCaminhos {
    caminhos_busca: Vec<PathBuf>,
}

impl ResolvedorCaminhos {
    pub fn new() -> Self {
        Self {
            caminhos_busca: vec![
                PathBuf::from("."),
                PathBuf::from("./modulos"),
                PathBuf::from("./lib"),
            ],
        }
    }

    pub fn adicionar_caminho(&mut self, caminho: PathBuf) {
        if !self.caminhos_busca.contains(&caminho) {
            self.caminhos_busca.push(caminho);
        }
    }

    pub fn resolver(&self, nome_modulo: &str) -> Option<PathBuf> {
        for caminho_base in &self.caminhos_busca {
            let caminho_completo = caminho_base.join(format!("{}.pr", nome_modulo));
            if caminho_completo.exists() {
                return Some(caminho_completo);
            }
        }
        None
    }
}