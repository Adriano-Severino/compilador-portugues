/* Glue TLS (mbedTLS) para o backend LLVM — cliente HTTPS.
 *
 * Expõe `porsol_https_request`, chamado por `porsol_http_request` em
 * porsol_runtime.c quando a URL usa https://. Requer que o executável
 * tenha sido linkado com os fontes de src/runtime/tls/mbedtls.
 */
#ifndef POR_DO_SOL_TLS_H
#define POR_DO_SOL_TLS_H

#ifdef __cplusplus
extern "C" {
#endif

/* CA bundle Mozilla embutido (porsol_cacert.c, gerado de cacert.pem). */
extern const unsigned char porsol_cacert_pem[];
extern const unsigned int porsol_cacert_pem_len;

/* Executa uma requisição HTTPS/1.1 síncrona. Retorna o corpo da resposta
 * alocado com malloc (mesmo contrato de porsol_http_request), ou NULL
 * em caso de falha (rede, handshake, certificado inválido). */
char *porsol_https_request(const char *metodo, const char *url,
                           const char *corpo);

#ifdef __cplusplus
}
#endif

#endif /* POR_DO_SOL_TLS_H */
