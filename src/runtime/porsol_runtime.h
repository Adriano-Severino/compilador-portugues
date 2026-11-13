/* Runtime nativo da biblioteca padrão para o alvo LLVM.
 *
 * Modelo: objetos da stdlib são handles opacos (i8*) gerenciados aqui.
 * Valores trocam entre o LLVM IR e o runtime via `porsol_value_t`, uma
 * união etiquetada equivalente ao `Valor` do interpretador de bytecode.
 *
 * Strings são UTF-8 NUL-terminated. Operações de caixa (maiúsculas/
 * minúsculas) são ASCII-only — limitação conhecida para acentos.
 */
#ifndef POR_DO_SOL_RUNTIME_H
#define POR_DO_SOL_RUNTIME_H

#ifdef _WIN32
#include <stdint.h>
typedef int8_t  i8;
typedef int32_t i32;
typedef int64_t i64;
typedef uint64_t u64;
typedef uint8_t  u8;
#else
#include <stdint.h>
#endif

#ifdef __cplusplus
extern "C" {
#endif

typedef enum PorsolKind {
    PORSOL_NULO = 0,
    PORSOL_INTEIRO = 1,
    PORSOL_DOUBLE = 2,
    PORSOL_BOOL = 3,
    PORSOL_TEXTO = 4,
    PORSOL_OBJETO = 5,
    PORSOL_ARRAY = 6
} PorsolKind;

typedef struct PorsolValue {
    int kind;          /* PorsolKind */
    int64_t i;         /* INTEIRO */
    double d;          /* DOUBLE */
    char *s;           /* TEXTO (UTF-8, NUL-terminated) */
    void *o;           /* OBJETO (porsol_obj_t*) ou ARRAY (porsol_array_hdr_t*) */
} PorsolValue;

/* Cabeçalho de array trocado com o LLVM. O backend espera o layout
 * { i32 len, i8* data } (%array); este cabeçalho estendido carrega o
 * tipo do elemento para que o runtime C possa interpretar data. */
typedef struct PorsolArrayHdr {
    int32_t len;
    int32_t elem_kind; /* PorsolKind do elemento */
    void *data;        /* elementos contíguos */
} PorsolArrayHdr;

/* Cria um objeto stdlib. `fqn` é o nome totalmente qualificado
 * (ex.: "Sistema.Colecoes.Lista"); inicializa o estado interno padrão. */
i8 *porsol_obj_new(const char *fqn);

/* Despacho único de nativos. `chave` segue a convenção "Classe::Metodo"
 * (ou chave explícita do atributo [Nativo]). Para métodos de instância,
 * args[0] é o handle do objeto (PORSOL_OBJETO). O resultado é escrito em
 * `out` (sempre inicializado, mesmo em erro — PORSOL_NULO). */
void porsol_native_call(PorsolValue *out, const char *chave, int argc,
                        const PorsolValue *args);

/* Utilitários expostos para o backend/ testes. */
char *porsol_strdup(const char *s);
int porsol_utf8_len(const char *s);

/* Cliente HTTP/1.1 síncrono (apenas http://). Retorna o corpo da resposta
 * alocado com malloc, ou NULL em caso de falha. Usado pelo async_runtime
 * para executar requisições em threads do pool. */
char *porsol_http_request(const char *metodo, const char *url,
                          const char *corpo);

/* Extrai o corpo de uma resposta HTTP/1.1 completa (cabeçalhos + corpo).
 * Usado por porsol_tls.c para respostas lidas via TLS. */
char *porsol_http_body_from_response(const char *resposta);

#ifdef __cplusplus
}
#endif

#endif /* POR_DO_SOL_RUNTIME_H */
