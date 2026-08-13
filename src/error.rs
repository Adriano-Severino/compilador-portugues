use colored::Colorize;
use std::fmt;
use std::path::PathBuf;

/// Tipos de erro que podem ocorrer durante a compilação
#[derive(Debug, Clone, PartialEq)]
pub enum TipoErro {
    Léxico,
    Sintático,
    Semântico,
}

impl fmt::Display for TipoErro {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TipoErro::Léxico => write!(f, "Erro léxico"),
            TipoErro::Sintático => write!(f, "Erro sintático"),
            TipoErro::Semântico => write!(f, "Erro semântico"),
        }
    }
}

/// Estrutura rica para representar erros do compilador
#[derive(Debug, Clone)]
pub struct ErroCompilador {
    pub tipo: TipoErro,
    pub arquivo: Option<PathBuf>,
    pub linha: usize,
    pub coluna: usize,
    pub mensagem: String,
    pub codigo_contexto: Option<String>,
    pub highlight_inicio: usize,
    pub highlight_fim: usize,
    pub sugestoes: Vec<String>,
}

impl ErroCompilador {
    /// Cria um novo erro básico
    pub fn novo(tipo: TipoErro, mensagem: String) -> Self {
        Self {
            tipo,
            arquivo: None,
            linha: 0,
            coluna: 0,
            mensagem,
            codigo_contexto: None,
            highlight_inicio: 0,
            highlight_fim: 0,
            sugestoes: Vec::new(),
        }
    }

    /// Define o arquivo onde o erro ocorreu
    pub fn com_arquivo(mut self, arquivo: PathBuf) -> Self {
        self.arquivo = Some(arquivo);
        self
    }

    /// Define a localização (linha e coluna) do erro
    pub fn com_localizacao(mut self, linha: usize, coluna: usize) -> Self {
        self.linha = linha;
        self.coluna = coluna;
        self
    }

    /// Adiciona contexto do código fonte onde o erro ocorreu
    pub fn com_contexto(
        mut self,
        codigo: String,
        highlight_inicio: usize,
        highlight_fim: usize,
    ) -> Self {
        self.codigo_contexto = Some(codigo);
        self.highlight_inicio = highlight_inicio;
        self.highlight_fim = highlight_fim;
        self
    }

    /// Adiciona uma sugestão de correção
    pub fn com_sugestao(mut self, sugestao: String) -> Self {
        self.sugestoes.push(sugestao);
        self
    }

    /// Adiciona múltiplas sugestões de correção
    pub fn com_sugestoes(mut self, sugestoes: Vec<String>) -> Self {
        self.sugestoes.extend(sugestoes);
        self
    }

    /// Extrai o contexto do código fonte baseado na posição do erro
    pub fn extrair_contexto(codigo_fonte: &str, posicao: usize) -> (usize, usize, String, usize, usize) {
        let linhas: Vec<&str> = codigo_fonte.lines().collect();
        let mut posicao_atual = 0;

        for (idx, linha) in linhas.iter().enumerate() {
            let linha_inicio = posicao_atual;
            let linha_fim = posicao_atual + linha.len();

            if posicao >= linha_inicio && posicao <= linha_fim {
                let coluna = posicao - linha_inicio + 1;
                return (
                    idx + 1,           // linha (1-based)
                    coluna,            // coluna (1-based)
                    linha.to_string(), // contexto da linha
                    linha_inicio,      // início do highlight
                    linha_fim,         // fim do highlight
                );
            }

            posicao_atual = linha_fim + 1; // +1 para o newline
        }

        // Se não encontrou, retorna valores padrão
        (1, 1, String::new(), 0, 0)
    }

    /// Gera sugestões automáticas baseadas no tipo de erro e contexto
    pub fn gerar_sugestoes_automaticas(
        tipo: &TipoErro,
        token_encontrado: Option<&str>,
        tokens_esperados: &[&str],
    ) -> Vec<String> {
        match tipo {
            TipoErro::Sintático => {
                let mut sugestoes = Vec::new();

                if let Some(token) = token_encontrado {
                    if token == "vazio" {
                        sugestoes.push(
                            "Em declarações de função, use a sintaxe: `funcao TipoRetorno Nome()`"
                                .to_string(),
                        );
                        sugestoes.push(
                            "Se o retorno for vazio, use: `funcao vazio Nome()`".to_string(),
                        );
                        sugestoes.push(
                            "Verifique se 'vazio' está sendo usado corretamente como tipo de retorno."
                                .to_string(),
                        );
                    } else if token == ";" {
                        sugestoes.push("Verifique se não há ponto e vírgula em excesso.".to_string());
                    }
                }

                if !tokens_esperados.is_empty() {
                    let esperados_fmt = tokens_esperados.join(", ");
                    sugestoes.push(format!("Esperava um dos seguintes: {}", esperados_fmt));
                }

                sugestoes
            }
            TipoErro::Léxico => {
                vec![
                    "Verifique se há caracteres inválidos no código.".to_string(),
                    "Confirme que as strings estão corretamente fechadas com aspas.".to_string(),
                ]
            }
            TipoErro::Semântico => {
                vec![
                    "Verifique se os tipos são compatíveis.".to_string(),
                    "Confirme que as variáveis foram declaradas antes do uso.".to_string(),
                ]
            }
        }
    }

    /// Formata o erro estilo C#/Rust com cores e contexto
    pub fn formatar(&self) -> String {
        let mut output = String::new();

        // Cabeçalho do erro
        let tipo_formatado = self.tipo.to_string().red().bold();
        let arquivo_nome = self
            .arquivo
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("desconhecido");

        output.push_str(&format!(
            "{}: em '{}'\n",
            tipo_formatado, arquivo_nome
        ));

        // Localização
        if self.linha > 0 || self.coluna > 0 {
            output.push_str(&format!(
                "  --> {}:{}:{}\n",
                arquivo_nome, self.linha, self.coluna
            ));
        }

        // Contexto do código
        if let Some(ref codigo) = self.codigo_contexto {
            output.push_str("   |\n");
            output.push_str(&format!(" {} | {}\n", self.linha, codigo));
            output.push_str("   |");

            // Highlight
            if self.highlight_inicio > 0 || self.highlight_fim > 0 {
                let espacos = " ".repeat(self.coluna.saturating_sub(1));
                let carets = "^".repeat((self.highlight_fim - self.highlight_inicio).max(1));
                output.push_str(&format!(" {}{}\n", espacos, carets.red().bold()));
            } else {
                output.push('\n');
            }
        }

        // Mensagem principal
        output.push_str("   |\n");
        output.push_str(&format!("   = {}\n", self.mensagem.yellow()));

        // Sugestões
        if !self.sugestoes.is_empty() {
            output.push_str("   |\n");
            output.push_str("   = ajuda:\n");
            for sugestao in &self.sugestoes {
                output.push_str(&format!("           {}\n", sugestao.cyan()));
            }
        }

        output
    }
}

impl fmt::Display for ErroCompilador {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.formatar())
    }
}

impl std::error::Error for ErroCompilador {}

/// Converte erros do LALRPOP para ErroCompilador
pub fn de_lalrpop_error<E>(
    error: &lalrpop_util::ParseError<usize, crate::lexer::Token, E>,
    arquivo: PathBuf,
    codigo_fonte: &str,
) -> ErroCompilador
where
    E: std::fmt::Display,
{
    match error {
        lalrpop_util::ParseError::InvalidToken { location } => {
            let (linha, coluna, contexto, inicio, fim) =
                ErroCompilador::extrair_contexto(codigo_fonte, *location);

            ErroCompilador::novo(
                TipoErro::Léxico,
                "Token inválido encontrado".to_string(),
            )
            .com_arquivo(arquivo)
            .com_localizacao(linha, coluna)
            .com_contexto(contexto, inicio, fim)
            .com_sugestoes(ErroCompilador::gerar_sugestoes_automaticas(
                &TipoErro::Léxico,
                None,
                &[],
            ))
        }
        lalrpop_util::ParseError::UnrecognizedToken {
            token: (inicio, token, _fim),
            expected,
        } => {
            let (linha, coluna, contexto, ctx_inicio, ctx_fim) =
                ErroCompilador::extrair_contexto(codigo_fonte, *inicio);

            let token_str = format!("{:?}", token);
            let esperados: Vec<&str> = expected.iter().map(|s| s.as_str()).collect();

            ErroCompilador::novo(
                TipoErro::Sintático,
                format!("Token não reconhecido: {}", token_str),
            )
            .com_arquivo(arquivo)
            .com_localizacao(linha, coluna)
            .com_contexto(contexto, ctx_inicio, ctx_fim)
            .com_sugestoes(ErroCompilador::gerar_sugestoes_automaticas(
                &TipoErro::Sintático,
                Some("vazio"), // Simplificado - na prática extrairia do token
                &esperados,
            ))
        }
        lalrpop_util::ParseError::ExtraToken {
            token: (inicio, token, _fim),
        } => {
            let (linha, coluna, contexto, ctx_inicio, ctx_fim) =
                ErroCompilador::extrair_contexto(codigo_fonte, *inicio);

            ErroCompilador::novo(
                TipoErro::Sintático,
                format!("Token extra encontrado: {:?}", token),
            )
            .com_arquivo(arquivo)
            .com_localizacao(linha, coluna)
            .com_contexto(contexto, ctx_inicio, ctx_fim)
        }
        lalrpop_util::ParseError::UnrecognizedEof { location, expected } => {
            let (linha, coluna, contexto, inicio, fim) =
                ErroCompilador::extrair_contexto(codigo_fonte, *location);

            let esperados: Vec<&str> = expected.iter().map(|s| s.as_str()).collect();

            ErroCompilador::novo(
                TipoErro::Sintático,
                "Fim de arquivo inesperado".to_string(),
            )
            .com_arquivo(arquivo)
            .com_localizacao(linha, coluna)
            .com_contexto(contexto, inicio, fim)
            .com_sugestao(format!("Esperava: {}", esperados.join(", ")))
        }
        lalrpop_util::ParseError::User { error } => {
            ErroCompilador::novo(
                TipoErro::Sintático,
                format!("Erro do usuário: {}", error),
            )
            .com_arquivo(arquivo)
        }
    }
}

/// Versão específica para erros LALRPOP com erro unitário (no user error)
pub fn de_lalrpop_error_unit(
    error: &lalrpop_util::ParseError<usize, crate::lexer::Token, ()>,
    arquivo: PathBuf,
    codigo_fonte: &str,
) -> ErroCompilador {
    match error {
        lalrpop_util::ParseError::InvalidToken { location } => {
            let (linha, coluna, contexto, inicio, fim) =
                ErroCompilador::extrair_contexto(codigo_fonte, *location);

            ErroCompilador::novo(
                TipoErro::Léxico,
                "Token inválido encontrado".to_string(),
            )
            .com_arquivo(arquivo)
            .com_localizacao(linha, coluna)
            .com_contexto(contexto, inicio, fim)
            .com_sugestoes(ErroCompilador::gerar_sugestoes_automaticas(
                &TipoErro::Léxico,
                None,
                &[],
            ))
        }
        lalrpop_util::ParseError::UnrecognizedToken {
            token: (inicio, token, _fim),
            expected,
        } => {
            let (linha, coluna, contexto, ctx_inicio, ctx_fim) =
                ErroCompilador::extrair_contexto(codigo_fonte, *inicio);

            let token_str = format!("{:?}", token);
            let esperados: Vec<&str> = expected.iter().map(|s| s.as_str()).collect();

            ErroCompilador::novo(
                TipoErro::Sintático,
                format!("Token não reconhecido: {}", token_str),
            )
            .com_arquivo(arquivo)
            .com_localizacao(linha, coluna)
            .com_contexto(contexto, ctx_inicio, ctx_fim)
            .com_sugestoes(ErroCompilador::gerar_sugestoes_automaticas(
                &TipoErro::Sintático,
                Some("vazio"), // Simplificado - na prática extrairia do token
                &esperados,
            ))
        }
        lalrpop_util::ParseError::ExtraToken {
            token: (inicio, token, _fim),
        } => {
            let (linha, coluna, contexto, ctx_inicio, ctx_fim) =
                ErroCompilador::extrair_contexto(codigo_fonte, *inicio);

            ErroCompilador::novo(
                TipoErro::Sintático,
                format!("Token extra encontrado: {:?}", token),
            )
            .com_arquivo(arquivo)
            .com_localizacao(linha, coluna)
            .com_contexto(contexto, ctx_inicio, ctx_fim)
        }
        lalrpop_util::ParseError::UnrecognizedEof { location, expected } => {
            let (linha, coluna, contexto, inicio, fim) =
                ErroCompilador::extrair_contexto(codigo_fonte, *location);

            let esperados: Vec<&str> = expected.iter().map(|s| s.as_str()).collect();

            ErroCompilador::novo(
                TipoErro::Sintático,
                "Fim de arquivo inesperado".to_string(),
            )
            .com_arquivo(arquivo)
            .com_localizacao(linha, coluna)
            .com_contexto(contexto, inicio, fim)
            .com_sugestao(format!("Esperava: {}", esperados.join(", ")))
        }
        lalrpop_util::ParseError::User { .. } => {
            ErroCompilador::novo(
                TipoErro::Sintático,
                "Erro interno do parser".to_string(),
            )
            .com_arquivo(arquivo)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_criacao_erro_basico() {
        let erro = ErroCompilador::novo(
            TipoErro::Sintático,
            "Teste de erro".to_string(),
        );
        
        assert_eq!(erro.tipo, TipoErro::Sintático);
        assert_eq!(erro.mensagem, "Teste de erro");
    }

    #[test]
    fn test_extracao_contexto() {
        let codigo = "linha 1\nlinha 2\nlinha 3";
        let (linha, coluna, contexto, inicio, fim) = 
            ErroCompilador::extrair_contexto(codigo, 8); // posição na "linha 2"
        
        assert_eq!(linha, 2);
        assert!(contexto.contains("linha 2"));
    }

    #[test]
    fn test_sugestoes_automaticas() {
        let sugestoes = ErroCompilador::gerar_sugestoes_automaticas(
            &TipoErro::Sintático,
            Some("vazio"),
            &["identificador", "publico"],
        );
        
        assert!(!sugestoes.is_empty());
        assert!(sugestoes.iter().any(|s| s.contains("funcao")));
    }
}