/* Runtime nativo da biblioteca padrão para o alvo LLVM.
 *
 * Objetos da stdlib são handles opacos (i8*) gerenciados aqui.
 * `porsol_value_t` é a união etiquetada equivalente ao `Valor` do
 * interpretador de bytecode; `porsol_native_call` é o dispatcher único
 * das chaves "Classe::Metodo" (ou chave explícita de [Nativo]).
 *
 * Strings são UTF-8 NUL-terminated. Caixa alta/baixa é ASCII-only.
 * Não há GC: o processo é de vida curta; alocações são intencionais.
 */
#ifdef _WIN32
#define _CRT_SECURE_NO_WARNINGS
#endif

#include "porsol_runtime.h"

#include <ctype.h>
#include <math.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <time.h>

/* S_ISREG/S_ISDIR não existem no MSVC — equivalentes portáteis. */
#if defined(_WIN32) && !defined(S_ISREG)
#define S_ISREG(m) (((m) & _S_IFMT) == _S_IFREG)
#define S_ISDIR(m) (((m) & _S_IFMT) == _S_IFDIR)
#endif

#ifdef _WIN32
#include <winsock2.h>
#include <ws2tcpip.h>
#include <direct.h>
#include <io.h>
#include <windows.h>
#define PATH_SEP '\\'
#define socklen_t int
#else
#include <dirent.h>
#include <netdb.h>
#include <netinet/in.h>
#include <pthread.h>
#include <sys/socket.h>
#include <unistd.h>
#define PATH_SEP '/'
#endif

/* ================================================================== */
/* Utilitários básicos                                                 */
/* ================================================================== */

char *porsol_strdup(const char *s) {
    if (!s) return NULL;
    size_t n = strlen(s) + 1;
    char *p = (char *)malloc(n);
    if (p) memcpy(p, s, n);
    return p;
}

int porsol_utf8_len(const char *s) {
    int n = 0;
    for (const unsigned char *p = (const unsigned char *)s; p && *p; p++) {
        if ((*p & 0xC0) != 0x80) n++;
    }
    return n;
}

static char *porsol_strndup(const char *s, size_t n) {
    char *p = (char *)malloc(n + 1);
    if (!p) return NULL;
    memcpy(p, s, n);
    p[n] = '\0';
    return p;
}

static PorsolValue v_nulo(void) {
    PorsolValue v;
    memset(&v, 0, sizeof(v));
    v.kind = PORSOL_NULO;
    return v;
}

static PorsolValue v_int(int64_t i) {
    PorsolValue v = v_nulo();
    v.kind = PORSOL_INTEIRO;
    v.i = i;
    return v;
}

static PorsolValue v_dbl(double d) {
    PorsolValue v = v_nulo();
    v.kind = PORSOL_DOUBLE;
    v.d = d;
    return v;
}

static PorsolValue v_bool(int b) {
    PorsolValue v = v_nulo();
    v.kind = PORSOL_BOOL;
    v.i = b ? 1 : 0;
    return v;
}

static PorsolValue v_text(const char *s) {
    PorsolValue v = v_nulo();
    v.kind = PORSOL_TEXTO;
    v.s = porsol_strdup(s ? s : "");
    return v;
}

static PorsolValue v_text_move(char *s) {
    PorsolValue v = v_nulo();
    v.kind = PORSOL_TEXTO;
    v.s = s;
    return v;
}

static PorsolValue v_obj(void *o) {
    PorsolValue v = v_nulo();
    v.kind = PORSOL_OBJETO;
    v.o = o;
    return v;
}

static PorsolValue v_array(void *hdr) {
    PorsolValue v = v_nulo();
    v.kind = PORSOL_ARRAY;
    v.o = hdr;
    return v;
}

/* Converte qualquer valor para double (como numero_de do interpretador). */
static double numero_de(const PorsolValue *v) {
    if (!v) return 0.0;
    switch (v->kind) {
        case PORSOL_INTEIRO: return (double)v->i;
        case PORSOL_DOUBLE: return v->d;
        case PORSOL_BOOL: return v->i ? 1.0 : 0.0;
        default: return 0.0;
    }
}

/* Converte qualquer valor para texto (como texto_de do interpretador). */
static const char *texto_de_buf(const PorsolValue *v, char *buf, size_t bufsz) {
    if (!v) return "";
    switch (v->kind) {
        case PORSOL_TEXTO: return v->s ? v->s : "";
        case PORSOL_INTEIRO:
            snprintf(buf, bufsz, "%lld", (long long)v->i);
            return buf;
        case PORSOL_DOUBLE:
            snprintf(buf, bufsz, "%.6f", v->d);
            return buf;
        case PORSOL_BOOL: return v->i ? "verdadeiro" : "falso";
        case PORSOL_NULO: return "nulo";
        default: return "";
    }
}

static const char *texto_de(const PorsolValue *v) {
    static char buf[64];
    return texto_de_buf(v, buf, sizeof(buf));
}

/* Igualdade de valores (espelha Valor::eq do interpretador). */
static int valor_igual(const PorsolValue *a, const PorsolValue *b) {
    if (!a || !b) return a == b;
    if (a->kind != b->kind) {
        /* numéricos de tipos mistos são comparáveis */
        int na = (a->kind == PORSOL_INTEIRO || a->kind == PORSOL_DOUBLE || a->kind == PORSOL_BOOL);
        int nb = (b->kind == PORSOL_INTEIRO || b->kind == PORSOL_DOUBLE || b->kind == PORSOL_BOOL);
        if (na && nb) return numero_de(a) == numero_de(b);
        return 0;
    }
    switch (a->kind) {
        case PORSOL_NULO: return 1;
        case PORSOL_INTEIRO: return a->i == b->i;
        case PORSOL_DOUBLE: return a->d == b->d;
        case PORSOL_BOOL: return (a->i != 0) == (b->i != 0);
        case PORSOL_TEXTO:
            return strcmp(a->s ? a->s : "", b->s ? b->s : "") == 0;
        case PORSOL_OBJETO:
        case PORSOL_ARRAY: return a->o == b->o;
        default: return 0;
    }
}

/* Comparação para ordenação (espelha comparar_valores do interpretador). */
static int valor_cmp(const PorsolValue *a, const PorsolValue *b) {
    if (a->kind == PORSOL_TEXTO && b->kind == PORSOL_TEXTO) {
        return strcmp(a->s ? a->s : "", b->s ? b->s : "");
    }
    if (a->kind == PORSOL_BOOL && b->kind == PORSOL_BOOL) {
        return (a->i != 0) - (b->i != 0);
    }
    double na = numero_de(a), nb = numero_de(b);
    if ((a->kind == PORSOL_INTEIRO || a->kind == PORSOL_DOUBLE) &&
        (b->kind == PORSOL_INTEIRO || b->kind == PORSOL_DOUBLE)) {
        return (na > nb) - (na < nb);
    }
    return 0;
}

/* ================================================================== *//* Vetor dinâmico de PorsolValue                                        */
/* ================================================================== */

typedef struct PVec {
    PorsolValue *itens;
    int n, cap;
} PVec;

static void pvec_init(PVec *v) {
    v->itens = NULL;
    v->n = 0;
    v->cap = 0;
}

static void pvec_reserve(PVec *v, int cap) {
    if (cap <= v->cap) return;
    int ncap = v->cap ? v->cap * 2 : 8;
    while (ncap < cap) ncap *= 2;
    v->itens = (PorsolValue *)realloc(v->itens, sizeof(PorsolValue) * ncap);
    v->cap = ncap;
}

static void pvec_push(PVec *v, PorsolValue item) {
    pvec_reserve(v, v->n + 1);
    v->itens[v->n++] = item;
}

static void pvec_free(PVec *v) {
    free(v->itens);
    v->itens = NULL;
    v->n = v->cap = 0;
}

/* ================================================================== */
/* Pares chave/valor para Dicionario                                    */
/* ================================================================== */

typedef struct PDPar {
    PorsolValue chave;
    PorsolValue valor;
} PDPar;

typedef struct PDict {
    PDPar *itens;
    int n, cap;
} PDict;

static void pdict_init(PDict *d) {
    d->itens = NULL;
    d->n = 0;
    d->cap = 0;
}

static void pdict_reserve(PDict *d, int cap) {
    if (cap <= d->cap) return;
    int ncap = d->cap ? d->cap * 2 : 8;
    while (ncap < cap) ncap *= 2;
    d->itens = (PDPar *)realloc(d->itens, sizeof(PDPar) * ncap);
    d->cap = ncap;
}

static int pdict_encontrar(const PDict *d, const PorsolValue *chave) {
    for (int i = 0; i < d->n; i++) {
        if (valor_igual(&d->itens[i].chave, chave)) return i;
    }
    return -1;
}

static void pdict_definir(PDict *d, PorsolValue chave, PorsolValue valor) {
    int i = pdict_encontrar(d, &chave);
    if (i >= 0) {
        d->itens[i].valor = valor;
        return;
    }
    pdict_reserve(d, d->n + 1);
    d->itens[d->n].chave = chave;
    d->itens[d->n].valor = valor;
    d->n++;
}

static void pdict_remover(PDict *d, int idx) {
    if (idx < 0 || idx >= d->n) return;
    memmove(&d->itens[idx], &d->itens[idx + 1], sizeof(PDPar) * (d->n - idx - 1));
    d->n--;
}

/* ================================================================== */
/* Objeto stdlib                                                        */
/* ================================================================== */

typedef enum EstadoKind {
    EST_GENERICO = 0,   /* campos genéricos */
    EST_VETOR,          /* PVec — Lista/Fila/Pilha/Conjunto */
    EST_DICIONARIO,     /* PDict */
    EST_TEXTO,          /* char* buffer — ConstrutorTexto */
    EST_LEITOR,         /* { char* restante; int fechado; } */
    EST_ALEATORIO,      /* u64 estado */
    EST_MUTEX,          /* int travado */
    EST_SOCKET          /* int fd (-1 = fechado) — SocketTcp/ServidorTcp */
} EstadoKind;

typedef struct TxtBuf {
    char *buf;
    size_t len, cap;
} TxtBuf;

typedef struct LeitorEstado {
    char *restante;
    int fechado;
} LeitorEstado;

/* Magic no primeiro campo: distingue handles PorsolObj de ponteiros de
 * objetos de usuário que também circulam como PORSOL_OBJETO. */
#define PORSOL_OBJ_MAGIC 0x504F52534F4C454DULL /* "PORSOLEM" */

typedef struct PorsolObj {
    uint64_t magic;         /* DEVE ser o primeiro campo */
    char *classe;            /* nome simples (ex.: "Lista") */
    char *fqn;               /* FQN completo */
    PorsolValue *campos_val; /* valores paralelos a campos_nom */
    char **campos_nom;
    int ncampos, capcampos;
    EstadoKind estado_kind;
    void *estado;
} PorsolObj;

static void obj_set_campo(PorsolObj *o, const char *nome, PorsolValue val) {
    for (int i = 0; i < o->ncampos; i++) {
        if (strcmp(o->campos_nom[i], nome) == 0) {
            o->campos_val[i] = val;
            return;
        }
    }
    if (o->ncampos == o->capcampos) {
        int ncap = o->capcampos ? o->capcampos * 2 : 8;
        o->campos_nom = (char **)realloc(o->campos_nom, sizeof(char *) * ncap);
        o->campos_val = (PorsolValue *)realloc(o->campos_val, sizeof(PorsolValue) * ncap);
        o->capcampos = ncap;
    }
    o->campos_nom[o->ncampos] = porsol_strdup(nome);
    o->campos_val[o->ncampos] = val;
    o->ncampos++;
}

static PorsolValue *obj_get_campo(PorsolObj *o, const char *nome) {
    for (int i = 0; i < o->ncampos; i++) {
        if (strcmp(o->campos_nom[i], nome) == 0) return &o->campos_val[i];
    }
    return NULL;
}

static int campo_int(PorsolObj *o, const char *nome, int padrao) {
    PorsolValue *v = obj_get_campo(o, nome);
    return v && v->kind == PORSOL_INTEIRO ? (int)v->i : padrao;
}

static void campo_set_int(PorsolObj *o, const char *nome, int valor) {
    obj_set_campo(o, nome, v_int(valor));
}

static PVec *obj_vetor(PorsolObj *o) {
    return o && o->estado_kind == EST_VETOR ? (PVec *)o->estado : NULL;
}

static PDict *obj_dict(PorsolObj *o) {
    return o && o->estado_kind == EST_DICIONARIO ? (PDict *)o->estado : NULL;
}

static const char *classe_simples(const char *fqn) {
    const char *p = strrchr(fqn, '.');
    return p ? p + 1 : fqn;
}

static int classe_eh(PorsolObj *o, const char *simples) {
    return o && o->classe && strcmp(o->classe, simples) == 0;
}

/* Cria um objeto lista (usado também para retornos de Chaves/Valores). */
static PorsolObj *obj_new_lista(void) {
    PorsolObj *o = (PorsolObj *)calloc(1, sizeof(PorsolObj));
    o->magic = PORSOL_OBJ_MAGIC;
    o->classe = porsol_strdup("Lista");
    o->fqn = porsol_strdup("Sistema.Colecoes.Lista");
    o->estado_kind = EST_VETOR;
    o->estado = calloc(1, sizeof(PVec));
    pvec_init((PVec *)o->estado);
    return o;
}

i8 *porsol_obj_new(const char *fqn) {
    const char *simples = classe_simples(fqn);
    PorsolObj *o = (PorsolObj *)calloc(1, sizeof(PorsolObj));
    o->magic = PORSOL_OBJ_MAGIC;
    o->classe = porsol_strdup(simples);
    o->fqn = porsol_strdup(fqn);

    if (!strcmp(simples, "Lista") || !strcmp(simples, "Fila") ||
        !strcmp(simples, "Pilha") || !strcmp(simples, "Conjunto")) {
        o->estado_kind = EST_VETOR;
        o->estado = calloc(1, sizeof(PVec));
        pvec_init((PVec *)o->estado);
    } else if (!strcmp(simples, "Dicionario")) {
        o->estado_kind = EST_DICIONARIO;
        o->estado = calloc(1, sizeof(PDict));
        pdict_init((PDict *)o->estado);
    } else if (!strcmp(simples, "ConstrutorTexto")) {
        o->estado_kind = EST_TEXTO;
        TxtBuf *t = (TxtBuf *)calloc(1, sizeof(TxtBuf));
        t->buf = porsol_strdup("");
        t->len = 0;
        t->cap = 16;
        o->estado = t;
    } else if (!strcmp(simples, "Leitor")) {
        o->estado_kind = EST_LEITOR;
        LeitorEstado *l = (LeitorEstado *)calloc(1, sizeof(LeitorEstado));
        l->restante = porsol_strdup("");
        l->fechado = 0;
        o->estado = l;
    } else if (!strcmp(simples, "Aleatorio")) {
        o->estado_kind = EST_ALEATORIO;
        u64 *e = (u64 *)calloc(1, sizeof(u64));
        *e = 0;
        o->estado = e;
    } else if (!strcmp(simples, "Mutex")) {
        o->estado_kind = EST_MUTEX;
        int *m = (int *)calloc(1, sizeof(int));
        *m = 0;
        o->estado = m;
    } else if (!strcmp(simples, "SocketTcp") ||
               !strcmp(simples, "ServidorTcp")) {
        o->estado_kind = EST_SOCKET;
        int *fd = (int *)calloc(1, sizeof(int));
        *fd = -1;
        o->estado = fd;
    } else if (!strcmp(simples, "Horario")) {
        /* total de segundos (i64) — espelha "__segundos__" do interpretador */
        o->estado_kind = EST_GENERICO;
        obj_set_campo(o, "__segundos__", v_int(0));
    } else if (!strcmp(simples, "Cronometro")) {
        o->estado_kind = EST_GENERICO;
        obj_set_campo(o, "__inicio_ms__", v_int(0));
        obj_set_campo(o, "__fim_ms__", v_int(0));
        obj_set_campo(o, "__rodando__", v_bool(0));
    } else if (!strcmp(simples, "Regex")) {
        o->estado_kind = EST_GENERICO;
        obj_set_campo(o, "__padrao__", v_text(""));
    } else if (!strcmp(simples, "ClienteHttp")) {
        o->estado_kind = EST_GENERICO;
        obj_set_campo(o, "BaseUrl", v_text(""));
    } else {
        o->estado_kind = EST_GENERICO;
        o->estado = NULL;
    }
    return (i8 *)o;
}

/* ================================================================== */
/* Arrays trocados com o LLVM                                          */
/* ================================================================== */

/* Extrai elementos de um PorsolArrayHdr de entrada para um PVec
 * de PorsolValue (conversão por elem_kind). */
static PVec array_para_pvec(const PorsolArrayHdr *h) {
    PVec out;
    pvec_init(&out);
    if (!h) return out;
    for (int i = 0; i < h->len; i++) {
        PorsolValue v = v_nulo();
        switch (h->elem_kind) {
            case PORSOL_INTEIRO:
                v = v_int(((const int32_t *)h->data)[i]);
                break;
            case PORSOL_DOUBLE:
                v = v_dbl(((const double *)h->data)[i]);
                break;
            case PORSOL_BOOL:
                v = v_bool(((const uint8_t *)h->data)[i]);
                break;
            case PORSOL_TEXTO: {
                const char *s = ((const char **)h->data)[i];
                v = v_text(s ? s : "");
                break;
            }
            case PORSOL_OBJETO:
                v = v_obj(((void **)h->data)[i]);
                break;
            default:
                v = v_nulo();
                break;
        }
        pvec_push(&out, v);
    }
    return out;
}

/* Itens de um valor que pode ser Lista (objeto) ou array cru —
 * equivalente a itens_de_lista do interpretador. */
static PVec itens_de_lista(const PorsolValue *v) {
    PVec out;
    pvec_init(&out);
    if (!v) return out;
    if (v->kind == PORSOL_ARRAY) {
        PVec tmp = array_para_pvec((PorsolArrayHdr *)v->o);
        out = tmp;
        return out;
    }
    if (v->kind == PORSOL_OBJETO) {
        PVec *vec = obj_vetor((PorsolObj *)v->o);
        if (vec) {
            for (int i = 0; i < vec->n; i++) pvec_push(&out, vec->itens[i]);
        }
    }
    return out;
}

/* ================================================================== */
/* Aleatório (xorshift64*, mesmo algoritmo do interpretador)            */
/* ================================================================== */

static u64 semente_aleatoria(u64 semente) {
    u64 z = semente;
    z = (z ^ (z >> 30)) * 0xBF58476D1CE4E5B9ULL;
    z = (z ^ (z >> 27)) * 0x94D049BB133111EBULL;
    return z ^ (z >> 31);
}

static u64 proximo_aleatorio_u64(u64 *estado) {
    u64 x = *estado;
    if (x == 0) x = semente_aleatoria(0x9E3779B97F4A7C15ULL);
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    *estado = x;
    return x * 0x2545F4914F6CDD1DULL;
}

/* ================================================================== */
/* Data/hora (algoritmo civil de Hinnant, como no interpretador)        */
/* ================================================================== */

static void agora_decomposto(int *ano, int *mes, int *dia,
                             int *hora, int *minuto, int *segundo) {
    time_t t = time(NULL);
    struct tm *tmv = gmtime(&t);
    if (!tmv) {
        *ano = 1970; *mes = 1; *dia = 1;
        *hora = *minuto = *segundo = 0;
        return;
    }
    *ano = tmv->tm_year + 1900;
    *mes = tmv->tm_mon + 1;
    *dia = tmv->tm_mday;
    *hora = tmv->tm_hour;
    *minuto = tmv->tm_min;
    *segundo = tmv->tm_sec;
}

static PorsolObj *obj_new_data(void) {
    PorsolObj *o = (PorsolObj *)calloc(1, sizeof(PorsolObj));
    o->magic = PORSOL_OBJ_MAGIC;
    o->classe = porsol_strdup("Data");
    o->fqn = porsol_strdup("Sistema.Data.Data");
    o->estado_kind = EST_GENERICO;
    campo_set_int(o, "Ano", 1970);
    campo_set_int(o, "Mes", 1);
    campo_set_int(o, "Dia", 1);
    campo_set_int(o, "Hora", 0);
    campo_set_int(o, "Minuto", 0);
    campo_set_int(o, "Segundo", 0);
    return o;
}

static void data_formatar(PorsolObj *d, const char *formato, char *out, size_t outsz) {
    int dia = campo_int(d, "Dia", 1);
    int mes = campo_int(d, "Mes", 1);
    int ano = campo_int(d, "Ano", 2000);
    int hora = campo_int(d, "Hora", 0);
    int min = campo_int(d, "Minuto", 0);
    int seg = campo_int(d, "Segundo", 0);
    char tmp[512];
    snprintf(tmp, sizeof(tmp), "%s", formato ? formato : "");
    /* substituições sequenciais respeitando a ordem do interpretador */
    char buf[512];
    size_t pos = 0;
    for (size_t i = 0; tmp[i] && pos < sizeof(buf) - 32; ) {
        if (!strncmp(tmp + i, "aaaa", 4)) {
            pos += snprintf(buf + pos, sizeof(buf) - pos, "%04d", ano);
            i += 4;
        } else if (!strncmp(tmp + i, "dd", 2) && i + 1 < strlen(tmp) &&
                   (i == 0 || tmp[i - 1] != 'a')) {
            /* 'dd' só se não for parte de 'dddd' — simplificação: aceita */
            pos += snprintf(buf + pos, sizeof(buf) - pos, "%02d", dia);
            i += 2;
        } else if (!strncmp(tmp + i, "MM", 2)) {
            pos += snprintf(buf + pos, sizeof(buf) - pos, "%02d", mes);
            i += 2;
        } else if (!strncmp(tmp + i, "HH", 2)) {
            pos += snprintf(buf + pos, sizeof(buf) - pos, "%02d", hora);
            i += 2;
        } else if (!strncmp(tmp + i, "mm", 2)) {
            pos += snprintf(buf + pos, sizeof(buf) - pos, "%02d", min);
            i += 2;
        } else if (!strncmp(tmp + i, "ss", 2)) {
            pos += snprintf(buf + pos, sizeof(buf) - pos, "%02d", seg);
            i += 2;
        } else if (!strncmp(tmp + i, "aa", 2) &&
                   !(i + 1 < strlen(tmp) && tmp[i + 2] == 'a')) {
            pos += snprintf(buf + pos, sizeof(buf) - pos, "%02d", ano % 100);
            i += 2;
        } else {
            buf[pos++] = tmp[i++];
        }
    }
    buf[pos] = '\0';
    snprintf(out, outsz, "%s", buf);
}

/* ================================================================== */
/* Console                                                              */
/* ================================================================== */

static void console_escrever_linha(int argc, const PorsolValue *args) {
    /* Concatena todos os argumentos como o EscreverLinha do interpretador
     * (que usa apenas o primeiro — aqui imprimimos todos por completude
     * quando argc > 1, separando por espaço). */
    for (int i = 0; i < argc; i++) {
        fputs(texto_de(&args[i]), stdout);
        if (i + 1 < argc) putchar(' ');
    }
    putchar('\n');
    fflush(stdout);
}

static void console_escrever(int argc, const PorsolValue *args) {
    for (int i = 0; i < argc; i++) fputs(texto_de(&args[i]), stdout);
    fflush(stdout);
}

static PorsolValue console_ler_linha(void) {
    /* Leitura portátil (sem getline do POSIX): buffer dinâmico com fgets. */
    size_t cap = 256, len = 0;
    char *linha = (char *)malloc(cap);
    if (!linha) return v_text("");
    linha[0] = '\0';
    while (fgets(linha + len, (int)(cap - len), stdin)) {
        len = strlen(linha);
        if (len && linha[len - 1] == '\n') break;
        if (len + 2 >= cap) {
            cap *= 2;
            linha = (char *)realloc(linha, cap);
            if (!linha) return v_text("");
        }
    }
    len = strlen(linha);
    while (len && (linha[len - 1] == '\n' || linha[len - 1] == '\r'))
        linha[--len] = '\0';
    PorsolValue v = v_text_move(linha);
    return v;
}

/* ================================================================== */
/* Arquivo / Diretorio / Caminho / Leitor / InformacaoArquivo            */
/* ================================================================== */

static int arquivo_existe(const char *caminho) {
    struct stat st;
    return stat(caminho, &st) == 0 && S_ISREG(st.st_mode);
}

static int diretorio_existe(const char *caminho) {
    struct stat st;
    return stat(caminho, &st) == 0 && S_ISDIR(st.st_mode);
}

static char *ler_arquivo_todo(const char *caminho) {
    FILE *f = fopen(caminho, "rb");
    if (!f) return NULL;
    fseek(f, 0, SEEK_END);
    long tam = ftell(f);
    fseek(f, 0, SEEK_SET);
    if (tam < 0) { fclose(f); return NULL; }
    char *buf = (char *)malloc((size_t)tam + 1);
    if (!buf) { fclose(f); return NULL; }
    size_t lidos = fread(buf, 1, (size_t)tam, f);
    buf[lidos] = '\0';
    fclose(f);
    return buf;
}

static int cmp_strptr(const void *a, const void *b) {
    return strcmp(*(char *const *)a, *(char *const *)b);
}

/* Lista ordenada de nomes de um diretório.
 * `somente_arquivos`: 1=arquivos, 0=subdiretórios. */
static char **listar_diretorio(const char *caminho, int somente_arquivos, int *count) {
    char **nomes = NULL;
    int n = 0, cap = 0;
#ifdef _WIN32
    char padrao[1024];
    snprintf(padrao, sizeof(padrao), "%s\\*", caminho);
    WIN32_FIND_DATAA fd;
    HANDLE h = FindFirstFileA(padrao, &fd);
    if (h != INVALID_HANDLE_VALUE) {
        do {
            if (!strcmp(fd.cFileName, ".") || !strcmp(fd.cFileName, "..")) continue;
            int eh_arq = !(fd.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY);
            if (eh_arq != somente_arquivos) continue;
            if (n == cap) { cap = cap ? cap * 2 : 16; nomes = realloc(nomes, cap * sizeof(char *)); }
            nomes[n++] = porsol_strdup(fd.cFileName);
        } while (FindNextFileA(h, &fd));
        FindClose(h);
    }
#else
    DIR *d = opendir(caminho);
    if (d) {
        struct dirent *ent;
        while ((ent = readdir(d)) != NULL) {
            if (!strcmp(ent->d_name, ".") || !strcmp(ent->d_name, "..")) continue;
            char full[4096];
            snprintf(full, sizeof(full), "%s/%s", caminho, ent->d_name);
            struct stat st;
            if (stat(full, &st) != 0) continue;
            int eh_arq = S_ISREG(st.st_mode);
            if (eh_arq != somente_arquivos) continue;
            if (n == cap) { cap = cap ? cap * 2 : 16; nomes = realloc(nomes, cap * sizeof(char *)); }
            nomes[n++] = porsol_strdup(ent->d_name);
        }
        closedir(d);
    }
#endif
    qsort(nomes, (size_t)n, sizeof(char *), cmp_strptr);
    *count = n;
    return nomes;
}

static PorsolValue info_arquivo_novo(const char *caminho) {
    PorsolObj *o = (PorsolObj *)calloc(1, sizeof(PorsolObj));
    o->magic = PORSOL_OBJ_MAGIC;
    o->classe = porsol_strdup("InformacaoArquivo");
    o->fqn = porsol_strdup("Sistema.IO.InformacaoArquivo");
    o->estado_kind = EST_GENERICO;

    struct stat st;
    int existe = stat(caminho, &st) == 0;
    const char *base = strrchr(caminho, '/');
    const char *base2 = strrchr(caminho, '\\');
    if (base2 > base) base = base2;
    const char *nome = base ? base + 1 : caminho;

    /* diretório pai */
    char pai[4096];
    snprintf(pai, sizeof(pai), "%s", caminho);
    char *slash = strrchr(pai, '/');
    char *bslash = strrchr(pai, '\\');
    char *ult = (slash > bslash) ? slash : bslash;
    if (ult) *ult = '\0'; else snprintf(pai, sizeof(pai), ".");

    obj_set_campo(o, "__caminho", v_text(caminho));
    obj_set_campo(o, "__existe", v_bool(existe));
    obj_set_campo(o, "__nome", v_text(nome));
    obj_set_campo(o, "__diretorio", v_text(pai));
    obj_set_campo(o, "__tamanho", v_int(existe ? (int64_t)st.st_size : 0));
    char data_buf[32] = "";
    if (existe) {
        time_t mt = st.st_mtime;
        struct tm *tmv = gmtime(&mt);
        if (tmv) snprintf(data_buf, sizeof(data_buf), "%04d-%02d-%02dT%02d:%02d:%02d",
                          tmv->tm_year + 1900, tmv->tm_mon + 1, tmv->tm_mday,
                          tmv->tm_hour, tmv->tm_min, tmv->tm_sec);
    }
    obj_set_campo(o, "__modificacao", v_text(data_buf));
    return v_obj(o);
}

static PorsolValue leitor_novo(const char *caminho) {
    char *conteudo = ler_arquivo_todo(caminho);
    PorsolObj *o = (PorsolObj *)calloc(1, sizeof(PorsolObj));
    o->magic = PORSOL_OBJ_MAGIC;
    o->classe = porsol_strdup("Leitor");
    o->fqn = porsol_strdup("Sistema.IO.Leitor");
    o->estado_kind = EST_LEITOR;
    LeitorEstado *l = (LeitorEstado *)calloc(1, sizeof(LeitorEstado));
    l->restante = conteudo ? conteudo : porsol_strdup("");
    l->fechado = 0;
    o->estado = l;
    return v_obj(o);
}

/* ================================================================== */
/* Texto (funções estáticas)                                           */
/* ================================================================== */

/* Divide `texto` por `sep` (sep vazio → caracteres UTF-8). Retorna
 * PorsolValue ARRAY de TEXTO. */
static PorsolValue texto_dividir(const char *texto, const char *sep) {
    /* conta partes */
    int n = 0;
    size_t seplen = strlen(sep);
    if (seplen == 0) {
        n = porsol_utf8_len(texto);
        if (n == 0) n = 1;
    } else {
        const char *p = texto;
        n = 1;
        while ((p = strstr(p, sep)) != NULL) { n++; p += seplen; }
    }
    char **partes = (char **)malloc(sizeof(char *) * (n > 0 ? n : 1));
    int idx = 0;
    if (seplen == 0) {
        /* por caractere UTF-8 */
        const unsigned char *p = (const unsigned char *)texto;
        while (*p && idx < n) {
            int len = (*p < 0x80) ? 1 : (*p < 0xE0) ? 2 : (*p < 0xF0) ? 3 : 4;
            partes[idx++] = porsol_strndup((const char *)p, (size_t)len);
            p += len;
        }
        if (idx == 0) partes[idx++] = porsol_strdup("");
    } else {
        const char *ini = texto;
        const char *p = strstr(ini, sep);
        while (p && idx < n - 1) {
            partes[idx++] = porsol_strndup(ini, (size_t)(p - ini));
            ini = p + seplen;
            p = strstr(ini, sep);
        }
        partes[idx++] = porsol_strdup(ini);
        n = idx;
    }
    /* array de char* para PorsolArrayHdr TEXTO */
    PorsolArrayHdr *h = (PorsolArrayHdr *)calloc(1, sizeof(PorsolArrayHdr));
    h->len = n;
    h->elem_kind = PORSOL_TEXTO;
    h->data = malloc(sizeof(char *) * (n > 0 ? n : 1));
    memcpy(h->data, partes, sizeof(char *) * n);
    free(partes);
    return v_array(h);
}

static PorsolValue texto_dividir_linhas(const char *texto) {
    /* conta linhas */
    int n = 1;
    for (const char *p = texto; *p; p++) if (*p == '\n') n++;
    char **partes = (char **)malloc(sizeof(char *) * n);
    int idx = 0;
    const char *ini = texto;
    for (const char *p = texto;; p++) {
        if (*p == '\n' || *p == '\0') {
            size_t len = (size_t)(p - ini);
            if (len && ini[len - 1] == '\r') len--;
            partes[idx++] = porsol_strndup(ini, len);
            if (*p == '\0') break;
            ini = p + 1;
        }
    }
    PorsolArrayHdr *h = (PorsolArrayHdr *)calloc(1, sizeof(PorsolArrayHdr));
    h->len = idx;
    h->elem_kind = PORSOL_TEXTO;
    h->data = malloc(sizeof(char *) * (idx > 0 ? idx : 1));
    memcpy(h->data, partes, sizeof(char *) * idx);
    free(partes);
    return v_array(h);
}

static char *ascii_case(const char *s, int upper) {
    char *r = porsol_strdup(s ? s : "");
    for (char *p = r; *p; p++) {
        *p = (char)(upper ? toupper((unsigned char)*p) : tolower((unsigned char)*p));
    }
    return r;
}

static char *texto_substituir(const char *texto, const char *antigo, const char *novo) {
    size_t seplen = strlen(antigo);
    if (seplen == 0) return porsol_strdup(texto);
    size_t textolen = strlen(texto), novolen = strlen(novo);
    size_t cap = textolen + 16;
    char *out = (char *)malloc(cap);
    size_t op = 0;
    const char *p = texto;
    const char *m;
    while ((m = strstr(p, antigo)) != NULL) {
        size_t pre = (size_t)(m - p);
        if (op + pre + novolen + 1 > cap) {
            cap = (op + pre + novolen + 1) * 2;
            out = (char *)realloc(out, cap);
        }
        memcpy(out + op, p, pre);
        op += pre;
        memcpy(out + op, novo, novolen);
        op += novolen;
        p = m + seplen;
    }
    size_t rest = strlen(p);
    if (op + rest + 1 > cap) out = (char *)realloc(out, op + rest + 1);
    memcpy(out + op, p, rest + 1);
    return out;
}

static char *texto_trim(const char *s) {
    while (*s && isspace((unsigned char)*s)) s++;
    size_t len = strlen(s);
    while (len && isspace((unsigned char)s[len - 1])) len--;
    return porsol_strndup(s, len);
}

/* ================================================================== */
/* ConstrutorTexto                                                      */
/* ================================================================== */

static void txtbuf_append(TxtBuf *t, const char *s) {
    size_t n = strlen(s);
    if (t->len + n + 1 > t->cap) {
        while (t->len + n + 1 > t->cap) t->cap *= 2;
        t->buf = (char *)realloc(t->buf, t->cap);
    }
    memcpy(t->buf + t->len, s, n + 1);
    t->len += n;
}

static TxtBuf *obj_txtbuf(PorsolObj *o) {
    return o && o->estado_kind == EST_TEXTO ? (TxtBuf *)o->estado : NULL;
}

/* Substitui o conteúdo do TxtBuf por uma nova string (tomada). */
static void txtbuf_set(TxtBuf *t, char *novo) {
    if (!t) { free(novo); return; }
    free(t->buf);
    t->buf = novo;
    t->len = novo ? strlen(novo) : 0;
    t->cap = t->len + 1;
}

/* ================================================================== */
/* Dispatcher                                                           */
/* ================================================================== */

/* ---------- helpers de args ---------- */

static PorsolObj *self_obj(int argc, const PorsolValue *args) {
    if (argc < 1 || args[0].kind != PORSOL_OBJETO || !args[0].o) return NULL;
    PorsolObj *o = (PorsolObj *)args[0].o;
    /* Só trata como objeto stdlib se o magic confere — objetos de
     * usuário também chegam como PORSOL_OBJETO mas são ponteiros opacos. */
    if (o->magic != PORSOL_OBJ_MAGIC) return NULL;
    return o;
}

static const char *arg_texto(int idx, int argc, const PorsolValue *args) {
    return idx < argc ? texto_de(&args[idx]) : "";
}

static double arg_num(int idx, int argc, const PorsolValue *args) {
    return idx < argc ? numero_de(&args[idx]) : 0.0;
}

static int64_t arg_int(int idx, int argc, const PorsolValue *args) {
    return idx < argc ? (int64_t)numero_de(&args[idx]) : 0;
}

/* ---------- Construtores ---------- */

static void ctor_lista(PorsolObj *o, int argc, const PorsolValue *args) {
    PVec *v = obj_vetor(o);
    if (v && argc >= 2) {
        int cap = (int)arg_num(1, argc, args);
        if (cap > 0) pvec_reserve(v, cap);
    }
}

static void ctor_data(PorsolObj *o, int argc, const PorsolValue *args) {
    campo_set_int(o, "Ano", (int)arg_int(1, argc, args));
    campo_set_int(o, "Mes", (int)arg_int(2, argc, args));
    campo_set_int(o, "Dia", (int)arg_int(3, argc, args));
    campo_set_int(o, "Hora", argc > 4 ? (int)arg_int(4, argc, args) : 0);
    campo_set_int(o, "Minuto", argc > 5 ? (int)arg_int(5, argc, args) : 0);
    campo_set_int(o, "Segundo", argc > 6 ? (int)arg_int(6, argc, args) : 0);
}

static void ctor_aleatorio(PorsolObj *o, int argc, const PorsolValue *args) {
    u64 *e = (u64 *)o->estado;
    if (e && argc >= 2) *e = semente_aleatoria((u64)arg_int(1, argc, args));
}

static void ctor_horario(PorsolObj *o, int argc, const PorsolValue *args) {
    /* (segundos) ou (dias, horas, minutos, segundos) — args[0] é self */
    int64_t seg;
    if (argc >= 5) {
        seg = arg_int(1, argc, args) * 86400 + arg_int(2, argc, args) * 3600 +
              arg_int(3, argc, args) * 60 + arg_int(4, argc, args);
    } else {
        seg = arg_int(1, argc, args);
    }
    obj_set_campo(o, "__segundos__", v_int(seg));
}

static void ctor_regex(PorsolObj *o, int argc, const PorsolValue *args) {
    if (argc >= 2) {
        obj_set_campo(o, "__padrao__", v_text(arg_texto(1, argc, args)));
    }
}

static void ctor_cliente_http(PorsolObj *o, int argc, const PorsolValue *args) {
    if (argc >= 2) {
        obj_set_campo(o, "BaseUrl", v_text(arg_texto(1, argc, args)));
    }
}

/* ---------- helpers diversos ---------- */

static int64_t campo_i64(PorsolObj *o, const char *nome, int64_t padrao) {
    PorsolValue *v = obj_get_campo(o, nome);
    return v && v->kind == PORSOL_INTEIRO ? v->i : padrao;
}

static void campo_set_i64(PorsolObj *o, const char *nome, int64_t valor) {
    obj_set_campo(o, nome, v_int(valor));
}

/* Relógio monotônico em milissegundos (Cronometro). */
static int64_t agora_ms(void) {
#ifdef _WIN32
    static LARGE_INTEGER freq = { 0 };
    LARGE_INTEGER contador;
    if (freq.QuadPart == 0) QueryPerformanceFrequency(&freq);
    QueryPerformanceCounter(&contador);
    return (int64_t)((contador.QuadPart * 1000) / freq.QuadPart);
#else
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (int64_t)ts.tv_sec * 1000 + ts.tv_nsec / 1000000;
#endif
}

/* Horario (TimeSpan): total de segundos com sinal. */
static int64_t horario_segundos(PorsolObj *o) {
    return o ? campo_i64(o, "__segundos__", 0) : 0;
}

static PorsolObj *obj_new_horario(int64_t segundos) {
    PorsolObj *o = (PorsolObj *)calloc(1, sizeof(PorsolObj));
    o->magic = PORSOL_OBJ_MAGIC;
    o->classe = porsol_strdup("Horario");
    o->fqn = porsol_strdup("Sistema.Data.Horario");
    o->estado_kind = EST_GENERICO;
    obj_set_campo(o, "__segundos__", v_int(segundos));
    return o;
}

/* "HH:MM:SS" ou "d.HH:MM:SS"; sinal negativo prefixa "-". */
static char *horario_para_texto(int64_t segundos) {
    uint64_t total = segundos < 0 ? (uint64_t)(-segundos) : (uint64_t)segundos;
    uint64_t dias = total / 86400;
    int horas = (int)((total / 3600) % 24);
    int minutos = (int)((total / 60) % 60);
    int segs = (int)(total % 60);
    char tmp[64];
    if (dias > 0) {
        snprintf(tmp, sizeof(tmp), "%llu.%02d:%02d:%02d",
                 (unsigned long long)dias, horas, minutos, segs);
    } else {
        snprintf(tmp, sizeof(tmp), "%02d:%02d:%02d", horas, minutos, segs);
    }
    if (segundos < 0) {
        char *r = (char *)malloc(strlen(tmp) + 2);
        sprintf(r, "-%s", tmp);
        return r;
    }
    return porsol_strdup(tmp);
}

/* ---------- UTF-8 ---------- */

/* Comprimento em bytes do codepoint que começa em p (1-4). */
static int utf8_cp_len(const char *p) {
    unsigned char c = (unsigned char)*p;
    if (c < 0x80) return 1;
    if ((c & 0xE0) == 0xC0) return 2;
    if ((c & 0xF0) == 0xE0) return 3;
    if ((c & 0xF8) == 0xF0) return 4;
    return 1;
}

static int utf8_len(const char *s) { return porsol_utf8_len(s); }

/* Índice de caractere -> offset em bytes. */
static size_t utf8_char_to_byte(const char *s, int64_t char_idx) {
    int64_t i = 0;
    size_t b = 0;
    while (s[b] && i < char_idx) {
        b += (size_t)utf8_cp_len(s + b);
        i++;
    }
    return b;
}

/* Número de caracteres numa fatia de bytes [ini, fim). */
static int64_t utf8_count_chars(const char *s, size_t ini, size_t fim) {
    int64_t n = 0;
    for (size_t b = ini; b < fim && s[b]; ) {
        b += (size_t)utf8_cp_len(s + b);
        n++;
    }
    return n;
}

/* Localiza `agulha` em `fardo`; retorna offset em bytes ou -1. */
static int64_t utf8_find(const char *fardo, const char *agulha, int de_tras) {
    size_t lf = strlen(fardo), la = strlen(agulha);
    if (!la) return 0;
    if (de_tras) {
        if (la > lf) return -1;
        for (size_t i = lf - la + 1; i-- > 0;) {
            if (!memcmp(fardo + i, agulha, la)) return (int64_t)i;
        }
        return -1;
    }
    const char *p = strstr(fardo, agulha);
    return p ? (int64_t)(p - fardo) : -1;
}

/* Contagem de ocorrências não sobrepostas. */
static int64_t utf8_count_occurrences(const char *fardo, const char *agulha) {
    size_t la = strlen(agulha);
    if (!la) return 0;
    int64_t n = 0;
    const char *p = fardo;
    while ((p = strstr(p, agulha)) != NULL) {
        n++;
        p += la;
    }
    return n;
}

/* Inverte por codepoints UTF-8. */
static char *utf8_inverter(const char *s) {
    size_t len = strlen(s);
    /* coleta offsets de cada codepoint */
    size_t *offs = (size_t *)malloc(sizeof(size_t) * (len + 1));
    int n = 0;
    for (size_t b = 0; b < len; ) {
        offs[n++] = b;
        b += (size_t)utf8_cp_len(s + b);
    }
    size_t total = len + 1;
    char *r = (char *)malloc(total);
    size_t pos = 0;
    for (int i = n - 1; i >= 0; i--) {
        size_t ini = offs[i];
        size_t fim = (i + 1 < n) ? offs[i + 1] : len;
        memcpy(r + pos, s + ini, fim - ini);
        pos += fim - ini;
    }
    r[pos] = '\0';
    free(offs);
    return r;
}

static int ascii_lower(int c) {
    return (c >= 'A' && c <= 'Z') ? c + 32 : c;
}

static int ascii_case_equal(const char *a, const char *b) {
    while (*a && *b) {
        if (ascii_lower((unsigned char)*a) != ascii_lower((unsigned char)*b)) return 0;
        a++; b++;
    }
    return *a == *b;
}

/* ---------- arrays de saída (PorsolArrayHdr) ---------- */

/* Converte PVec de PorsolValue para PorsolArrayHdr com elementos no
 * layout LLVM ({i32 len, i32 kind, i8* data} — data contíguo por tipo).
 * O elem_kind é inferido do primeiro elemento (INTEIRO se vazio). */
static PorsolValue pvec_para_array_val(PVec *v) {
    PorsolArrayHdr *h = (PorsolArrayHdr *)calloc(1, sizeof(PorsolArrayHdr));
    h->len = v->n;
    int kind = v->n > 0 ? v->itens[0].kind : PORSOL_INTEIRO;
    /* arrays são homogêneos: normaliza para o kind do primeiro elemento */
    h->elem_kind = kind;
    size_t sz = 0;
    switch (kind) {
        case PORSOL_INTEIRO: sz = sizeof(int32_t); break;
        case PORSOL_DOUBLE:  sz = sizeof(double); break;
        case PORSOL_BOOL:    sz = sizeof(uint8_t); break;
        case PORSOL_TEXTO:   sz = sizeof(char *); break;
        case PORSOL_OBJETO:  sz = sizeof(void *); break;
        default:             sz = sizeof(void *); kind = PORSOL_OBJETO; break;
    }
    h->elem_kind = kind;
    h->data = malloc(sz * (v->n > 0 ? v->n : 1));
    for (int i = 0; i < v->n; i++) {
        PorsolValue *it = &v->itens[i];
        switch (kind) {
            case PORSOL_INTEIRO: ((int32_t *)h->data)[i] = (int32_t)it->i; break;
            case PORSOL_DOUBLE:  ((double *)h->data)[i] = numero_de(it); break;
            case PORSOL_BOOL:    ((uint8_t *)h->data)[i] = it->kind == PORSOL_BOOL ? (uint8_t)(it->i != 0) : (uint8_t)(numero_de(it) != 0.0); break;
            case PORSOL_TEXTO:   ((char **)h->data)[i] = porsol_strdup(texto_de(it)); break;
            default:             ((void **)h->data)[i] = it->o; break;
        }
    }
    PorsolValue out = v_nulo();
    out.kind = PORSOL_ARRAY;
    out.o = h;
    return out;
}

/* Lista-objeto nova a partir de um PVec (para ParaLista e retornos). */
static PorsolObj *obj_new_colecao(const char *simples, const char *fqn, PVec *itens) {
    PorsolObj *o = (PorsolObj *)calloc(1, sizeof(PorsolObj));
    o->magic = PORSOL_OBJ_MAGIC;
    o->classe = porsol_strdup(simples);
    o->fqn = porsol_strdup(fqn);
    o->estado_kind = EST_VETOR;
    PVec *v = (PVec *)calloc(1, sizeof(PVec));
    pvec_init(v);
    if (itens) {
        for (int i = 0; i < itens->n; i++) pvec_push(v, itens->itens[i]);
    }
    o->estado = v;
    return o;
}

/* Copia o PVec de um objeto coleção para um novo PVec. */
static void pvec_copiar(PVec *dst, const PVec *src) {
    pvec_init(dst);
    if (!src) return;
    for (int i = 0; i < src->n; i++) pvec_push(dst, src->itens[i]);
}

/* Itens de um argumento que pode ser coleção-objeto ou array. */
static void pvec_de_arg(PVec *out, const PorsolValue *v) {
    pvec_init(out);
    if (!v) return;
    if (v->kind == PORSOL_ARRAY) {
        PVec tmp = array_para_pvec((PorsolArrayHdr *)v->o);
        *out = tmp;
        return;
    }
    if (v->kind == PORSOL_OBJETO) {
        PVec *vec = obj_vetor((PorsolObj *)v->o);
        pvec_copiar(out, vec);
    }
}

/* ================================================================== */
/* Utilitários de texto (índices em caracteres UTF-8)                   */
/* ================================================================== */

/* Remove `comp` caracteres a partir de `inicio` (char-based, UTF-8). */
static char *texto_remover(const char *valor, int64_t inicio, int64_t comp) {
    int64_t len = utf8_len(valor);
    if (inicio < 0) inicio = 0;
    if (inicio > len) inicio = len;
    int64_t fim = comp < 0 ? len : inicio + comp;
    if (fim > len) fim = len;
    if (fim < inicio) fim = inicio;
    size_t b_ini = utf8_char_to_byte(valor, inicio);
    size_t b_fim = utf8_char_to_byte(valor, fim);
    size_t vl = strlen(valor);
    char *buf = (char *)malloc(vl - (b_fim - b_ini) + 1);
    memcpy(buf, valor, b_ini);
    strcpy(buf + b_ini, valor + b_fim);
    return buf;
}

/* Insere `trecho` na posição `inicio` (índice em caracteres UTF-8). */
static char *texto_inserir(const char *valor, int64_t inicio, const char *trecho) {
    int64_t len = utf8_len(valor);
    if (inicio < 0) inicio = 0;
    if (inicio > len) inicio = len;
    size_t b_ini = utf8_char_to_byte(valor, inicio);
    size_t vl = strlen(valor), tl = strlen(trecho);
    char *buf = (char *)malloc(vl + tl + 1);
    memcpy(buf, valor, b_ini);
    memcpy(buf + b_ini, trecho, tl);
    strcpy(buf + b_ini + tl, valor + b_ini);
    return buf;
}

/* Colapsa sequências de espaços em branco em um único espaço e
 * remove espaços nas extremidades. */
static char *texto_normalizar_espacos(const char *s) {
    size_t n = strlen(s);
    char *buf = (char *)malloc(n + 1);
    size_t bp = 0;
    int pendente = 0;
    for (size_t i = 0; i < n; i++) {
        unsigned char c = (unsigned char)s[i];
        if (isspace(c)) {
            pendente = bp > 0;
            continue;
        }
        if (pendente) { buf[bp++] = ' '; pendente = 0; }
        buf[bp++] = (char)c;
    }
    buf[bp] = '\0';
    return buf;
}

/* Divide em "palavras" (sequências separadas por espaços em branco,
 * sem entradas vazias) e retorna array de textos. */
static PorsolValue texto_dividir_palavras(const char *s) {
    PVec v;
    pvec_init(&v);
    const char *p = s;
    while (*p) {
        while (*p && isspace((unsigned char)*p)) p++;
        if (!*p) break;
        const char *ini = p;
        while (*p && !isspace((unsigned char)*p)) p++;
        PorsolValue t = v_nulo();
        t.kind = PORSOL_TEXTO;
        t.s = porsol_strndup(ini, (size_t)(p - ini));
        pvec_push(&v, t);
    }
    PorsolValue out = pvec_para_array_val(&v);
    pvec_free(&v);
    return out;
}

/* Conta palavras (mesma regra de texto_dividir_palavras). */
static int64_t texto_contar_palavras(const char *s) {
    int64_t n = 0;
    const char *p = s;
    while (*p) {
        while (*p && isspace((unsigned char)*p)) p++;
        if (!*p) break;
        while (*p && !isspace((unsigned char)*p)) p++;
        n++;
    }
    return n;
}

/* Retorna array com todas as ocorrências não sobrepostas de `trecho`. */
static PorsolValue texto_localizar_todas(const char *valor, const char *trecho) {
    PVec v;
    pvec_init(&v);
    size_t tl = strlen(trecho);
    if (tl) {
        const char *p = valor;
        const char *achou;
        while ((achou = strstr(p, trecho)) != NULL) {
            PorsolValue t = v_nulo();
            t.kind = PORSOL_TEXTO;
            t.s = porsol_strdup(trecho);
            pvec_push(&v, t);
            p = achou + tl;
        }
    }
    PorsolValue out = pvec_para_array_val(&v);
    pvec_free(&v);
    return out;
}

/* ================================================================== */
/* Base64 / hexadecimal                                                 */
/* ================================================================== */

static const char BASE64_ALFABETO[] =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

static char *base64_codificar(const unsigned char *bytes, size_t n) {
    size_t out_n = ((n + 2) / 3) * 4;
    char *out = (char *)malloc(out_n + 1);
    size_t op = 0;
    for (size_t i = 0; i < n; i += 3) {
        unsigned b0 = bytes[i];
        unsigned b1 = i + 1 < n ? bytes[i + 1] : 0;
        unsigned b2 = i + 2 < n ? bytes[i + 2] : 0;
        unsigned v = (b0 << 16) | (b1 << 8) | b2;
        out[op++] = BASE64_ALFABETO[(v >> 18) & 63];
        out[op++] = BASE64_ALFABETO[(v >> 12) & 63];
        out[op++] = i + 1 < n ? BASE64_ALFABETO[(v >> 6) & 63] : '=';
        out[op++] = i + 2 < n ? BASE64_ALFABETO[v & 63] : '=';
    }
    out[op] = '\0';
    return out;
}

static int base64_valor(int c) {
    if (c >= 'A' && c <= 'Z') return c - 'A';
    if (c >= 'a' && c <= 'z') return c - 'a' + 26;
    if (c >= '0' && c <= '9') return c - '0' + 52;
    if (c == '+') return 62;
    if (c == '/') return 63;
    return -1;
}

/* Decodifica Base64 para bytes; retorna NULL se inválido.
 * *out_n recebe a quantidade de bytes. */
static unsigned char *base64_decodificar(const char *texto, size_t *out_n) {
    /* filtra whitespace */
    size_t n = strlen(texto);
    unsigned char *limpo = (unsigned char *)malloc(n + 1);
    size_t ln = 0;
    for (size_t i = 0; i < n; i++) {
        unsigned char c = (unsigned char)texto[i];
        if (!isspace(c)) limpo[ln++] = c;
    }
    if (ln % 4 != 0) { free(limpo); return NULL; }
    unsigned char *out = (unsigned char *)malloc((ln / 4) * 3 + 1);
    size_t on = 0;
    for (size_t i = 0; i < ln; i += 4) {
        int vals[4];
        int padding = 0;
        for (int j = 0; j < 4; j++) {
            int c = limpo[i + j];
            if (c == '=') {
                if (j < 2) { free(limpo); free(out); return NULL; }
                padding++;
                vals[j] = 0;
            } else {
                if (padding) { free(limpo); free(out); return NULL; }
                vals[j] = base64_valor(c);
                if (vals[j] < 0) { free(limpo); free(out); return NULL; }
            }
        }
        if (padding > 2) { free(limpo); free(out); return NULL; }
        unsigned v = ((unsigned)vals[0] << 18) | ((unsigned)vals[1] << 12) |
                     ((unsigned)vals[2] << 6) | (unsigned)vals[3];
        out[on++] = (unsigned char)(v >> 16);
        if (padding < 2) out[on++] = (unsigned char)(v >> 8);
        if (padding < 1) out[on++] = (unsigned char)v;
    }
    free(limpo);
    *out_n = on;
    return out;
}

static int hex_valor(int c) {
    if (c >= '0' && c <= '9') return c - '0';
    if (c >= 'a' && c <= 'f') return c - 'a' + 10;
    if (c >= 'A' && c <= 'F') return c - 'A' + 10;
    return -1;
}

/* ---------- JSON (motor mínimo em C) ---------- */
static PorsolValue json_serializar_valor(const PorsolValue *v);
static PorsolValue json_serializar_valor_pretty(const PorsolValue *v);
static PorsolValue json_desserializar(const char *texto, int *ok);
static char *json_formatar(const char *texto);
static int json_validar(const char *texto);
static const char *json_tipo_de(const char *texto);

/* ---------- Regex (engine backtracking mínima) ---------- */
typedef struct RegexProg RegexProg;
static RegexProg *regex_compilar(const char *padrao, int *ok);
static int regex_match(const RegexProg *re, const char *texto, size_t *match_ini, size_t *match_fim);
static int regex_match_from(const RegexProg *re, const char *texto, size_t pos, size_t *match_ini, size_t *match_fim);
static char *regex_escape(const char *texto);
static int regex_padrao_valido(const char *padrao);

/* ---------- Rede ---------- */
static int winsock_init(void);
static void close_sock(int fd);
static PorsolObj *socket_tcp_novo(int fd);
static PorsolObj *servidor_tcp_novo(int fd);
static char *http_request_sync(const char *metodo, const char *url, const char *corpo, int *ok);
static char *dns_resolver_host(const char *host, int todos, int *count);

/* ---------- estáticos ---------- */

static void despachar_estatico(const char *chave, int argc, const PorsolValue *args,
                               PorsolValue *out) {
    /* Console */
    if (!strcmp(chave, "Console::EscreverLinha")) {
        console_escrever_linha(argc, args);
        *out = v_nulo();
        return;
    }
    if (!strcmp(chave, "Console::Escrever")) {
        console_escrever(argc, args);
        *out = v_nulo();
        return;
    }
    if (!strcmp(chave, "Console::LerLinha")) {
        *out = console_ler_linha();
        return;
    }

    /* Matematica */
    if (!strncmp(chave, "Matematica::", 12)) {
        const char *m = chave + 12;
        double v = arg_num(0, argc, args);
        double r = 0.0;
        int eh_double = 1;
        if (!strcmp(m, "Abs")) r = fabs(v);
        else if (!strcmp(m, "Max")) r = v > arg_num(1, argc, args) ? v : arg_num(1, argc, args);
        else if (!strcmp(m, "Min")) r = v < arg_num(1, argc, args) ? v : arg_num(1, argc, args);
        else if (!strcmp(m, "Potencia")) r = pow(v, arg_num(1, argc, args));
        else if (!strcmp(m, "RaizQuadrada")) { if (v < 0) { *out = v_nulo(); return; } r = sqrt(v); }
        else if (!strcmp(m, "Arredondar")) r = round(v);
        else if (!strcmp(m, "Truncar")) r = trunc(v);
        else if (!strcmp(m, "Piso")) r = floor(v);
        else if (!strcmp(m, "Teto")) r = ceil(v);
        else if (!strcmp(m, "Seno")) r = sin(v);
        else if (!strcmp(m, "Cosseno")) r = cos(v);
        else if (!strcmp(m, "Tangente")) r = tan(v);
        else if (!strcmp(m, "LogaritmoNatural")) { if (v <= 0) { *out = v_nulo(); return; } r = log(v); }
        else if (!strcmp(m, "LogaritmoDez")) { if (v <= 0) { *out = v_nulo(); return; } r = log10(v); }
        else if (!strcmp(m, "Expoente")) r = exp(v);
        else if (!strcmp(m, "Pi")) { *out = v_dbl(3.14159265358979323846); return; }
        else if (!strcmp(m, "E")) { *out = v_dbl(2.71828182845904523536); return; }
        else eh_double = 0;
        if (eh_double) *out = v_dbl(r);
        else *out = v_nulo();
        return;
    }

    /* Texto */
    if (!strncmp(chave, "Texto::", 7)) {
        const char *m = chave + 7;
        const char *valor = arg_texto(0, argc, args);
        if (!strcmp(m, "Dividir")) {
            *out = texto_dividir(valor, arg_texto(1, argc, args));
        } else if (!strcmp(m, "DividirLinhas")) {
            *out = texto_dividir_linhas(valor);
        } else if (!strcmp(m, "Juntar")) {
            const char *sep = arg_texto(1, argc, args);
            PVec itens = argc > 0 && args[0].kind == PORSOL_ARRAY
                             ? array_para_pvec((PorsolArrayHdr *)args[0].o)
                             : itens_de_lista(&args[0]);
            size_t total = 1;
            for (int i = 0; i < itens.n; i++) total += strlen(texto_de(&itens.itens[i])) + strlen(sep);
            char *buf = (char *)malloc(total);
            buf[0] = '\0';
            for (int i = 0; i < itens.n; i++) {
                if (i) strcat(buf, sep);
                strcat(buf, texto_de(&itens.itens[i]));
            }
            *out = v_text_move(buf);
            pvec_free(&itens);
        } else if (!strcmp(m, "RemoverEspacos")) {
            *out = v_text_move(texto_trim(valor));
        } else if (!strcmp(m, "ParaMaiusculas")) {
            *out = v_text_move(ascii_case(valor, 1));
        } else if (!strcmp(m, "ParaMinusculas")) {
            *out = v_text_move(ascii_case(valor, 0));
        } else if (!strcmp(m, "Substituir")) {
            *out = v_text_move(texto_substituir(valor, arg_texto(1, argc, args),
                                                arg_texto(2, argc, args)));
        } else if (!strcmp(m, "ComecaCom")) {
            const char *pre = arg_texto(1, argc, args);
            *out = v_bool(strncmp(valor, pre, strlen(pre)) == 0);
        } else if (!strcmp(m, "TerminaCom")) {
            const char *suf = arg_texto(1, argc, args);
            size_t lv = strlen(valor), ls = strlen(suf);
            *out = v_bool(ls <= lv && strcmp(valor + lv - ls, suf) == 0);
        } else if (!strcmp(m, "Contem")) {
            *out = v_bool(strstr(valor, arg_texto(1, argc, args)) != NULL);
        } else if (!strcmp(m, "VazioOuNulo")) {
            int vazio = (argc == 0 || args[0].kind == PORSOL_NULO ||
                         (args[0].kind == PORSOL_TEXTO && (!args[0].s || !*args[0].s)));
            *out = v_bool(vazio);
        } else if (!strcmp(m, "Tamanho")) {
            *out = v_int(porsol_utf8_len(valor));
        } else if (!strcmp(m, "Subtexto")) {
            /* Subtexto(texto, inicio, comprimento=-1) — índices em caracteres */
            int64_t ini = (int64_t)arg_num(1, argc, args);
            int64_t comp = argc > 2 ? (int64_t)arg_num(2, argc, args) : -1;
            int64_t len = utf8_len(valor);
            if (ini < 0) ini = 0;
            if (ini > len) ini = len;
            int64_t fim;
            if (comp < 0) {
                fim = len;
            } else {
                fim = ini + comp;
                if (fim > len) fim = len;
            }
            size_t b_ini = utf8_char_to_byte(valor, ini);
            size_t b_fim = utf8_char_to_byte(valor, fim);
            *out = v_text_move(porsol_strndup(valor + b_ini, b_fim - b_ini));
        } else if (!strcmp(m, "IndiceDe") || !strcmp(m, "UltimoIndiceDe")) {
            const char *trecho = arg_texto(1, argc, args);
            int de_tras = !strcmp(m, "UltimoIndiceDe");
            int64_t byte_idx = utf8_find(valor, trecho, de_tras);
            int64_t char_idx = -1;
            if (byte_idx >= 0) {
                char_idx = utf8_count_chars(valor, 0, (size_t)byte_idx);
            }
            *out = v_int(char_idx);
        } else if (!strcmp(m, "Inverter")) {
            *out = v_text_move(utf8_inverter(valor));
        } else if (!strcmp(m, "PadEsquerda") || !strcmp(m, "PadDireita")) {
            int64_t largura = (int64_t)arg_num(1, argc, args);
            const char *preench = argc > 2 ? arg_texto(2, argc, args) : " ";
            int plen = preench[0] ? utf8_cp_len(preench) : 1;
            const char *fill = preench[0] ? preench : " ";
            int64_t atual = utf8_len(valor);
            if (atual >= largura) {
                *out = v_text(valor);
            } else {
                int64_t faltam = largura - atual;
                size_t fill_sz = (size_t)plen;
                size_t vl = strlen(valor);
                size_t total = vl + (size_t)faltam * fill_sz + 1;
                char *buf = (char *)malloc(total);
                char *p = buf;
                if (!strcmp(m, "PadEsquerda")) {
                    for (int64_t i = 0; i < faltam; i++) {
                        memcpy(p, fill, fill_sz); p += fill_sz;
                    }
                    memcpy(p, valor, vl + 1);
                } else {
                    memcpy(p, valor, vl); p += vl;
                    for (int64_t i = 0; i < faltam; i++) {
                        memcpy(p, fill, fill_sz); p += fill_sz;
                    }
                    *p = '\0';
                }
                *out = v_text_move(buf);
            }
        } else if (!strcmp(m, "ContarOcorrencias")) {
            *out = v_int(utf8_count_occurrences(valor, arg_texto(1, argc, args)));
        } else if (!strcmp(m, "EstaVazia")) {
            *out = v_bool(valor[0] == '\0');
        } else if (!strcmp(m, "Repete")) {
            const char *texto = valor;
            int64_t vezes = (int64_t)arg_num(1, argc, args);
            if (vezes < 0) vezes = 0;
            size_t tl = strlen(texto);
            size_t total = tl * (size_t)vezes + 1;
            char *buf = (char *)malloc(total);
            buf[0] = '\0';
            for (int64_t i = 0; i < vezes; i++) strcat(buf, texto);
            *out = v_text_move(buf);
        } else if (!strcmp(m, "Comparar")) {
            int c = strcmp(valor, arg_texto(1, argc, args));
            *out = v_int(c < 0 ? -1 : (c > 0 ? 1 : 0));
        } else if (!strcmp(m, "IgualIgnoreCase")) {
            *out = v_bool(ascii_case_equal(valor, arg_texto(1, argc, args)));
        } else if (!strcmp(m, "Remover")) {
            *out = v_text_move(texto_remover(valor, arg_int(1, argc, args),
                                             arg_int(2, argc, args)));
        } else if (!strcmp(m, "Inserir")) {
            *out = v_text_move(texto_inserir(valor, arg_int(1, argc, args),
                                             arg_texto(2, argc, args)));
        } else if (!strcmp(m, "NormalizarEspacos")) {
            *out = v_text_move(texto_normalizar_espacos(valor));
        } else if (!strcmp(m, "DividirPalavras")) {
            *out = texto_dividir_palavras(valor);
        } else if (!strcmp(m, "ContarPalavras")) {
            *out = v_int(texto_contar_palavras(valor));
        } else if (!strcmp(m, "LocalizarTodas")) {
            *out = texto_localizar_todas(valor, arg_texto(1, argc, args));
        } else {
            *out = v_nulo();
        }
        return;
    }

    /* Data estáticos */
    if (!strcmp(chave, "Data::Agora") || !strcmp(chave, "Data::Hoje")) {
        PorsolObj *d = obj_new_data();
        int a, me, di, h, mi, s;
        agora_decomposto(&a, &me, &di, &h, &mi, &s);
        if (!strcmp(chave, "Data::Hoje")) { h = mi = s = 0; }
        campo_set_int(d, "Ano", a);
        campo_set_int(d, "Mes", me);
        campo_set_int(d, "Dia", di);
        campo_set_int(d, "Hora", h);
        campo_set_int(d, "Minuto", mi);
        campo_set_int(d, "Segundo", s);
        *out = v_obj(d);
        return;
    }

    /* Guid */
    if (!strncmp(chave, "Guid::", 6)) {
        const char *m = chave + 6;
        if (!strcmp(m, "Vazio")) {
            *out = v_text("00000000-0000-0000-0000-000000000000");
        } else if (!strcmp(m, "Novo")) {
            static unsigned long long contador = 0;
            u64 seed = (u64)time(NULL) ^ (++contador * 0x9E3779B97F4A7C15ULL);
            u64 st = semente_aleatoria(seed);
            u64 a = proximo_aleatorio_u64(&st);
            u64 b = proximo_aleatorio_u64(&st);
            char g[40];
            snprintf(g, sizeof(g),
                     "%08llx-%04llx-%04llx-%04llx-%012llx",
                     (unsigned long long)(a >> 32),
                     (unsigned long long)((a >> 16) & 0xFFFF),
                     (unsigned long long)(a & 0xFFFF),
                     (unsigned long long)((b >> 48) & 0xFFFF),
                     (unsigned long long)(b & 0xFFFFFFFFFFFFULL));
            *out = v_text(g);
        } else if (!strcmp(m, "EhValido")) {
            const char *v = arg_texto(0, argc, args);
            int ok = strlen(v) == 36;
            for (int i = 0; ok && i < 36; i++) {
                int traco = (i == 8 || i == 13 || i == 18 || i == 23);
                if (traco) ok = v[i] == '-';
                else ok = isxdigit((unsigned char)v[i]) != 0;
            }
            *out = v_bool(ok);
        } else {
            *out = v_nulo();
        }
        return;
    }

    /* Arquivo */
    if (!strncmp(chave, "Arquivo::", 9)) {
        const char *m = chave + 9;
        const char *caminho = arg_texto(0, argc, args);
        if (!strcmp(m, "LerTexto")) {
            char *c = ler_arquivo_todo(caminho);
            *out = c ? v_text_move(c) : v_nulo();
        } else if (!strcmp(m, "EscreverTexto")) {
            FILE *f = fopen(caminho, "wb");
            if (f) {
                fputs(arg_texto(1, argc, args), f);
                fclose(f);
            }
            *out = v_nulo();
        } else if (!strcmp(m, "AdicionarTexto")) {
            FILE *f = fopen(caminho, "ab");
            if (f) {
                fputs(arg_texto(1, argc, args), f);
                fclose(f);
            }
            *out = v_nulo();
        } else if (!strcmp(m, "Existe")) {
            *out = v_bool(arquivo_existe(caminho));
        } else if (!strcmp(m, "Excluir")) {
            remove(caminho);
            *out = v_nulo();
        } else if (!strcmp(m, "Copiar")) {
            char *c = ler_arquivo_todo(caminho);
            if (c) {
                FILE *f = fopen(arg_texto(1, argc, args), "wb");
                if (f) { fputs(c, f); fclose(f); }
                free(c);
            }
            *out = v_nulo();
        } else if (!strcmp(m, "Mover")) {
            rename(caminho, arg_texto(1, argc, args));
            *out = v_nulo();
        } else if (!strcmp(m, "Tamanho")) {
            struct stat st;
            *out = stat(caminho, &st) == 0 ? v_int((int64_t)st.st_size) : v_nulo();
        } else if (!strcmp(m, "Criar")) {
            FILE *f = fopen(caminho, "ab");
            if (f) fclose(f);
            *out = v_nulo();
        } else if (!strcmp(m, "LerLinhas")) {
            char *c = ler_arquivo_todo(caminho);
            if (!c) { *out = v_nulo(); return; }
            PorsolValue arr = texto_dividir_linhas(c);
            free(c);
            *out = arr;
        } else {
            fprintf(stderr, "[aviso nativo] '%s' não implementado no runtime LLVM\n", chave);
            *out = v_nulo();
        }
        return;
    }

    /* Diretorio */
    if (!strncmp(chave, "Diretorio::", 11)) {
        const char *m = chave + 11;
        const char *caminho = arg_texto(0, argc, args);
        if (!strcmp(m, "Existe")) {
            *out = v_bool(diretorio_existe(caminho));
        } else if (!strcmp(m, "Criar")) {
#ifdef _WIN32
            _mkdir(caminho);
#else
            mkdir(caminho, 0777);
#endif
            *out = v_nulo();
        } else if (!strcmp(m, "Excluir")) {
            int rec = arg_int(1, argc, args) != 0;
#ifdef _WIN32
            char cmd[4200];
            snprintf(cmd, sizeof(cmd), rec ? "rmdir /s /q \"%s\"" : "rmdir \"%s\"", caminho);
            system(cmd);
#else
            if (rec) {
                char cmd[4200];
                snprintf(cmd, sizeof(cmd), "rm -rf -- '%s'", caminho);
                system(cmd);
            } else {
                rmdir(caminho);
            }
#endif
            *out = v_nulo();
        } else if (!strcmp(m, "ObterAtual")) {
            char buf[4096];
#ifdef _WIN32
            _getcwd(buf, sizeof(buf));
#else
            getcwd(buf, sizeof(buf));
#endif
            *out = v_text(buf);
        } else if (!strcmp(m, "DefinirAtual")) {
#ifdef _WIN32
            _chdir(caminho);
#else
            chdir(caminho);
#endif
            *out = v_nulo();
        } else if (!strcmp(m, "ObterArquivos") || !strcmp(m, "ObterDiretorios")) {
            int count = 0;
            int somente_arquivos = !strcmp(m, "ObterArquivos");
            char **nomes = listar_diretorio(caminho, somente_arquivos, &count);
            /* array de TEXTO */
            PorsolArrayHdr *h = (PorsolArrayHdr *)calloc(1, sizeof(PorsolArrayHdr));
            h->len = count;
            h->elem_kind = PORSOL_TEXTO;
            h->data = malloc(sizeof(char *) * (count > 0 ? count : 1));
            memcpy(h->data, nomes, sizeof(char *) * count);
            free(nomes);
            *out = v_array(h);
        } else {
            *out = v_nulo();
        }
        return;
    }

    /* Caminho */
    if (!strncmp(chave, "Caminho::", 9)) {
        const char *m = chave + 9;
        const char *caminho = arg_texto(0, argc, args);
        if (!strcmp(m, "Combinar")) {
            const char *a = caminho;
            const char *b = arg_texto(1, argc, args);
            size_t la = strlen(a);
            int termina_sep = la && (a[la - 1] == '/' || a[la - 1] == '\\');
            char buf[4096];
            snprintf(buf, sizeof(buf), "%s%s%s", a, termina_sep ? "" : "/", b);
            *out = v_text(buf);
        } else if (!strcmp(m, "NomeArquivo")) {
            const char *base = strrchr(caminho, '/');
            const char *b2 = strrchr(caminho, '\\');
            if (b2 > base) base = b2;
            *out = v_text(base ? base + 1 : caminho);
        } else if (!strcmp(m, "NomeSemExtensao")) {
            const char *base = strrchr(caminho, '/');
            const char *b2 = strrchr(caminho, '\\');
            if (b2 > base) base = b2;
            const char *nome = base ? base + 1 : caminho;
            char tmp[1024];
            snprintf(tmp, sizeof(tmp), "%s", nome);
            char *dot = strrchr(tmp, '.');
            if (dot && dot != tmp) *dot = '\0';
            *out = v_text(tmp);
        } else if (!strcmp(m, "DiretorioPai")) {
            char tmp[4096];
            snprintf(tmp, sizeof(tmp), "%s", caminho);
            char *slash = strrchr(tmp, '/');
            char *bslash = strrchr(tmp, '\\');
            char *ult = (slash > bslash) ? slash : bslash;
            if (ult) *ult = '\0'; else snprintf(tmp, sizeof(tmp), "");
            *out = v_text(tmp);
        } else if (!strcmp(m, "Extensao")) {
            const char *base = strrchr(caminho, '/');
            const char *b2 = strrchr(caminho, '\\');
            if (b2 > base) base = b2;
            const char *nome = base ? base + 1 : caminho;
            const char *dot = strrchr(nome, '.');
            *out = v_text(dot ? dot : "");
        } else if (!strcmp(m, "Absoluto")) {
#ifdef _WIN32
            char buf[4096];
            if (_fullpath(buf, caminho, sizeof(buf))) {
                *out = v_text(buf);
            } else {
                *out = v_text(caminho);
            }
#else
            if (caminho[0] == '/') {
                *out = v_text(caminho);
            } else {
                char cwd[4096];
                if (getcwd(cwd, sizeof(cwd))) {
                    char buf[8192];
                    snprintf(buf, sizeof(buf), "%s/%s", cwd, caminho);
                    *out = v_text(buf);
                } else {
                    *out = v_text(caminho);
                }
            }
#endif
        } else if (!strcmp(m, "Separadores")) {
            *out = v_text("/\\");
        } else if (!strcmp(m, "Separador")) {
            char sep[2] = { PATH_SEP, '\0' };
            *out = v_text(sep);
        } else {
            *out = v_nulo();
        }
        return;
    }

    /* Leitor / InformacaoArquivo — criação */
    if (!strcmp(chave, "Leitor::Abrir")) {
        *out = leitor_novo(arg_texto(0, argc, args));
        return;
    }
    if (!strcmp(chave, "InformacaoArquivo::Abrir")) {
        *out = info_arquivo_novo(arg_texto(0, argc, args));
        return;
    }

    /* Threading estáticos */
    if (!strcmp(chave, "Tarefa::Atraso") || !strcmp(chave, "Temporizador::Aguardar")) {
        long ms = (long)arg_num(0, argc, args);
        if (ms > 0) {
#ifdef _WIN32
            Sleep((DWORD)ms);
#else
            struct timespec ts = { ms / 1000, (ms % 1000) * 1000000L };
            nanosleep(&ts, NULL);
#endif
        }
        *out = v_nulo();
        return;
    }
    if (!strcmp(chave, "Tarefa::DeResultado")) {
        *out = argc > 0 ? args[0] : v_nulo();
        return;
    }
    if (!strcmp(chave, "PoolTarefas::ObterMaximoParalelismo")) {
        *out = v_int(4);
        return;
    }
    if (!strcmp(chave, "PoolTarefas::DefinirMaximoParalelismo")) {
        *out = v_nulo();
        return;
    }

    /* Ambiente */
    if (!strncmp(chave, "Ambiente::", 10)) {
        const char *m = chave + 10;
        if (!strcmp(m, "Argumentos")) {
            /* argv do processo: saltamos argv[0] como o interpretador */
            PVec v;
            pvec_init(&v);
#ifdef _WIN32
            int wargc = 0;
            LPWSTR *wargv = CommandLineToArgvW(GetCommandLineW(), &wargc);
            if (wargv) {
                for (int i = 1; i < wargc; i++) {
                    int sz = WideCharToMultiByte(CP_UTF8, 0, wargv[i], -1, NULL, 0, NULL, NULL);
                    char *buf = (char *)malloc((size_t)sz);
                    WideCharToMultiByte(CP_UTF8, 0, wargv[i], -1, buf, sz, NULL, NULL);
                    PorsolValue t = v_nulo();
                    t.kind = PORSOL_TEXTO;
                    t.s = buf;
                    pvec_push(&v, t);
                }
                LocalFree(wargv);
            }
#else
            /* POSIX: /proc/self/cmdline (fallback portátil) */
            FILE *cf = fopen("/proc/self/cmdline", "rb");
            if (cf) {
                char *buf = (char *)malloc(65536);
                size_t rd = fread(buf, 1, 65535, cf);
                fclose(cf);
                buf[rd] = '\0';
                /* argv[0]\0arg1\0arg2\0...\0 — saltamos o primeiro */
                char *p = buf;
                while (*p) p += strlen(p) + 1; /* fim de argv[0] */
                while (*p) {
                    char *arg = p;
                    PorsolValue t = v_nulo();
                    t.kind = PORSOL_TEXTO;
                    t.s = porsol_strdup(arg);
                    pvec_push(&v, t);
                    p += strlen(p) + 1;
                }
                free(buf);
            }
#endif
            *out = pvec_para_array_val(&v);
            pvec_free(&v);
            return;
        }
        if (!strcmp(m, "VariavelAmbiente")) {
            const char *val = getenv(arg_texto(0, argc, args));
            *out = v_text(val ? val : "");
            return;
        }
        if (!strcmp(m, "DefinirVariavelAmbiente")) {
#ifdef _WIN32
            _putenv_s(arg_texto(0, argc, args), arg_texto(1, argc, args));
#else
            setenv(arg_texto(0, argc, args), arg_texto(1, argc, args), 1);
#endif
            *out = v_nulo();
            return;
        }
        if (!strcmp(m, "NomeComputador")) {
            const char *nome = getenv("COMPUTERNAME");
            if (!nome) nome = getenv("HOSTNAME");
            if (!nome) nome = "desconhecido";
            *out = v_text(nome);
            return;
        }
        *out = v_nulo();
        return;
    }

    /* Conversao */
    if (!strncmp(chave, "Conversao::", 11)) {
        const char *m = chave + 11;
        const char *txt = arg_texto(0, argc, args);
        /* trim simples */
        while (*txt == ' ' || *txt == '\t' || *txt == '\r' || *txt == '\n') txt++;
        char tmp[256];
        snprintf(tmp, sizeof(tmp), "%s", txt);
        char *end = tmp + strlen(tmp);
        while (end > tmp && (end[-1] == ' ' || end[-1] == '\t' ||
               end[-1] == '\r' || end[-1] == '\n')) *--end = '\0';
        if (!strcmp(m, "ParaInteiro")) {
            char *e = NULL;
            long long n = strtoll(tmp, &e, 10);
            if (e == tmp) {
                double d = strtod(tmp, &e);
                n = (e != tmp) ? (long long)d : 0;
            }
            *out = v_int(n);
        } else if (!strcmp(m, "ParaFlutuante") || !strcmp(m, "ParaDuplo") ||
                   !strcmp(m, "ParaDecimal")) {
            char *e = NULL;
            double d = strtod(tmp, &e);
            if (e == tmp) d = 0.0;
            *out = v_dbl(d); /* Decimal é transportado como texto no LLVM; duplo cobre flutuante */
        } else if (!strcmp(m, "ParaBooleano")) {
            char low[256];
            snprintf(low, sizeof(low), "%s", tmp);
            for (char *p = low; *p; p++) *p = (char)ascii_lower((unsigned char)*p);
            int b;
            if (!strcmp(low, "verdadeiro") || !strcmp(low, "true") ||
                !strcmp(low, "1") || !strcmp(low, "sim") || !strcmp(low, "yes")) {
                b = 1;
            } else if (!strcmp(low, "falso") || !strcmp(low, "false") ||
                       !strcmp(low, "0") || !strcmp(low, "nao") ||
                       !strcmp(low, "não") || !strcmp(low, "no")) {
                b = 0;
            } else {
                char *e = NULL;
                double d = strtod(tmp, &e);
                b = (e != tmp) ? (d != 0.0) : 0;
            }
            *out = v_bool(b);
        } else if (!strcmp(m, "EhNumero")) {
            char *e = NULL;
            (void)strtod(tmp, &e);
            *out = v_bool(e != tmp && *e == '\0' && tmp[0] != '\0');
        } else {
            *out = v_nulo();
        }
        return;
    }

    /* Horario — fábricas estáticas (devolvem objeto Horario) */
    if (!strncmp(chave, "Horario::De", 11)) {
        const char *m = chave + 11;
        int64_t n = arg_int(0, argc, args);
        int64_t fator = 1;
        if (!strcmp(m, "Minutos")) fator = 60;
        else if (!strcmp(m, "Horas")) fator = 3600;
        else if (!strcmp(m, "Dias")) fator = 86400;
        else if (strcmp(m, "Segundos")) { *out = v_nulo(); return; }
        *out = v_obj(obj_new_horario(n * fator));
        return;
    }

    /* Codificacao (UTF-8) */
    if (!strncmp(chave, "Codificacao::", 13)) {
        const char *m = chave + 13;
        if (!strcmp(m, "ParaBytes")) {
            const char *s = arg_texto(0, argc, args);
            PVec v;
            pvec_init(&v);
            for (const unsigned char *p = (const unsigned char *)s; *p; p++) {
                PorsolValue b = v_nulo();
                b.kind = PORSOL_INTEIRO;
                b.i = *p;
                pvec_push(&v, b);
            }
            *out = pvec_para_array_val(&v);
            pvec_free(&v);
        } else if (!strcmp(m, "ParaTexto")) {
            PVec itens = itens_de_lista(argc > 0 ? &args[0] : NULL);
            size_t n = itens.n;
            char *buf = (char *)malloc(n + 1);
            for (size_t i = 0; i < n; i++) {
                int64_t b = itens.itens[i].kind == PORSOL_INTEIRO ? itens.itens[i].i
                            : itens.itens[i].kind == PORSOL_DOUBLE ? (int64_t)itens.itens[i].d
                            : 0;
                if (b < 0) b = 0;
                if (b > 255) b = 255;
                buf[i] = (char)b;
            }
            buf[n] = '\0';
            *out = v_text_move(buf);
            pvec_free(&itens);
        } else if (!strcmp(m, "TamanhoBytes")) {
            *out = v_int((int64_t)strlen(arg_texto(0, argc, args)));
        } else if (!strcmp(m, "Base64Codificar")) {
            const unsigned char *s = (const unsigned char *)arg_texto(0, argc, args);
            *out = v_text_move(base64_codificar(s, strlen((const char *)s)));
        } else if (!strcmp(m, "Base64Decodificar")) {
            size_t bn = 0;
            unsigned char *bytes = base64_decodificar(arg_texto(0, argc, args), &bn);
            if (!bytes) {
                *out = v_text("");
            } else {
                char *buf = (char *)malloc(bn + 1);
                memcpy(buf, bytes, bn);
                buf[bn] = '\0';
                free(bytes);
                *out = v_text_move(buf);
            }
        } else if (!strcmp(m, "ParaHex")) {
            PVec itens = itens_de_lista(argc > 0 ? &args[0] : NULL);
            size_t out_n = itens.n * 2;
            char *buf = (char *)malloc(out_n + 1);
            static const char hexd[] = "0123456789abcdef";
            for (size_t i = 0; i < itens.n; i++) {
                int64_t b = itens.itens[i].kind == PORSOL_INTEIRO ? itens.itens[i].i
                            : itens.itens[i].kind == PORSOL_DOUBLE ? (int64_t)itens.itens[i].d
                            : 0;
                if (b < 0) b = 0;
                if (b > 255) b = 255;
                buf[i * 2] = hexd[(b >> 4) & 0xF];
                buf[i * 2 + 1] = hexd[b & 0xF];
            }
            buf[out_n] = '\0';
            *out = v_text_move(buf);
            pvec_free(&itens);
        } else if (!strcmp(m, "DeHex")) {
            const char *s = arg_texto(0, argc, args);
            /* filtra whitespace */
            size_t n = strlen(s);
            unsigned char *limpo = (unsigned char *)malloc(n + 1);
            size_t ln = 0;
            for (size_t i = 0; i < n; i++) {
                unsigned char c = (unsigned char)s[i];
                if (!isspace(c)) limpo[ln++] = c;
            }
            PVec v;
            pvec_init(&v);
            int valido = (ln % 2 == 0);
            if (valido) {
                for (size_t i = 0; i < ln; i += 2) {
                    int hi = hex_valor(limpo[i]);
                    int lo = hex_valor(limpo[i + 1]);
                    if (hi < 0 || lo < 0) { valido = 0; break; }
                    PorsolValue b = v_nulo();
                    b.kind = PORSOL_INTEIRO;
                    b.i = (hi << 4) | lo;
                    pvec_push(&v, b);
                }
            }
            free(limpo);
            if (!valido) {
                pvec_free(&v);
                pvec_init(&v);
            }
            *out = pvec_para_array_val(&v);
            pvec_free(&v);
        } else {
            *out = v_nulo();
        }
        return;
    }

    /* Regex — estáticos */
    if (!strncmp(chave, "Regex::", 7)) {
        const char *m = chave + 7;
        if (!strcmp(m, "Escape")) {
            *out = v_text_move(regex_escape(arg_texto(0, argc, args)));
            return;
        }
        if (!strcmp(m, "EhValido")) {
            *out = v_bool(regex_padrao_valido(arg_texto(0, argc, args)));
            return;
        }
        *out = v_nulo();
        return;
    }

    /* Json */
    if (!strncmp(chave, "Json::", 6)) {
        const char *m = chave + 6;
        const char *txt = arg_texto(0, argc, args);
        if (!strcmp(m, "Serializar")) {
            PorsolValue v = argc > 0 ? args[0] : v_nulo();
            PorsolValue j = json_serializar_valor(&v);
            *out = j;
        } else if (!strcmp(m, "ValidarJson")) {
            *out = v_bool(json_validar(txt));
        } else if (!strcmp(m, "Formatar")) {
            char *f = json_formatar(txt);
            *out = f ? v_text_move(f) : v_text(txt);
        } else if (!strcmp(m, "Desserializar")) {
            int ok = 0;
            PorsolValue v = json_desserializar(txt, &ok);
            *out = ok ? v : v_nulo();
        } else if (!strcmp(m, "SerializarFormatado")) {
            PorsolValue v = argc > 0 ? args[0] : v_nulo();
            *out = json_serializar_valor_pretty(&v);
        } else if (!strcmp(m, "TipoDe")) {
            *out = v_text(json_tipo_de(txt));
        } else {
            *out = v_nulo();
        }
        return;
    }

    /* Rede — estáticos */
    if (!strcmp(chave, "Dns::Resolver") || !strcmp(chave, "Dns::ResolverTodos")) {
        int count = 0;
        int todos = !strcmp(chave, "Dns::ResolverTodos");
        char *ips = dns_resolver_host(arg_texto(0, argc, args), todos, &count);
        if (todos) {
            /* ips é uma lista separada por '\n' — converte para array de texto */
            PVec v;
            pvec_init(&v);
            if (ips) {
#ifdef _WIN32
                char *save = NULL;
                for (char *tok = strtok_s(ips, "\n", &save); tok;
                     tok = strtok_s(NULL, "\n", &save)) {
#else
                char *save = NULL;
                for (char *tok = strtok_r(ips, "\n", &save); tok;
                     tok = strtok_r(NULL, "\n", &save)) {
#endif
                    PorsolValue t = v_nulo();
                    t.kind = PORSOL_TEXTO;
                    t.s = porsol_strdup(tok);
                    pvec_push(&v, t);
                }
                free(ips);
            }
            *out = pvec_para_array_val(&v);
            pvec_free(&v);
        } else {
            *out = v_text(ips ? ips : "");
            free(ips);
        }
        return;
    }
    if (!strcmp(chave, "SocketTcp::Conectar")) {
        const char *host = arg_texto(0, argc, args);
        int porta = (int)arg_num(1, argc, args);
        int fd = -1;
        if (winsock_init() == 0) {
            struct addrinfo hints, *res = NULL;
            memset(&hints, 0, sizeof(hints));
            hints.ai_family = AF_UNSPEC;
            hints.ai_socktype = SOCK_STREAM;
            char portabuf[16];
            snprintf(portabuf, sizeof(portabuf), "%d", porta);
            if (getaddrinfo(host, portabuf, &hints, &res) == 0 && res) {
                fd = socket(res->ai_family, res->ai_socktype, res->ai_protocol);
                if (fd >= 0) {
#ifdef _WIN32
                    u_long one = 1;
                    ioctlsocket((SOCKET)fd, FIONBIO, &one);
#endif
                    if (connect(fd, res->ai_addr, (int)res->ai_addrlen) != 0) {
                        close_sock(fd);
                        fd = -1;
                    }
                }
                freeaddrinfo(res);
            }
        }
        if (fd < 0) {
            fprintf(stderr, "SocketTcp::Conectar: falha ao conectar em %s:%d\n", host, porta);
            *out = v_nulo();
        } else {
            *out = v_obj(socket_tcp_novo(fd));
        }
        return;
    }
    if (!strcmp(chave, "ServidorTcp::Ouvir")) {
        int porta = (int)arg_num(0, argc, args);
        int fd = -1;
        if (winsock_init() == 0) {
            fd = socket(AF_INET, SOCK_STREAM, 0);
            if (fd >= 0) {
                int opt = 1;
                setsockopt(fd, SOL_SOCKET, SO_REUSEADDR, (const char *)&opt, sizeof(opt));
                struct sockaddr_in addr;
                memset(&addr, 0, sizeof(addr));
                addr.sin_family = AF_INET;
                addr.sin_addr.s_addr = htonl(INADDR_ANY);
                addr.sin_port = htons((uint16_t)porta);
                if (bind(fd, (struct sockaddr *)&addr, sizeof(addr)) != 0 ||
                    listen(fd, 8) != 0) {
                    close_sock(fd);
                    fd = -1;
                }
            }
        }
        if (fd < 0) {
            fprintf(stderr, "ServidorTcp::Ouvir: falha ao ouvir na porta %d\n", porta);
            *out = v_nulo();
        } else {
            *out = v_obj(servidor_tcp_novo(fd));
        }
        return;
    }

    fprintf(stderr, "[aviso nativo] Função estática '%s' não implementada no runtime LLVM\n", chave);
    *out = v_nulo();
}

/* ---------- instância ---------- */

static void despachar_instancia(const char *chave, PorsolObj *este, int argc,
                                const PorsolValue *args, PorsolValue *out) {
    /* args aqui são args[1..] da chamada original (sem self) — convenção:
       passamos argc/args já deslocados pelo dispatcher principal. */
    int ai = 0; /* índice do primeiro arg real */

    /* Propriedades Data */
    if (classe_eh(este, "Data")) {
        static const char *props[] = {"Ano", "Mes", "Dia", "Hora", "Minuto", "Segundo"};
        const char *m = strchr(chave, ':');
        m = m ? m + 2 : chave;
        if (!strncmp(m, "set_", 4)) {
            const char *prop = m + 4;
            for (int i = 0; i < 6; i++) {
                if (!strcmp(prop, props[i])) {
                    campo_set_int(este, props[i], (int)arg_num(ai, argc, args));
                    *out = v_nulo();
                    return;
                }
            }
            *out = v_nulo();
            return;
        }
        for (int i = 0; i < 6; i++) {
            if (!strcmp(m, props[i])) {
                *out = v_int(campo_int(este, props[i], 0));
                return;
            }
        }
        if (!strcmp(m, "Formatar")) {
            char buf[512];
            data_formatar(este, arg_texto(ai, argc, args), buf, sizeof(buf));
            *out = v_text(buf);
            return;
        }
        if (!strcmp(m, "ParaTexto")) {
            char buf[64];
            snprintf(buf, sizeof(buf), "%02d/%02d/%04d %02d:%02d:%02d",
                     campo_int(este, "Dia", 1), campo_int(este, "Mes", 1),
                     campo_int(este, "Ano", 0), campo_int(este, "Hora", 0),
                     campo_int(este, "Minuto", 0), campo_int(este, "Segundo", 0));
            *out = v_text(buf);
            return;
        }
        if (!strcmp(m, "Igual")) {
            PorsolObj *outro = NULL;
            if (argc > ai && args[ai].kind == PORSOL_OBJETO) outro = (PorsolObj *)args[ai].o;
            int eq = 0;
            if (outro) {
                eq = 1;
                for (int i = 0; i < 6; i++) {
                    const char *p = (const char *[]){"Ano","Mes","Dia","Hora","Minuto","Segundo"}[i];
                    if (campo_int(este, p, -1) != campo_int(outro, p, -2)) { eq = 0; break; }
                }
            }
            *out = v_bool(eq);
            return;
        }
    }

    /* Lista / Fila / Pilha / Conjunto compartilham PVec */
    if (classe_eh(este, "Lista") || classe_eh(este, "Fila") ||
        classe_eh(este, "Pilha") || classe_eh(este, "Conjunto")) {
        PVec *v = obj_vetor(este);
        const char *m = strchr(chave, ':');
        m = m ? m + 2 : chave;
        int eh_lista = classe_eh(este, "Lista");
        int eh_conj = classe_eh(este, "Conjunto");

        /* propriedades Tamanho/Capacidade */
        if (!strcmp(m, "Tamanho") || !strcmp(m, "Contagem")) {
            *out = v_int(v ? v->n : 0);
            return;
        }
        if (eh_lista && !strcmp(m, "Capacidade")) {
            *out = v_int(v ? v->cap : 0);
            return;
        }

        if (!strcmp(m, "Adicionar") || !strcmp(m, "Enfileirar") ||
            !strcmp(m, "Empilhar")) {
            if (v && argc > ai) {
                PorsolValue item = args[ai];
                /* Conjunto::Adicionar retorna bool */
                if (eh_conj) {
                    int existe = 0;
                    for (int i = 0; i < v->n; i++)
                        if (valor_igual(&v->itens[i], &item)) { existe = 1; break; }
                    if (!existe) pvec_push(v, item);
                    *out = v_bool(!existe);
                } else {
                    pvec_push(v, item);
                    *out = v_nulo();
                }
            } else {
                *out = eh_conj ? v_bool(0) : v_nulo();
            }
            return;
        }
        if (!strcmp(m, "Obter")) {
            int idx = (int)arg_int(ai, argc, args);
            if (v && idx >= 0 && idx < v->n) {
                *out = v->itens[idx];
            } else {
                *out = v_nulo();
            }
            return;
        }
        if (!strcmp(m, "Definir")) {
            int idx = (int)arg_int(ai, argc, args);
            if (v && idx >= 0 && idx < v->n && argc > ai + 1) {
                v->itens[idx] = args[ai + 1];
            }
            *out = v_nulo();
            return;
        }
        if (!strcmp(m, "RemoverEm")) {
            int idx = (int)arg_int(ai, argc, args);
            if (v && idx >= 0 && idx < v->n) {
                memmove(&v->itens[idx], &v->itens[idx + 1],
                        sizeof(PorsolValue) * (v->n - idx - 1));
                v->n--;
            }
            *out = v_nulo();
            return;
        }
        if (!strcmp(m, "Remover")) {
            if (v && argc > ai) {
                for (int i = 0; i < v->n; i++) {
                    if (valor_igual(&v->itens[i], &args[ai])) {
                        if (eh_conj) {
                            memmove(&v->itens[i], &v->itens[i + 1],
                                    sizeof(PorsolValue) * (v->n - i - 1));
                            v->n--;
                        } else {
                            memmove(&v->itens[i], &v->itens[i + 1],
                                    sizeof(PorsolValue) * (v->n - i - 1));
                            v->n--;
                        }
                        *out = v_bool(1);
                        return;
                    }
                }
            }
            *out = v_bool(0);
            return;
        }
        if (!strcmp(m, "Contem")) {
            int achou = 0;
            if (v && argc > ai) {
                for (int i = 0; i < v->n; i++)
                    if (valor_igual(&v->itens[i], &args[ai])) { achou = 1; break; }
            }
            *out = v_bool(achou);
            return;
        }
        if (!strcmp(m, "Limpar")) {
            if (v) v->n = 0;
            *out = v_nulo();
            return;
        }
        if (!strcmp(m, "IndiceDe")) {
            int idx = -1;
            if (v && argc > ai) {
                for (int i = 0; i < v->n; i++)
                    if (valor_igual(&v->itens[i], &args[ai])) { idx = i; break; }
            }
            *out = v_int(idx);
            return;
        }
        if (!strcmp(m, "EstaVazia")) {
            *out = v_bool(!v || v->n == 0);
            return;
        }
        if (!strcmp(m, "ObterPrimeiro") || !strcmp(m, "Espiar")) {
            *out = (v && v->n > 0) ? v->itens[0] : v_nulo();
            return;
        }
        if (!strcmp(m, "ObterUltimo") || !strcmp(m, "EspiarTopo")) {
            *out = (v && v->n > 0) ? v->itens[v->n - 1] : v_nulo();
            return;
        }
        if (!strcmp(m, "Desenfileirar")) {
            if (v && v->n > 0) {
                PorsolValue item = v->itens[0];
                memmove(&v->itens[0], &v->itens[1], sizeof(PorsolValue) * (v->n - 1));
                v->n--;
                *out = item;
            } else {
                *out = v_nulo();
            }
            return;
        }
        if (!strcmp(m, "Desempilhar")) {
            if (v && v->n > 0) {
                *out = v->itens[--v->n];
            } else {
                *out = v_nulo();
            }
            return;
        }
        if (!strcmp(m, "Inverter") && eh_lista) {
            if (v) {
                for (int i = 0, j = v->n - 1; i < j; i++, j--) {
                    PorsolValue t = v->itens[i];
                    v->itens[i] = v->itens[j];
                    v->itens[j] = t;
                }
            }
            *out = v_nulo();
            return;
        }
        if (!strcmp(m, "Ordenar") && eh_lista) {
            if (v && v->n > 1) {
                /* insertion sort simples por valor_cmp */
                for (int i = 1; i < v->n; i++) {
                    PorsolValue key = v->itens[i];
                    int j = i - 1;
                    while (j >= 0 && valor_cmp(&v->itens[j], &key) > 0) {
                        v->itens[j + 1] = v->itens[j];
                        j--;
                    }
                    v->itens[j + 1] = key;
                }
            }
            *out = v_nulo();
            return;
        }
        if (!strcmp(m, "Recortar") && eh_lista) {
            int ini = (int)arg_int(ai, argc, args);
            int fim = (int)arg_int(ai + 1, argc, args);
            PorsolObj *sub = obj_new_lista();
            PVec *sv = obj_vetor(sub);
            if (v) {
                if (ini < 0) ini = 0;
                if (fim > v->n) fim = v->n;
                for (int i = ini; i < fim && i < v->n; i++)
                    if (i >= 0) pvec_push(sv, v->itens[i]);
            }
            *out = v_obj(sub);
            return;
        }
        if (!strcmp(m, "OrdenarDecrescente") && eh_lista) {
            if (v && v->n > 1) {
                for (int i = 1; i < v->n; i++) {
                    PorsolValue key = v->itens[i];
                    int j = i - 1;
                    while (j >= 0 && valor_cmp(&v->itens[j], &key) < 0) {
                        v->itens[j + 1] = v->itens[j];
                        j--;
                    }
                    v->itens[j + 1] = key;
                }
            }
            *out = v_nulo();
            return;
        }
        if (!strcmp(m, "Inserir") && eh_lista) {
            int64_t indice = arg_int(ai, argc, args);
            if (v && argc > ai + 1) {
                int64_t len = v->n;
                if (indice < 0) indice = 0;
                if (indice > len) indice = len;
                pvec_reserve(v, v->n + 1);
                memmove(&v->itens[indice + 1], &v->itens[indice],
                        sizeof(PorsolValue) * (v->n - indice));
                v->itens[indice] = args[ai + 1];
                v->n++;
            }
            *out = v_nulo();
            return;
        }
        if (!strcmp(m, "RemoverUltimo") && eh_lista) {
            if (v && v->n > 0) {
                *out = v->itens[--v->n];
            } else {
                *out = v_nulo();
            }
            return;
        }
        if (!strcmp(m, "AdicionarSeAusente") && eh_lista) {
            int existe = 0;
            if (v && argc > ai) {
                for (int i = 0; i < v->n; i++)
                    if (valor_igual(&v->itens[i], &args[ai])) { existe = 1; break; }
                if (!existe) pvec_push(v, args[ai]);
            }
            *out = v_bool(!existe);
            return;
        }
        /* ParaArray: Lista/Fila/Pilha/Conjunto -> array tipado */
        if (!strcmp(m, "ParaArray")) {
            PVec tmp;
            pvec_init(&tmp);
            if (v) {
                for (int i = 0; i < v->n; i++) pvec_push(&tmp, v->itens[i]);
            }
            *out = pvec_para_array_val(&tmp);
            pvec_free(&tmp);
            return;
        }
        /* ParaLista: Fila/Pilha -> objeto Lista com os mesmos itens */
        if (!strcmp(m, "ParaLista") && !eh_lista && !eh_conj) {
            PVec tmp;
            pvec_init(&tmp);
            if (v) {
                for (int i = 0; i < v->n; i++) pvec_push(&tmp, v->itens[i]);
            }
            *out = v_obj(obj_new_colecao("Lista", "Sistema.Colecoes.Lista", &tmp));
            pvec_free(&tmp);
            return;
        }
        /* Conjunto: Uniao/Intersecao/Diferenca -> novo Conjunto */
        if (eh_conj && (!strcmp(m, "Uniao") || !strcmp(m, "Intersecao") ||
                        !strcmp(m, "Diferenca"))) {
            PVec outro;
            pvec_de_arg(&outro, argc > ai ? &args[ai] : NULL);
            PVec res;
            pvec_init(&res);
            if (v) {
                if (!strcmp(m, "Uniao")) {
                    for (int i = 0; i < v->n; i++) pvec_push(&res, v->itens[i]);
                    for (int i = 0; i < outro.n; i++) {
                        int existe = 0;
                        for (int j = 0; j < res.n; j++)
                            if (valor_igual(&res.itens[j], &outro.itens[i])) { existe = 1; break; }
                        if (!existe) pvec_push(&res, outro.itens[i]);
                    }
                } else if (!strcmp(m, "Intersecao")) {
                    for (int i = 0; i < v->n; i++) {
                        for (int j = 0; j < outro.n; j++)
                            if (valor_igual(&v->itens[i], &outro.itens[j])) { pvec_push(&res, v->itens[i]); break; }
                    }
                } else { /* Diferenca */
                    for (int i = 0; i < v->n; i++) {
                        int dentro = 0;
                        for (int j = 0; j < outro.n; j++)
                            if (valor_igual(&v->itens[i], &outro.itens[j])) { dentro = 1; break; }
                        if (!dentro) pvec_push(&res, v->itens[i]);
                    }
                }
            }
            *out = v_obj(obj_new_colecao("Conjunto", "Sistema.Colecoes.Conjunto", &res));
            pvec_free(&res);
            pvec_free(&outro);
            return;
        }
    }

    /* Dicionario */
    if (classe_eh(este, "Dicionario")) {
        PDict *d = obj_dict(este);
        const char *m = strchr(chave, ':');
        m = m ? m + 2 : chave;
        if (!strcmp(m, "Tamanho") || !strcmp(m, "Contagem")) {
            *out = v_int(d ? d->n : 0);
            return;
        }
        if (!strcmp(m, "Adicionar")) {
            if (d && argc > ai + 1) pdict_definir(d, args[ai], args[ai + 1]);
            *out = v_nulo();
            return;
        }
        if (!strcmp(m, "Obter") || !strcmp(m, "TentarObter")) {
            PorsolValue r = v_nulo();
            if (d && argc > ai) {
                int i = pdict_encontrar(d, &args[ai]);
                if (i >= 0) r = d->itens[i].valor;
            }
            *out = r;
            return;
        }
        if (!strcmp(m, "ContemChave")) {
            int achou = 0;
            if (d && argc > ai) achou = pdict_encontrar(d, &args[ai]) >= 0;
            *out = v_bool(achou);
            return;
        }
        if (!strcmp(m, "ContemValor")) {
            int achou = 0;
            if (d && argc > ai) {
                for (int i = 0; i < d->n; i++)
                    if (valor_igual(&d->itens[i].valor, &args[ai])) { achou = 1; break; }
            }
            *out = v_bool(achou);
            return;
        }
        if (!strcmp(m, "Remover")) {
            int achou = 0;
            if (d && argc > ai) {
                int i = pdict_encontrar(d, &args[ai]);
                if (i >= 0) { pdict_remover(d, i); achou = 1; }
            }
            *out = v_bool(achou);
            return;
        }
        if (!strcmp(m, "Limpar")) {
            if (d) d->n = 0;
            *out = v_nulo();
            return;
        }
        if (!strcmp(m, "Chaves") || !strcmp(m, "Valores")) {
            PorsolObj *lista = obj_new_lista();
            PVec *lv = obj_vetor(lista);
            int extrair_valor = !strcmp(m, "Valores");
            if (d) {
                for (int i = 0; i < d->n; i++)
                    pvec_push(lv, extrair_valor ? d->itens[i].valor : d->itens[i].chave);
            }
            *out = v_obj(lista);
            return;
        }
    }

    /* ConstrutorTexto */
    if (classe_eh(este, "ConstrutorTexto")) {
        TxtBuf *t = obj_txtbuf(este);
        const char *m = strchr(chave, ':');
        m = m ? m + 2 : chave;
        if (!strcmp(m, "Anexar")) {
            if (t) txtbuf_append(t, arg_texto(ai, argc, args));
            *out = v_nulo();
            return;
        }
        if (!strcmp(m, "AnexarLinha")) {
            if (t) {
                txtbuf_append(t, arg_texto(ai, argc, args));
                txtbuf_append(t, "\n");
            }
            *out = v_nulo();
            return;
        }
        if (!strcmp(m, "AnexarValor")) {
            if (t) txtbuf_append(t, texto_de(&args[ai]));
            *out = v_nulo();
            return;
        }
        if (!strcmp(m, "AnexarNovaLinha")) {
            if (t) txtbuf_append(t, "\n");
            *out = v_nulo();
            return;
        }
        if (!strcmp(m, "Inserir")) {
            if (t) {
                const char *atual = t->buf ? t->buf : "";
                char *novo = texto_inserir(atual, arg_int(ai, argc, args),
                                           arg_texto(ai + 1, argc, args));
                txtbuf_set(t, novo);
            }
            *out = v_nulo();
            return;
        }
        if (!strcmp(m, "Remover")) {
            if (t) {
                const char *atual = t->buf ? t->buf : "";
                char *novo = texto_remover(atual, arg_int(ai, argc, args),
                                           arg_int(ai + 1, argc, args));
                txtbuf_set(t, novo);
            }
            *out = v_nulo();
            return;
        }
        if (!strcmp(m, "IndiceDe")) {
            const char *atual = (t && t->buf) ? t->buf : "";
            int64_t byte_idx = utf8_find(atual, arg_texto(ai, argc, args), 0);
            *out = v_int(byte_idx >= 0
                             ? utf8_count_chars(atual, 0, (size_t)byte_idx)
                             : -1);
            return;
        }
        if (!strcmp(m, "Subtexto")) {
            const char *atual = (t && t->buf) ? t->buf : "";
            int64_t ini = arg_int(ai, argc, args);
            int64_t comp = arg_int(ai + 1, argc, args);
            int64_t len = utf8_len(atual);
            if (ini < 0) ini = 0;
            if (ini > len) ini = len;
            int64_t fim = comp < 0 ? len : ini + comp;
            if (fim > len) fim = len;
            size_t b_ini = utf8_char_to_byte(atual, ini);
            size_t b_fim = utf8_char_to_byte(atual, fim);
            *out = v_text_move(porsol_strndup(atual + b_ini, b_fim - b_ini));
            return;
        }
        if (!strcmp(m, "ParaTexto")) {
            *out = v_text(t && t->buf ? t->buf : "");
            return;
        }
        if (!strcmp(m, "Limpar")) {
            if (t) { t->len = 0; if (t->buf) t->buf[0] = '\0'; }
            *out = v_nulo();
            return;
        }
        if (!strcmp(m, "Tamanho") || !strcmp(m, "Comprimento")) {
            *out = v_int(t ? (int64_t)porsol_utf8_len(t->buf ? t->buf : "") : 0);
            return;
        }
    }

    /* Leitor */
    if (classe_eh(este, "Leitor")) {
        LeitorEstado *l = (LeitorEstado *)este->estado;
        const char *m = strchr(chave, ':');
        m = m ? m + 2 : chave;
        if (!strcmp(m, "LerLinha")) {
            if (!l || l->fechado) { *out = v_nulo(); return; }
            if (!l->restante || !*l->restante) { *out = v_text(""); return; }
            char *nl = strchr(l->restante, '\n');
            if (nl) {
                size_t linelen = (size_t)(nl - l->restante);
                if (linelen && l->restante[linelen - 1] == '\r') linelen--;
                char *linha = porsol_strndup(l->restante, linelen);
                char *resto = porsol_strdup(nl + 1);
                free(l->restante);
                l->restante = resto;
                *out = v_text_move(linha);
            } else {
                char *linha = porsol_strdup(l->restante);
                free(l->restante);
                l->restante = porsol_strdup("");
                *out = v_text_move(linha);
            }
            return;
        }
        if (!strcmp(m, "LerResto")) {
            if (l) {
                char *r = l->restante ? l->restante : porsol_strdup("");
                l->restante = porsol_strdup("");
                *out = v_text_move(r);
            } else {
                *out = v_text("");
            }
            return;
        }
        if (!strcmp(m, "EstaNoFim") || !strcmp(m, "TemMais")) {
            int vazio = !l || !l->restante || !*l->restante;
            int no_fim = vazio || (l && l->fechado);
            *out = v_bool(!strcmp(m, "EstaNoFim") ? no_fim : !no_fim);
            return;
        }
        if (!strcmp(m, "Fechar")) {
            if (l) {
                l->fechado = 1;
                free(l->restante);
                l->restante = porsol_strdup("");
            }
            *out = v_nulo();
            return;
        }
    }

    /* InformacaoArquivo */
    if (classe_eh(este, "InformacaoArquivo")) {
        const char *m = strchr(chave, ':');
        m = m ? m + 2 : chave;
        static const struct { const char *metodo; const char *campo; } mapa[] = {
            {"Existe", "__existe"}, {"Nome", "__nome"},
            {"CaminhoCompleto", "__caminho"}, {"Tamanho", "__tamanho"},
            {"UltimaModificacao", "__modificacao"}, {"Diretorio", "__diretorio"},
        };
        for (size_t i = 0; i < sizeof(mapa) / sizeof(mapa[0]); i++) {
            if (!strcmp(m, mapa[i].metodo)) {
                PorsolValue *v = obj_get_campo(este, mapa[i].campo);
                *out = v ? *v : v_nulo();
                return;
            }
        }
    }

    /* Horario (TimeSpan) */
    if (classe_eh(este, "Horario")) {
        const char *m = strchr(chave, ':');
        m = m ? m + 2 : chave;
        int64_t seg = horario_segundos(este);
        if (!strcmp(m, "TotalSegundos")) {
            *out = v_int(seg);
        } else if (!strcmp(m, "TotalMinutos")) {
            *out = v_int(seg / 60);
        } else if (!strcmp(m, "TotalHoras")) {
            *out = v_int(seg / 3600);
        } else if (!strcmp(m, "TotalDias")) {
            *out = v_int(seg / 86400);
        } else if (!strcmp(m, "ParaTexto")) {
            *out = v_text_move(horario_para_texto(seg));
        } else if (!strcmp(m, "Adicionar") || !strcmp(m, "Subtrair")) {
            PorsolObj *outro = NULL;
            if (argc > ai && args[ai].kind == PORSOL_OBJETO)
                outro = (PorsolObj *)args[ai].o;
            int64_t seg_outro = outro ? horario_segundos(outro) : 0;
            int64_t novo = !strcmp(m, "Adicionar") ? seg + seg_outro : seg - seg_outro;
            *out = v_obj(obj_new_horario(novo));
        } else if (!strcmp(m, "Igual") || !strcmp(m, "MaiorQue") ||
                   !strcmp(m, "MenorQue")) {
            PorsolObj *outro = NULL;
            if (argc > ai && args[ai].kind == PORSOL_OBJETO)
                outro = (PorsolObj *)args[ai].o;
            int64_t seg_outro = outro ? horario_segundos(outro) : 0;
            int r;
            if (!strcmp(m, "Igual")) r = seg == seg_outro;
            else if (!strcmp(m, "MaiorQue")) r = seg > seg_outro;
            else r = seg < seg_outro;
            *out = v_bool(r);
        } else {
            *out = v_nulo();
        }
        return;
    }

    /* Cronometro (Stopwatch) */
    if (classe_eh(este, "Cronometro")) {
        const char *m = strchr(chave, ':');
        m = m ? m + 2 : chave;
        int64_t inicio = campo_i64(este, "__inicio_ms__", 0);
        int64_t fim = campo_i64(este, "__fim_ms__", 0);
        int rodando = obj_get_campo(este, "__rodando__") &&
                      obj_get_campo(este, "__rodando__")->kind == PORSOL_BOOL &&
                      obj_get_campo(este, "__rodando__")->i != 0;
        int64_t agora = agora_ms();
        if (!strcmp(m, "Iniciar")) {
            if (!rodando) {
                campo_set_i64(este, "__inicio_ms__", agora);
                campo_set_i64(este, "__fim_ms__", 0);
                obj_set_campo(este, "__rodando__", v_bool(1));
            }
            *out = v_nulo();
        } else if (!strcmp(m, "Reiniciar")) {
            campo_set_i64(este, "__inicio_ms__", agora);
            campo_set_i64(este, "__fim_ms__", 0);
            obj_set_campo(este, "__rodando__", v_bool(1));
            *out = v_nulo();
        } else if (!strcmp(m, "Parar")) {
            if (rodando) {
                campo_set_i64(este, "__fim_ms__", agora);
                obj_set_campo(este, "__rodando__", v_bool(0));
            }
            *out = v_nulo();
        } else if (!strcmp(m, "EstaExecutando")) {
            *out = v_bool(rodando);
        } else {
            /* Decorrido / DecorridoSegundos / DecorridoMilissegundos */
            int64_t fim_ef = (fim > 0) ? fim : agora;
            int64_t decorrido = fim_ef - inicio;
            if (decorrido < 0) decorrido = 0;
            if (!strcmp(m, "DecorridoMilissegundos")) {
                *out = v_int(decorrido);
            } else {
                *out = v_dbl((double)decorrido / 1000.0);
            }
        }
        return;
    }

    /* Regex — instância (engine backtracking) */
    if (classe_eh(este, "Regex")) {
        const char *m = strchr(chave, ':');
        m = m ? m + 2 : chave;
        PorsolValue *pv = obj_get_campo(este, "__padrao__");
        const char *padrao = (pv && pv->kind == PORSOL_TEXTO && pv->s) ? pv->s : "";
        int ok = 0;
        RegexProg *re = regex_compilar(padrao, &ok);
        const char *texto = argc > ai ? arg_texto(ai, argc, args) : "";
        if (!strcmp(m, "Corresponde")) {
            size_t a = 0, b = 0;
            *out = v_bool(ok && regex_match(re, texto, &a, &b));
        } else if (!strcmp(m, "Localizar")) {
            size_t a = 0, b = 0;
            if (ok && regex_match(re, texto, &a, &b)) {
                *out = v_int(utf8_count_chars(texto, 0, a));
            } else {
                *out = v_int(-1);
            }
        } else if (!strcmp(m, "Substituir")) {
            const char *sub = argc > ai + 1 ? arg_texto(ai + 1, argc, args) : "";
            if (!ok) {
                *out = v_text(texto);
            } else {
                /* substitui todas as ocorrências não sobrepostas */
                size_t cap = strlen(texto) + strlen(sub) * 16 + 64;
                char *buf = (char *)malloc(cap);
                size_t bp = 0, pos = 0;
                buf[0] = '\0';
                while (pos <= strlen(texto)) {
                    size_t a = 0, b = 0;
                    /* procura match a partir de pos */
                    int achou = regex_match_from(re, texto, pos, &a, &b);
                    if (!achou) break;
                    /* copia trecho anterior */
                    size_t pre = a - pos;
                    if (bp + pre + strlen(sub) + 1 >= cap) {
                        cap *= 2;
                        buf = (char *)realloc(buf, cap);
                    }
                    memcpy(buf + bp, texto + pos, pre);
                    bp += pre;
                    size_t sl = strlen(sub);
                    memcpy(buf + bp, sub, sl);
                    bp += sl;
                    pos = (b > a) ? b : b + 1; /* evita loop em match vazio */
                    if (a == b) {
                        /* match vazio: avança um codepoint */
                        if (texto[pos]) pos += (size_t)utf8_cp_len(texto + pos);
                    }
                }
                if (bp + strlen(texto + pos) + 1 >= cap) {
                    cap = bp + strlen(texto + pos) + 1;
                    buf = (char *)realloc(buf, cap);
                }
                strcpy(buf + bp, texto + pos);
                *out = v_text_move(buf);
            }
        } else if (!strcmp(m, "Dividir")) {
            PVec partes;
            pvec_init(&partes);
            if (!ok) {
                PorsolValue t = v_nulo();
                t.kind = PORSOL_TEXTO;
                t.s = porsol_strdup(texto);
                pvec_push(&partes, t);
            } else {
                size_t inicio = 0, pos = 0;
                size_t tl = strlen(texto);
                while (pos <= tl) {
                    size_t a = 0, b = 0;
                    if (!regex_match_from(re, texto, pos, &a, &b)) break;
                    if (a == b && a == pos && pos >= tl) break;
                    PorsolValue t = v_nulo();
                    t.kind = PORSOL_TEXTO;
                    t.s = porsol_strndup(texto + inicio, a - inicio);
                    pvec_push(&partes, t);
                    pos = (b > a) ? b : b + 1;
                    if (a == b && texto[pos]) pos += (size_t)utf8_cp_len(texto + pos);
                    inicio = pos;
                    if (pos > tl) break;
                }
                PorsolValue t = v_nulo();
                t.kind = PORSOL_TEXTO;
                t.s = porsol_strdup(texto + inicio);
                pvec_push(&partes, t);
            }
            *out = pvec_para_array_val(&partes);
            pvec_free(&partes);
        } else if (!strcmp(m, "Contar")) {
            int64_t n = 0;
            if (ok) {
                size_t pos = 0, tl = strlen(texto);
                while (pos <= tl) {
                    size_t a = 0, b = 0;
                    if (!regex_match_from(re, texto, pos, &a, &b)) break;
                    n++;
                    pos = (b > a) ? b : b + 1;
                    if (a == b && texto[pos]) pos += (size_t)utf8_cp_len(texto + pos);
                }
            }
            *out = v_int(n);
        } else if (!strcmp(m, "CorrespondeCompleto")) {
            size_t a = 0, b = 0, tl = strlen(texto);
            *out = v_bool(ok && regex_match(re, texto, &a, &b) && a == 0 && b == tl);
        } else if (!strcmp(m, "LocalizarTodas")) {
            PVec partes;
            pvec_init(&partes);
            if (ok) {
                size_t pos = 0, tl = strlen(texto);
                while (pos <= tl) {
                    size_t a = 0, b = 0;
                    if (!regex_match_from(re, texto, pos, &a, &b)) break;
                    PorsolValue t = v_nulo();
                    t.kind = PORSOL_TEXTO;
                    t.s = porsol_strndup(texto + a, b - a);
                    pvec_push(&partes, t);
                    pos = (b > a) ? b : b + 1;
                    if (a == b && texto[pos]) pos += (size_t)utf8_cp_len(texto + pos);
                }
            }
            *out = pvec_para_array_val(&partes);
            pvec_free(&partes);
        } else if (!strcmp(m, "SubstituirPrimeiro")) {
            const char *sub = argc > ai + 1 ? arg_texto(ai + 1, argc, args) : "";
            if (!ok) {
                *out = v_text(texto);
            } else {
                size_t a = 0, b = 0;
                if (!regex_match(re, texto, &a, &b)) {
                    *out = v_text(texto);
                } else {
                    size_t tl = strlen(texto), sl = strlen(sub);
                    char *buf = (char *)malloc(tl - (b - a) + sl + 1);
                    memcpy(buf, texto, a);
                    memcpy(buf + a, sub, sl);
                    strcpy(buf + a + sl, texto + b);
                    *out = v_text_move(buf);
                }
            }
        } else {
            *out = v_nulo();
        }
        return;
    }

    /* ClienteHttp */
    if (classe_eh(este, "ClienteHttp")) {
        const char *m = strchr(chave, ':');
        m = m ? m + 2 : chave;
        /* propriedade BaseUrl (get/set) */
        if (!strcmp(m, "BaseUrl")) {
            PorsolValue *pv = obj_get_campo(este, "BaseUrl");
            *out = (pv && pv->kind == PORSOL_TEXTO) ? v_text(pv->s ? pv->s : "") : v_text("");
            return;
        }
        if (!strncmp(m, "set_", 4) && !strcmp(m + 4, "BaseUrl")) {
            if (argc > ai) obj_set_campo(este, "BaseUrl", v_text(arg_texto(ai, argc, args)));
            *out = v_nulo();
            return;
        }
        const char *verbo = NULL;
        if (!strcmp(m, "Obter") || !strcmp(m, "Get")) verbo = "GET";
        else if (!strcmp(m, "Enviar") || !strcmp(m, "Post")) verbo = "POST";
        else if (!strcmp(m, "Atualizar") || !strcmp(m, "Put")) verbo = "PUT";
        else if (!strcmp(m, "Excluir") || !strcmp(m, "Delete")) verbo = "DELETE";
        if (verbo) {
            const char *url = argc > ai ? arg_texto(ai, argc, args) : "";
            const char *corpo_req = (verbo[0] == 'P' || verbo[0] == 'p' ||
                                     !strcmp(verbo, "PUT")) && argc > ai + 1
                                   ? arg_texto(ai + 1, argc, args) : "";
            /* resolve BaseUrl quando a URL é relativa */
            char urlbuf[2048];
            snprintf(urlbuf, sizeof(urlbuf), "%s", url);
            if (strncmp(url, "http://", 7) && strncmp(url, "https://", 8)) {
                PorsolValue *bv = obj_get_campo(este, "BaseUrl");
                const char *base = (bv && bv->kind == PORSOL_TEXTO && bv->s) ? bv->s : "";
                size_t bl = strlen(base);
                while (bl && base[bl - 1] == '/') bl--;
                const char *rel = url;
                while (*rel == '/') rel++;
                snprintf(urlbuf, sizeof(urlbuf), "%.*s/%s", (int)bl, base, rel);
            }
            char *resp = porsol_http_request(verbo, urlbuf, corpo_req);
            if (resp) {
                *out = v_text_move(resp);
            } else {
                fprintf(stderr, "ClienteHttp::%s: falha na requisição %s\n", verbo, urlbuf);
                *out = v_nulo();
            }
            return;
        }
        *out = v_nulo();
        return;
    }

    /* SocketTcp */
    if (classe_eh(este, "SocketTcp")) {
        const char *m = strchr(chave, ':');
        m = m ? m + 2 : chave;
        int *fdp = (int *)este->estado;
        int fd = (fdp && este->estado_kind == EST_SOCKET) ? *fdp : -1;
        if (!strcmp(m, "Enviar")) {
            const char *dados = argc > ai ? arg_texto(ai, argc, args) : "";
            size_t len = strlen(dados), sent = 0;
            int ok = fd >= 0;
            while (ok && sent < len) {
                int n = send(fd, dados + sent, (int)(len - sent), 0);
                if (n <= 0) { ok = 0; break; }
                sent += (size_t)n;
            }
            if (!ok) fprintf(stderr, "SocketTcp::Enviar: falha ao enviar dados\n");
            *out = v_nulo();
        } else if (!strcmp(m, "Receber")) {
            if (fd < 0) {
                fprintf(stderr, "SocketTcp::Receber: socket inválido\n");
                *out = v_nulo();
            } else {
                char buf[65536];
                int n = recv(fd, buf, sizeof(buf) - 1, 0);
                if (n > 0) {
                    buf[n] = '\0';
                    *out = v_text_move(porsol_strdup(buf));
                } else if (n == 0) {
                    *out = v_text(""); /* conexão encerrada */
                } else {
                    *out = v_text(""); /* timeout/erro: string vazia como o interpretador */
                }
            }
        } else if (!strcmp(m, "Fechar")) {
            if (fd >= 0) { close_sock(fd); *fdp = -1; }
            *out = v_nulo();
        } else if (!strcmp(m, "Conectado")) {
            *out = v_bool(fd >= 0);
        } else {
            *out = v_nulo();
        }
        return;
    }

    /* ServidorTcp */
    if (classe_eh(este, "ServidorTcp")) {
        const char *m = strchr(chave, ':');
        m = m ? m + 2 : chave;
        int *fdp = (int *)este->estado;
        int fd = (fdp && este->estado_kind == EST_SOCKET) ? *fdp : -1;
        if (!strcmp(m, "Aceitar")) {
            if (fd < 0) {
                fprintf(stderr, "ServidorTcp::Aceitar: servidor inválido\n");
                *out = v_nulo();
            } else {
                struct sockaddr_in cli;
                socklen_t clen = sizeof(cli);
                int cfd = accept(fd, (struct sockaddr *)&cli, &clen);
                if (cfd >= 0) {
                    *out = v_obj(socket_tcp_novo(cfd));
                } else {
                    *out = v_nulo();
                }
            }
        } else if (!strcmp(m, "Fechar")) {
            if (fd >= 0) { close_sock(fd); *fdp = -1; }
            *out = v_nulo();
        } else if (!strcmp(m, "EstaAberto")) {
            *out = v_bool(fd >= 0);
        } else {
            *out = v_nulo();
        }
        return;
    }

    /* Mutex */
    if (classe_eh(este, "Mutex")) {
        int *travado = (int *)este->estado;
        const char *m = strchr(chave, ':');
        m = m ? m + 2 : chave;
        if (!strcmp(m, "Entrar")) {
            if (travado && *travado) {
                fprintf(stderr, "Mutex::Entrar: mutex já está travado (possível deadlock)\n");
            } else if (travado) {
                *travado = 1;
            }
            *out = v_nulo();
            return;
        }
        if (!strcmp(m, "TentarEntrar")) {
            int ok = travado && !*travado;
            if (ok) *travado = 1;
            *out = v_bool(ok);
            return;
        }
        if (!strcmp(m, "Sair") || !strcmp(m, "Fechar")) {
            if (travado) *travado = 0;
            *out = v_nulo();
            return;
        }
        if (!strcmp(m, "EstaTravado")) {
            *out = v_bool(travado && *travado);
            return;
        }
    }

    /* Aleatorio */
    if (classe_eh(este, "Aleatorio")) {
        u64 *estado = (u64 *)este->estado;
        const char *m = strchr(chave, ':');
        m = m ? m + 2 : chave;
        if (!estado) { *out = v_nulo(); return; }
        if (!strcmp(m, "Reseed")) {
            *estado = semente_aleatoria((u64)arg_int(ai, argc, args));
            *out = v_nulo();
            return;
        }
        if (!strcmp(m, "ProximoEntre")) {
            int64_t min = arg_int(ai, argc, args);
            int64_t max = arg_int(ai + 1, argc, args);
            uint64_t faixa = (uint64_t)(max - min > 0 ? max - min : 1);
            uint64_t v = proximo_aleatorio_u64(estado) % faixa;
            *out = v_int(min + (int64_t)v);
            return;
        }
        if (!strcmp(m, "ProximoFlutuante")) {
            uint64_t v = proximo_aleatorio_u64(estado);
            *out = v_dbl((double)(v >> 11) / (double)(1ULL << 53));
            return;
        }
        if (!strcmp(m, "ProximoBooleano")) {
            *out = v_bool((proximo_aleatorio_u64(estado) & 1) == 1);
            return;
        }
        if (!strcmp(m, "Proximo")) {
            *out = v_int((int64_t)(proximo_aleatorio_u64(estado) % 0x7FFFFFFFULL));
            return;
        }
    }

    fprintf(stderr, "[aviso nativo] Método '%s' não implementado no runtime LLVM\n", chave);
    *out = v_nulo();
}

/* ================================================================== */
/* JSON — parser/serializer mínimo compatível com serde_json           */
/* ================================================================== */

typedef enum JTipo {
    J_NULO, J_BOOL, J_NUM, J_TEXTO, J_ARRAY, J_OBJETO
} JTipo;

typedef struct JValor {
    JTipo tipo;
    int booleano;
    double numero;
    int64_t inteiro;
    int eh_inteiro;
    char *texto;                    /* J_TEXTO: conteúdo; J_OBJETO: não usado */
    struct JValor **itens;          /* J_ARRAY */
    int n_itens, cap_itens;
    char **chaves;                  /* J_OBJETO */
    struct JValor **valores;
    int n_pares, cap_pares;
} JValor;

static JValor *j_new(JTipo t) {
    JValor *j = (JValor *)calloc(1, sizeof(JValor));
    j->tipo = t;
    return j;
}

static void j_array_push(JValor *j, JValor *item) {
    if (j->n_itens == j->cap_itens) {
        j->cap_itens = j->cap_itens ? j->cap_itens * 2 : 8;
        j->itens = (JValor **)realloc(j->itens, sizeof(JValor *) * j->cap_itens);
    }
    j->itens[j->n_itens++] = item;
}

static void j_obj_set(JValor *j, const char *chave, JValor *valor) {
    for (int i = 0; i < j->n_pares; i++) {
        if (!strcmp(j->chaves[i], chave)) {
            j->valores[i] = valor;
            return;
        }
    }
    if (j->n_pares == j->cap_pares) {
        j->cap_pares = j->cap_pares ? j->cap_pares * 2 : 8;
        j->chaves = (char **)realloc(j->chaves, sizeof(char *) * j->cap_pares);
        j->valores = (JValor **)realloc(j->valores, sizeof(JValor *) * j->cap_pares);
    }
    j->chaves[j->n_pares] = porsol_strdup(chave);
    j->valores[j->n_pares] = valor;
    j->n_pares++;
}

/* ---------- parser JSON ---------- */

typedef struct JParser {
    const char *p;
    int ok;
} JParser;

static void j_skip_ws(JParser *ps) {
    while (*ps->p == ' ' || *ps->p == '\t' || *ps->p == '\r' || *ps->p == '\n') ps->p++;
}

static JValor *j_parse_valor(JParser *ps);

static char *j_parse_string(JParser *ps) {
    /* ps->p aponta para '"' */
    ps->p++;
    size_t cap = 32, n = 0;
    char *buf = (char *)malloc(cap);
    while (*ps->p && *ps->p != '"') {
        char c = *ps->p++;
        if (c == '\\') {
            char e = *ps->p++;
            switch (e) {
                case '"': c = '"'; break;
                case '\\': c = '\\'; break;
                case '/': c = '/'; break;
                case 'b': c = '\b'; break;
                case 'f': c = '\f'; break;
                case 'n': c = '\n'; break;
                case 'r': c = '\r'; break;
                case 't': c = '\t'; break;
                case 'u': {
                    /* \uXXXX — decodifica para UTF-8 */
                    unsigned cp = 0;
                    for (int i = 0; i < 4 && isxdigit((unsigned char)*ps->p); i++, ps->p++) {
                        char h = *ps->p;
                        cp = cp * 16 + (unsigned)(h <= '9' ? h - '0' :
                            (h <= 'F' ? h - 'A' + 10 : h - 'a' + 10));
                    }
                    if (cp < 0x80) {
                        c = (char)cp;
                    } else if (cp < 0x800) {
                        if (n + 2 >= cap) { cap *= 2; buf = (char *)realloc(buf, cap); }
                        buf[n++] = (char)(0xC0 | (cp >> 6));
                        buf[n++] = (char)(0x80 | (cp & 0x3F));
                        continue;
                    } else {
                        if (n + 3 >= cap) { cap *= 2; buf = (char *)realloc(buf, cap); }
                        buf[n++] = (char)(0xE0 | (cp >> 12));
                        buf[n++] = (char)(0x80 | ((cp >> 6) & 0x3F));
                        buf[n++] = (char)(0x80 | (cp & 0x3F));
                        continue;
                    }
                    break;
                }
                default: ps->ok = 0; c = e; break;
            }
        }
        if (n + 2 >= cap) { cap *= 2; buf = (char *)realloc(buf, cap); }
        buf[n++] = c;
    }
    if (*ps->p == '"') ps->p++;
    else ps->ok = 0;
    buf[n] = '\0';
    return buf;
}

static JValor *j_parse_valor(JParser *ps) {
    j_skip_ws(ps);
    char c = *ps->p;
    if (c == '"') {
        JValor *j = j_new(J_TEXTO);
        j->texto = j_parse_string(ps);
        return j;
    }
    if (c == '{') {
        ps->p++;
        JValor *j = j_new(J_OBJETO);
        j_skip_ws(ps);
        if (*ps->p == '}') { ps->p++; return j; }
        while (ps->ok && *ps->p) {
            j_skip_ws(ps);
            if (*ps->p != '"') { ps->ok = 0; break; }
            char *chave = j_parse_string(ps);
            j_skip_ws(ps);
            if (*ps->p != ':') { free(chave); ps->ok = 0; break; }
            ps->p++;
            JValor *val = j_parse_valor(ps);
            if (!val) { free(chave); ps->ok = 0; break; }
            j_obj_set(j, chave, val);
            free(chave);
            j_skip_ws(ps);
            if (*ps->p == ',') { ps->p++; continue; }
            if (*ps->p == '}') { ps->p++; break; }
            ps->ok = 0;
            break;
        }
        return j;
    }
    if (c == '[') {
        ps->p++;
        JValor *j = j_new(J_ARRAY);
        j_skip_ws(ps);
        if (*ps->p == ']') { ps->p++; return j; }
        while (ps->ok && *ps->p) {
            JValor *item = j_parse_valor(ps);
            if (!item) { ps->ok = 0; break; }
            j_array_push(j, item);
            j_skip_ws(ps);
            if (*ps->p == ',') { ps->p++; continue; }
            if (*ps->p == ']') { ps->p++; break; }
            ps->ok = 0;
            break;
        }
        return j;
    }
    if (c == 't' && !strncmp(ps->p, "true", 4)) {
        ps->p += 4;
        JValor *j = j_new(J_BOOL); j->booleano = 1; return j;
    }
    if (c == 'f' && !strncmp(ps->p, "false", 5)) {
        ps->p += 5;
        JValor *j = j_new(J_BOOL); j->booleano = 0; return j;
    }
    if (c == 'n' && !strncmp(ps->p, "null", 4)) {
        ps->p += 4;
        return j_new(J_NULO);
    }
    /* número */
    char *end = NULL;
    double d = strtod(ps->p, &end);
    if (end != ps->p) {
        /* detecta inteiro */
        int eh_int = 1;
        for (const char *q = ps->p; q < end; q++) {
            if (*q == '.' || *q == 'e' || *q == 'E') { eh_int = 0; break; }
        }
        JValor *j = j_new(J_NUM);
        j->numero = d;
        j->eh_inteiro = eh_int;
        if (eh_int) j->inteiro = (int64_t)d;
        ps->p = end;
        return j;
    }
    ps->ok = 0;
    return NULL;
}

static JValor *json_parse(const char *texto) {
    JParser ps;
    ps.p = texto ? texto : "";
    ps.ok = 1;
    JValor *j = j_parse_valor(&ps);
    j_skip_ws(&ps);
    if (!ps.ok || *ps.p != '\0') {
        /* inválido */
        if (j) { /* vaza árvore — processo de vida curta */ }
        return NULL;
    }
    return j;
}

static void json_escape_into(const char *s, char **buf, size_t *n, size_t *cap) {
    for (const unsigned char *p = (const unsigned char *)s; *p; p++) {
        const char *esc = NULL;
        char tmp[8];
        switch (*p) {
            case '"': esc = "\\\""; break;
            case '\\': esc = "\\\\"; break;
            case '\b': esc = "\\b"; break;
            case '\f': esc = "\\f"; break;
            case '\n': esc = "\\n"; break;
            case '\r': esc = "\\r"; break;
            case '\t': esc = "\\t"; break;
            default:
                if (*p < 0x20) {
                    snprintf(tmp, sizeof(tmp), "\\u%04x", *p);
                    esc = tmp;
                }
        }
        const char *src = esc ? esc : (const char *)p;
        size_t add = esc ? strlen(esc) : 1;
        if (*n + add + 1 >= *cap) {
            *cap *= 2;
            *buf = (char *)realloc(*buf, *cap);
        }
        memcpy(*buf + *n, src, add);
        *n += add;
        (*buf)[*n] = '\0';
    }
}

static char *json_string_quoted(const char *s) {
    size_t cap = strlen(s) * 2 + 8, n = 0;
    char *buf = (char *)malloc(cap);
    buf[n++] = '"'; buf[n] = '\0';
    json_escape_into(s, &buf, &n, &cap);
    if (n + 2 >= cap) { cap = n + 16; buf = (char *)realloc(buf, cap); }
    buf[n++] = '"'; buf[n] = '\0';
    return buf;
}

/* ---------- JValor -> PorsolValue ---------- */

static PorsolValue json_j_para_valor(JValor *j);

static PorsolValue json_j_array_para_valor(JValor *j) {
    PVec v;
    pvec_init(&v);
    for (int i = 0; i < j->n_itens; i++) {
        PorsolValue item = json_j_para_valor(j->itens[i]);
        pvec_push(&v, item);
    }
    PorsolValue out = pvec_para_array_val(&v);
    pvec_free(&v);
    return out;
}

static PorsolValue json_j_objeto_para_valor(JValor *j) {
    /* Objeto JSON vira PorsolObj "JsonObjeto" com campos nomeados. */
    PorsolObj *o = (PorsolObj *)calloc(1, sizeof(PorsolObj));
    o->magic = PORSOL_OBJ_MAGIC;
    o->classe = porsol_strdup("JsonObjeto");
    o->fqn = porsol_strdup("Sistema.Texto.JsonObjeto");
    o->estado_kind = EST_GENERICO;
    for (int i = 0; i < j->n_pares; i++) {
        PorsolValue val = json_j_para_valor(j->valores[i]);
        obj_set_campo(o, j->chaves[i], val);
    }
    PorsolValue out = v_nulo();
    out.kind = PORSOL_OBJETO;
    out.o = o;
    return out;
}

static PorsolValue json_j_para_valor(JValor *j) {
    if (!j) return v_nulo();
    switch (j->tipo) {
        case J_NULO: return v_nulo();
        case J_BOOL: return v_bool(j->booleano);
        case J_NUM:
            if (j->eh_inteiro) return v_int(j->inteiro);
            return v_dbl(j->numero);
        case J_TEXTO: return v_text(j->texto ? j->texto : "");
        case J_ARRAY: return json_j_array_para_valor(j);
        case J_OBJETO: return json_j_objeto_para_valor(j);
    }
    return v_nulo();
}

/* ---------- PorsolValue -> JSON ---------- */

typedef struct JsonOut {
    char *buf;
    size_t n, cap;
    int indent;      /* nível de indentação */
    int pretty;
} JsonOut;

static void json_out_reserve(JsonOut *o, size_t add) {
    if (o->n + add + 1 >= o->cap) {
        while (o->n + add + 1 >= o->cap) o->cap *= 2;
        o->buf = (char *)realloc(o->buf, o->cap);
    }
}

static void json_out_raw(JsonOut *o, const char *s) {
    size_t l = strlen(s);
    json_out_reserve(o, l);
    memcpy(o->buf + o->n, s, l);
    o->n += l;
    o->buf[o->n] = '\0';
}

static void json_out_char(JsonOut *o, char c) {
    json_out_reserve(o, 1);
    o->buf[o->n++] = c;
    o->buf[o->n] = '\0';
}

static void json_out_newline_indent(JsonOut *o) {
    if (!o->pretty) return;
    json_out_char(o, '\n');
    for (int i = 0; i < o->indent * 2; i++) json_out_char(o, ' ');
}

static void json_serializar_valor_rec(JsonOut *o, const PorsolValue *v);

static void json_serializar_texto(JsonOut *o, const char *s) {
    char *q = json_string_quoted(s ? s : "");
    json_out_raw(o, q);
    free(q);
}

static void json_serializar_array_dados(JsonOut *o, const PorsolArrayHdr *h) {
    json_out_char(o, '[');
    for (int i = 0; i < h->len; i++) {
        if (i) json_out_char(o, ',');
        PorsolValue item = v_nulo();
        switch (h->elem_kind) {
            case PORSOL_INTEIRO: item = v_int(((const int32_t *)h->data)[i]); break;
            case PORSOL_DOUBLE:  item = v_dbl(((const double *)h->data)[i]); break;
            case PORSOL_BOOL:    item = v_bool(((const uint8_t *)h->data)[i]); break;
            case PORSOL_TEXTO:   item = v_text(((const char **)h->data)[i] ? ((const char **)h->data)[i] : ""); break;
            case PORSOL_OBJETO: {
                void *p = ((void **)h->data)[i];
                item = v_obj(p);
                item.kind = PORSOL_OBJETO;
                break;
            }
            default: item = v_nulo(); break;
        }
        if (o->pretty && h->len > 1) { o->indent++; json_out_newline_indent(o); }
        json_serializar_valor_rec(o, &item);
        if (o->pretty && h->len > 1) { o->indent--; json_out_newline_indent(o); }
        if (item.kind == PORSOL_TEXTO) free(item.s);
    }
    if (o->pretty && h->len > 1) json_out_newline_indent(o);
    json_out_char(o, ']');
}

static void json_serializar_objeto_campos(JsonOut *o, PorsolObj *obj, int primeiro_nivel) {
    (void)primeiro_nivel;
    json_out_char(o, '{');
    int escritos = 0;
    for (int i = 0; i < obj->ncampos; i++) {
        if (obj->campos_nom[i][0] == '_' && obj->campos_nom[i][1] == '_') continue;
        if (escritos) json_out_char(o, ',');
        if (o->pretty) { o->indent++; json_out_newline_indent(o); }
        json_serializar_texto(o, obj->campos_nom[i]);
        json_out_char(o, ':');
        if (o->pretty) json_out_char(o, ' ');
        json_serializar_valor_rec(o, &obj->campos_val[i]);
        if (o->pretty) { o->indent--; json_out_newline_indent(o); }
        escritos++;
    }
    if (o->pretty && escritos) json_out_newline_indent(o);
    json_out_char(o, '}');
}

static void json_serializar_valor_rec(JsonOut *o, const PorsolValue *v) {
    if (!v) { json_out_raw(o, "null"); return; }
    switch (v->kind) {
        case PORSOL_NULO: json_out_raw(o, "null"); break;
        case PORSOL_BOOL: json_out_raw(o, v->i ? "true" : "false"); break;
        case PORSOL_INTEIRO: {
            char tmp[32];
            snprintf(tmp, sizeof(tmp), "%lld", (long long)v->i);
            json_out_raw(o, tmp);
            break;
        }
        case PORSOL_DOUBLE: {
            char tmp[64];
            /* repr curta estilo JSON */
            snprintf(tmp, sizeof(tmp), "%.17g", v->d);
            /* garante formato float válido */
            if (!strchr(tmp, '.') && !strchr(tmp, 'e') && !strchr(tmp, 'E') &&
                strcmp(tmp, "inf") && strcmp(tmp, "-inf") && strcmp(tmp, "nan")) {
                size_t l = strlen(tmp);
                tmp[l] = '.'; tmp[l + 1] = '0'; tmp[l + 2] = '\0';
            }
            json_out_raw(o, tmp);
            break;
        }
        case PORSOL_TEXTO: json_serializar_texto(o, v->s ? v->s : ""); break;
        case PORSOL_ARRAY:
            if (v->o) json_serializar_array_dados(o, (PorsolArrayHdr *)v->o);
            else json_out_raw(o, "[]");
            break;
        case PORSOL_OBJETO: {
            PorsolObj *obj = (PorsolObj *)v->o;
            if (obj && obj->magic == PORSOL_OBJ_MAGIC) {
                /* Dicionario -> objeto JSON chave/valor */
                if (obj->estado_kind == EST_DICIONARIO) {
                    PDict *d = (PDict *)obj->estado;
                    json_out_char(o, '{');
                    int escritos = 0;
                    for (int i = 0; i < d->n; i++) {
                        if (d->itens[i].chave.kind != PORSOL_TEXTO) continue;
                        if (escritos) json_out_char(o, ',');
                        if (o->pretty) { o->indent++; json_out_newline_indent(o); }
                        json_serializar_texto(o, d->itens[i].chave.s ? d->itens[i].chave.s : "");
                        json_out_char(o, ':');
                        if (o->pretty) json_out_char(o, ' ');
                        json_serializar_valor_rec(o, &d->itens[i].valor);
                        if (o->pretty) { o->indent--; json_out_newline_indent(o); }
                        escritos++;
                    }
                    if (o->pretty && escritos) json_out_newline_indent(o);
                    json_out_char(o, '}');
                } else {
                    json_serializar_objeto_campos(o, obj, 1);
                }
            } else {
                /* objeto de usuário opaco -> null */
                json_out_raw(o, "null");
            }
            break;
        }
        default: json_out_raw(o, "null"); break;
    }
}

static char *json_serializar(const PorsolValue *v, int pretty) {
    JsonOut o;
    o.buf = (char *)malloc(256);
    o.buf[0] = '\0';
    o.n = 0; o.cap = 256; o.indent = 0; o.pretty = pretty;
    json_serializar_valor_rec(&o, v);
    return o.buf;
}

static PorsolValue json_serializar_valor(const PorsolValue *v) {
    char *s = json_serializar(v, 0);
    PorsolValue out = v_nulo();
    out.kind = PORSOL_TEXTO;
    out.s = s;
    return out;
}

static PorsolValue json_serializar_valor_pretty(const PorsolValue *v) {
    char *s = json_serializar(v, 1);
    PorsolValue out = v_nulo();
    out.kind = PORSOL_TEXTO;
    out.s = s;
    return out;
}

/* Retorna o tipo de um documento JSON: "objeto", "array", "texto",
 * "numero", "booleano", "nulo" ou "invalido". */
static const char *json_tipo_de(const char *texto) {
    JValor *j = json_parse(texto);
    if (!j) return "invalido";
    switch (j->tipo) {
        case J_OBJETO: return "objeto";
        case J_ARRAY:  return "array";
        case J_TEXTO:  return "texto";
        case J_NUM:    return "numero";
        case J_BOOL:   return "booleano";
        case J_NULO:   return "nulo";
    }
    return "invalido";
}

static PorsolValue json_desserializar(const char *texto, int *ok) {
    JValor *j = json_parse(texto);
    if (!j) { *ok = 0; return v_nulo(); }
    *ok = 1;
    return json_j_para_valor(j);
}

static char *json_formatar(const char *texto) {
    JValor *j = json_parse(texto);
    if (!j) return NULL;
    /* re-serializa a partir do JValor usando o mesmo emissor:
       converte JValor -> PorsolValue -> JSON pretty. */
    PorsolValue v = json_j_para_valor(j);
    return json_serializar(&v, 1);
}

static int json_validar(const char *texto) {
    JValor *j = json_parse(texto);
    return j != NULL;
}

/* ================================================================== */
/* Regex — engine backtracking com fuel (sem lookaround/backrefs)      */
/* ================================================================== */

typedef enum ROp {
    R_CHAR,     /* byte literal */
    R_ANY,      /* . */
    R_CLASS,    /* [..] ou classe predefinida \d \w \s */
    R_BOL,      /* ^ */
    R_EOL,      /* $ */
    R_SEQ,      /* sequência */
    R_ALT,      /* | */
    R_REP       /* repetição {min,max,greedy} */
} ROp;

typedef struct RENode {
    ROp op;
    int c;                 /* R_CHAR: byte */
    int cls;               /* R_CLASS: índice em classes[] ou tipo predefinido */
    struct RENode **filhos;
    int n_filhos;
    int min_rep, max_rep;  /* R_REP: -1 = infinito */
    int greedy;
} RENode;

/* classes de caracteres predefinidas */
enum {
    CLS_DIGIT = 1, CLS_NOT_DIGIT, CLS_WORD, CLS_NOT_WORD,
    CLS_SPACE, CLS_NOT_SPACE, CLS_CUSTOM
};

typedef struct RClass {
    unsigned char mapa[32]; /* bitmap de 256 bits */
    int negada;
} RClass;

typedef struct RegexProg {
    RENode *raiz;
    RClass *classes;
    int n_classes, cap_classes;
    char *padrao;
} RegexProg;

static RENode *re_new(ROp op) {
    RENode *n = (RENode *)calloc(1, sizeof(RENode));
    n->op = op;
    n->max_rep = 1;
    n->greedy = 1;
    return n;
}

static void re_add_filho(RENode *pai, RENode *filho) {
    pai->filhos = (RENode **)realloc(pai->filhos, sizeof(RENode *) * (pai->n_filhos + 1));
    pai->filhos[pai->n_filhos++] = filho;
}

struct RParser {
    const char *p;
    RegexProg *prog;
    int ok;
};

static int re_hexval(int c) {
    if (c >= '0' && c <= '9') return c - '0';
    if (c >= 'a' && c <= 'f') return c - 'a' + 10;
    if (c >= 'A' && c <= 'F') return c - 'A' + 10;
    return -1;
}

/* classes predefinidas como bitmap */
static void rclass_init_pre(RClass *c, int tipo) {
    memset(c, 0, sizeof(*c));
    c->negada = 0;
    for (int i = 0; i < 256; i++) {
        int incl = 0;
        switch (tipo) {
            case CLS_DIGIT: incl = i >= '0' && i <= '9'; break;
            case CLS_WORD: incl = (i >= '0' && i <= '9') || (i >= 'a' && i <= 'z') ||
                                  (i >= 'A' && i <= 'Z') || i == '_'; break;
            case CLS_SPACE: incl = i == ' ' || i == '\t' || i == '\n' || i == '\r' || i == '\f' || i == '\v'; break;
            default: break;
        }
        if (incl) c->mapa[i >> 3] |= (unsigned char)(1 << (i & 7));
    }
    if (tipo == CLS_NOT_DIGIT || tipo == CLS_NOT_WORD || tipo == CLS_NOT_SPACE) {
        /* inverte */
        for (int i = 0; i < 32; i++) c->mapa[i] = (unsigned char)~c->mapa[i];
    }
}

static int rclass_test(const RClass *c, int byte) {
    int m = (c->mapa[byte >> 3] >> (byte & 7)) & 1;
    return c->negada ? !m : m;
}

static int rclass_add(RClass *c, int byte) {
    c->mapa[byte >> 3] |= (unsigned char)(1 << (byte & 7));
    return 0;
}

/* parser de classe [...] -> RClass */
static RClass *re_parse_class(struct RParser *ps) {
    /* consome '[' */
    ps->p++;
    RClass *c = (RClass *)calloc(1, sizeof(RClass));
    int primeiro = 1;
    if (*ps->p == '^') { c->negada = 1; ps->p++; }
    int tem_algo = 0;
    while (*ps->p && (*ps->p != ']' || primeiro)) {
        primeiro = 0;
        int lo;
        if (*ps->p == '\\') {
            ps->p++;
            char e = *ps->p++;
            switch (e) {
                case 'd': rclass_init_pre(c, CLS_DIGIT); tem_algo = 1; continue;
                case 'w': rclass_init_pre(c, CLS_WORD); tem_algo = 1; continue;
                case 's': rclass_init_pre(c, CLS_SPACE); tem_algo = 1; continue;
                case 'n': lo = '\n'; break;
                case 't': lo = '\t'; break;
                case 'r': lo = '\r'; break;
                default: lo = (unsigned char)e; break;
            }
        } else {
            lo = (unsigned char)*ps->p++;
        }
        /* intervalo lo-hi */
        if (*ps->p == '-' && ps->p[1] && ps->p[1] != ']') {
            ps->p++;
            int hi;
            if (*ps->p == '\\') {
                ps->p++;
                char e = *ps->p++;
                hi = (e == 'n') ? '\n' : (e == 't') ? '\t' : (e == 'r') ? '\r' : (unsigned char)e;
            } else {
                hi = (unsigned char)*ps->p++;
            }
            for (int b = lo; b <= hi && b < 256; b++) rclass_add(c, b);
        } else {
            rclass_add(c, lo);
        }
        tem_algo = 1;
    }
    if (*ps->p == ']') ps->p++;
    else ps->ok = 0;
    if (!tem_algo) ps->ok = 0;
    return c;
}

/* escape fora de classe -> byte ou classe predefinida */
static int re_parse_escape_byte(struct RParser *ps, int *cls_tipo) {
    *cls_tipo = 0;
    char e = *ps->p++;
    switch (e) {
        case 'n': return '\n';
        case 't': return '\t';
        case 'r': return '\r';
        case 'd': *cls_tipo = CLS_DIGIT; return 0;
        case 'D': *cls_tipo = CLS_NOT_DIGIT; return 0;
        case 'w': *cls_tipo = CLS_WORD; return 0;
        case 'W': *cls_tipo = CLS_NOT_WORD; return 0;
        case 's': *cls_tipo = CLS_SPACE; return 0;
        case 'S': *cls_tipo = CLS_NOT_SPACE; return 0;
        case 'x': {
            int v = 0, n = 0;
            while (n < 2 && re_hexval((unsigned char)*ps->p) >= 0) {
                v = v * 16 + re_hexval((unsigned char)*ps->p);
                ps->p++; n++;
            }
            return v;
        }
        default: return (unsigned char)e;
    }
}

static RENode *re_parse_alt(struct RParser *ps);

static RENode *re_parse_seq(struct RParser *ps) {
    RENode *seq = re_new(R_SEQ);
    while (*ps->p && *ps->p != ')' && *ps->p != '|') {
        RENode *atomo = NULL;
        char c = *ps->p;
        if (c == '(') {
            ps->p++;
            atomo = re_parse_alt(ps);
            if (*ps->p == ')') ps->p++;
            else ps->ok = 0;
        } else if (c == '[') {
            /* classe customizada: guardamos como R_CLASS com índice */
            RClass *cl = re_parse_class(ps);
            if (ps->prog->n_classes == ps->prog->cap_classes) {
                ps->prog->cap_classes = ps->prog->cap_classes ? ps->prog->cap_classes * 2 : 8;
                ps->prog->classes = (RClass *)realloc(ps->prog->classes, sizeof(RClass) * ps->prog->cap_classes);
            }
            ps->prog->classes[ps->prog->n_classes] = *cl;
            free(cl);
            atomo = re_new(R_CLASS);
            atomo->cls = ps->prog->n_classes++;
        } else if (c == '.') {
            ps->p++;
            atomo = re_new(R_ANY);
        } else if (c == '^') {
            ps->p++;
            atomo = re_new(R_BOL);
        } else if (c == '$') {
            ps->p++;
            atomo = re_new(R_EOL);
        } else if (c == '\\') {
            ps->p++;
            int cls_tipo = 0;
            int b = re_parse_escape_byte(ps, &cls_tipo);
            if (cls_tipo) {
                /* classe predefinida: registra bitmap */
                if (ps->prog->n_classes == ps->prog->cap_classes) {
                    ps->prog->cap_classes = ps->prog->cap_classes ? ps->prog->cap_classes * 2 : 8;
                    ps->prog->classes = (RClass *)realloc(ps->prog->classes, sizeof(RClass) * ps->prog->cap_classes);
                }
                rclass_init_pre(&ps->prog->classes[ps->prog->n_classes], cls_tipo);
                atomo = re_new(R_CLASS);
                atomo->cls = ps->prog->n_classes++;
                atomo->cls |= 0x10000; /* marca predefinida (não usada) */
            } else {
                atomo = re_new(R_CHAR);
                atomo->c = b;
            }
        } else {
            ps->p++;
            atomo = re_new(R_CHAR);
            atomo->c = (unsigned char)c;
        }
        /* quantificador */
        if (atomo && *ps->p) {
            int min_r = -1, max_r = -1;
            if (*ps->p == '*') { min_r = 0; max_r = -1; ps->p++; }
            else if (*ps->p == '+') { min_r = 1; max_r = -1; ps->p++; }
            else if (*ps->p == '?') { min_r = 0; max_r = 1; ps->p++; }
            else if (*ps->p == '{') {
                const char *q = ps->p + 1;
                int lo = 0, digits = 0;
                while (*q >= '0' && *q <= '9') { lo = lo * 10 + (*q - '0'); q++; digits++; }
                if (digits && *q == '}') { min_r = lo; max_r = lo; ps->p = q + 1; }
                else if (digits && *q == ',') {
                    q++;
                    if (*q == '}') { min_r = lo; max_r = -1; ps->p = q + 1; }
                    else {
                        int hv = 0, hd = 0;
                        while (*q >= '0' && *q <= '9') { hv = hv * 10 + (*q - '0'); q++; hd++; }
                        if (hd && *q == '}') { min_r = lo; max_r = hv; ps->p = q + 1; }
                    }
                }
            }
            if (min_r >= 0) {
                int greedy = 1;
                if (*ps->p == '?') { greedy = 0; ps->p++; }
                RENode *rep = re_new(R_REP);
                rep->min_rep = min_r;
                rep->max_rep = max_r;
                rep->greedy = greedy;
                re_add_filho(rep, atomo);
                atomo = rep;
            }
        }
        re_add_filho(seq, atomo);
    }
    return seq;
}

static RENode *re_parse_alt(struct RParser *ps) {
    RENode *alt = re_new(R_ALT);
    re_add_filho(alt, re_parse_seq(ps));
    while (*ps->p == '|') {
        ps->p++;
        re_add_filho(alt, re_parse_seq(ps));
    }
    if (alt->n_filhos == 1) {
        RENode *only = alt->filhos[0];
        free(alt->filhos);
        free(alt);
        return only;
    }
    return alt;
}

/* ---------- matcher backtracking ---------- */

typedef struct RMatchCtx2 {
    const char *texto;
    size_t len;
    const RClass *classes;
    long fuel;
} RMatchCtx2;

static int re_atomo(RENode *n, RMatchCtx2 *ctx, size_t pos, size_t *out) {
    const unsigned char *t = (const unsigned char *)ctx->texto;
    if (--ctx->fuel <= 0) return 0;
    switch (n->op) {
        case R_CHAR:
            if (pos < ctx->len && t[pos] == (unsigned char)n->c) { *out = pos + 1; return 1; }
            return 0;
        case R_ANY:
            if (pos < ctx->len && t[pos] != '\n') { *out = pos + 1; return 1; }
            return 0;
        case R_CLASS: {
            if (pos >= ctx->len) return 0;
            int idx = n->cls & 0xFFFF;
            if (rclass_test(&ctx->classes[idx], t[pos])) { *out = pos + 1; return 1; }
            return 0;
        }
        case R_BOL:
            if (pos == 0 || t[pos - 1] == '\n') { *out = pos; return 1; }
            return 0;
        case R_EOL:
            if (pos == ctx->len || t[pos] == '\n') { *out = pos; return 1; }
            return 0;
        default:
            return 0;
    }
}

/* Casa sequência de nós a partir de idx em pos.
 * Retorna 1 no primeiro sucesso (ordem greedy) e preenche fim_out. */
static int re_seq(RENode **nos, int n, int idx, RMatchCtx2 *ctx,
                  size_t pos, size_t *fim_out) {
    if (idx >= n) { *fim_out = pos; return 1; }
    RENode *node = nos[idx];
    if (node->op == R_REP) {
        RENode *inner = node->filhos[0];
        /* tenta repetições: primeiro o máximo (greedy) ou mínimo */
        /* coletamos posições após k repetições */
        /* abordagem: tenta k de max->min (greedy) ou min->max (non-greedy) */
        /* para simplificar: tenta todos k de 0..max e para cada continua */
        int maxk = node->max_rep < 0 ? 64 : node->max_rep; /* limite prático */
        /* gera candidatos iterativamente */
        size_t *positions = (size_t *)malloc(sizeof(size_t) * (size_t)(maxk + 2));
        int npos = 0;
        positions[npos++] = pos;
        size_t cur = pos;
        for (int k = 0; k < maxk; k++) {
            /* casa inner UMA vez a partir de cur — inner pode ser SEQ/ALT;
               convertemos inner em lista de 1 nó e chamamos re_seq */
            RENode *inner_nos[1] = { inner };
            size_t fim_inner;
            if (!re_seq(inner_nos, 1, 0, ctx, cur, &fim_inner)) break;
            if (fim_inner == cur && k > 0) break; /* progresso zero */
            cur = fim_inner;
            positions[npos++] = cur;
            if (cur > ctx->len) break;
        }
        /* tenta k's na ordem */
        int ord_ini = node->greedy ? npos - 1 : (node->min_rep < npos ? node->min_rep : 0);
        int ord_fim = node->greedy ? (node->min_rep > 0 ? node->min_rep - 1 : -1) : npos - 1;
        int step = node->greedy ? -1 : 1;
        for (int k = ord_ini; node->greedy ? (k >= ord_fim) : (k <= ord_fim); k += step) {
            if (k < node->min_rep || k > maxk) continue;
            size_t fim;
            if (re_seq(nos, n, idx + 1, ctx, positions[k], &fim)) {
                free(positions);
                *fim_out = fim;
                return 1;
            }
            if (ctx->fuel <= 0) break;
        }
        free(positions);
        return 0;
    }
    if (node->op == R_ALT) {
        for (int i = 0; i < node->n_filhos && ctx->fuel > 0; i++) {
            RENode *alt_nos[1] = { node->filhos[i] };
            size_t fim;
            if (re_seq(alt_nos, 1, 0, ctx, pos, &fim) &&
                re_seq(nos, n, idx + 1, ctx, fim, fim_out)) {
                return 1;
            }
        }
        return 0;
    }
    /* átomo ou grupo (SEQ/CHAR/etc.) */
    if (node->op == R_SEQ && node->n_filhos == 0) {
        /* sequência vazia (de grupos vazios) */
        return re_seq(nos, n, idx + 1, ctx, pos, fim_out);
    }
    if (node->op == R_SEQ || node->op == R_ALT) {
        /* nó composto como filho único: tenta casar o sub- nó e continua.
           Para SEQ, seus filhos devem casar em sequência a partir de pos. */
        if (node->op == R_SEQ) {
            /* tenta casar os filhos do SEQ como subsequência e depois continua */
            /* chamamos re_seq recursivamente nos filhos; se suceder, continuamos */
            /* truque: empacota como [SEQ-filhos..., resto...] — simplificação:
               casamos os filhos e usamos o fim como novo pos */
            size_t sub_fim;
            if (!re_seq(node->filhos, node->n_filhos, 0, ctx, pos, &sub_fim)) return 0;
            return re_seq(nos, n, idx + 1, ctx, sub_fim, fim_out);
        }
    }
    /* átomo simples */
    {
        size_t fim_atomo;
        if (!re_atomo(node, ctx, pos, &fim_atomo)) return 0;
        return re_seq(nos, n, idx + 1, ctx, fim_atomo, fim_out);
    }
}

static int regex_match_from(const RegexProg *prog, const char *texto, size_t pos,
                            size_t *match_ini, size_t *match_fim) {
    if (!prog || !prog->raiz) return 0;
    RMatchCtx2 ctx;
    ctx.texto = texto;
    ctx.len = strlen(texto);
    ctx.classes = prog->classes;
    ctx.fuel = 2000000; /* limite de passos para evitar DoS */
    /* normaliza raiz em lista de nós de topo */
    RENode *topo[1];
    topo[0] = prog->raiz;
    /* tenta cada posição inicial a partir de pos */
    size_t p = pos;
    while (p <= ctx.len && ctx.fuel > 0) {
        size_t fim;
        if (re_seq(topo, 1, 0, &ctx, p, &fim)) {
            *match_ini = p;
            *match_fim = fim;
            return 1;
        }
        /* avança um byte (matches são por byte; UTF-8 preservado pois
           classes/bytes >127 só casam literais exatos) */
        p++;
    }
    return 0;
}

static int regex_match(const RegexProg *prog, const char *texto,
                       size_t *match_ini, size_t *match_fim) {
    return regex_match_from(prog, texto, 0, match_ini, match_fim);
}

static RegexProg *regex_compilar(const char *padrao, int *ok) {
    *ok = 0;
    if (!padrao) return NULL;
    RegexProg *prog = (RegexProg *)calloc(1, sizeof(RegexProg));
    prog->padrao = porsol_strdup(padrao);
    struct RParser ps;
    ps.p = padrao;
    ps.prog = prog;
    ps.ok = 1;
    prog->raiz = re_parse_alt(&ps);
    if (!ps.ok || *ps.p != '\0') {
        return prog; /* ok fica 0 */
    }
    *ok = 1;
    return prog;
}

static int regex_padrao_valido(const char *padrao) {
    int ok = 0;
    regex_compilar(padrao, &ok);
    return ok;
}

static char *regex_escape(const char *texto) {
    size_t cap = strlen(texto) * 2 + 8, n = 0;
    char *buf = (char *)malloc(cap);
    for (const char *p = texto; *p; p++) {
        if (strchr("\\.^$|?*+()[]{}", *p)) {
            if (n + 3 >= cap) { cap *= 2; buf = (char *)realloc(buf, cap); }
            buf[n++] = '\\';
        }
        if (n + 2 >= cap) { cap *= 2; buf = (char *)realloc(buf, cap); }
        buf[n++] = *p;
    }
    buf[n] = '\0';
    return buf;
}

/* ================================================================== */
/* Rede — sockets TCP, DNS e HTTP/1.1 mínimo                           */
/* ================================================================== */

static int winsock_init(void) {
#ifdef _WIN32
    static int iniciado = 0;
    if (!iniciado) {
        WSADATA wsa;
        iniciado = (WSAStartup(MAKEWORD(2, 2), &wsa) == 0) ? 1 : -1;
    }
    return iniciado == 1 ? 0 : -1;
#else
    return 0;
#endif
}

static void close_sock(int fd) {
    if (fd < 0) return;
#ifdef _WIN32
    shutdown((SOCKET)fd, SD_BOTH);
    closesocket((SOCKET)fd);
#else
    shutdown(fd, SHUT_RDWR);
    close(fd);
#endif
}

static PorsolObj *socket_tcp_novo(int fd) {
    PorsolObj *o = (PorsolObj *)calloc(1, sizeof(PorsolObj));
    o->magic = PORSOL_OBJ_MAGIC;
    o->classe = porsol_strdup("SocketTcp");
    o->fqn = porsol_strdup("Sistema.Rede.SocketTcp");
    o->estado_kind = EST_SOCKET;
    int *f = (int *)calloc(1, sizeof(int));
    *f = fd;
    o->estado = f;
    return o;
}

static PorsolObj *servidor_tcp_novo(int fd) {
    PorsolObj *o = (PorsolObj *)calloc(1, sizeof(PorsolObj));
    o->magic = PORSOL_OBJ_MAGIC;
    o->classe = porsol_strdup("ServidorTcp");
    o->fqn = porsol_strdup("Sistema.Rede.ServidorTcp");
    o->estado_kind = EST_SOCKET;
    int *f = (int *)calloc(1, sizeof(int));
    *f = fd;
    o->estado = f;
    return o;
}

/* DNS: resolve host -> IPs. todos=0 retorna primeira; todos=1 retorna
 * todas separadas por '\n' (string única para o chamador converter). */
static char *dns_resolver_host(const char *host, int todos, int *count) {
    *count = 0;
    if (winsock_init() != 0) return NULL;
    struct addrinfo hints, *res = NULL;
    memset(&hints, 0, sizeof(hints));
    hints.ai_family = AF_UNSPEC;
    hints.ai_socktype = SOCK_STREAM;
    if (getaddrinfo(host, NULL, &hints, &res) != 0 || !res) return NULL;
    size_t cap = 256;
    char *out = (char *)malloc(cap);
    out[0] = '\0';
    int n = 0;
    for (struct addrinfo *ai = res; ai && (!todos || n < 64); ai = ai->ai_next) {
        char addr[INET6_ADDRSTRLEN] = {0};
        if (ai->ai_family == AF_INET) {
            struct sockaddr_in *sa = (struct sockaddr_in *)ai->ai_addr;
            inet_ntop(AF_INET, &sa->sin_addr, addr, sizeof(addr));
        } else if (ai->ai_family == AF_INET6) {
            struct sockaddr_in6 *sa = (struct sockaddr_in6 *)ai->ai_addr;
            inet_ntop(AF_INET6, &sa->sin6_addr, addr, sizeof(addr));
        } else {
            continue;
        }
        if (!addr[0]) continue;
        size_t need = strlen(addr) + (n ? 1 : 0) + 1;
        if (strlen(out) + need + 1 > cap) {
            cap *= 2;
            out = (char *)realloc(out, cap);
        }
        if (n) strcat(out, "\n");
        strcat(out, addr);
        n++;
        *count = n;
        if (!todos) break;
    }
    freeaddrinfo(res);
    if (!n) { free(out); return NULL; }
    return out;
}

/* parse http://host[:porta]/caminho */
static int http_parse_url(const char *url, char *host, size_t hsz,
                          int *porta, char *caminho, size_t csz) {
    const char *p = url;
    if (!strncmp(p, "http://", 7)) p += 7;
    else return 0;
    const char *barra = strchr(p, '/');
    const char *host_end = barra ? barra : p + strlen(p);
    size_t hlen = (size_t)(host_end - p);
    const char *doispontos = memchr(p, ':', hlen);
    if (doispontos) {
        size_t hn = (size_t)(doispontos - p);
        if (hn >= hsz) hn = hsz - 1;
        memcpy(host, p, hn);
        host[hn] = '\0';
        *porta = atoi(doispontos + 1);
        if (*porta <= 0) *porta = 80;
    } else {
        if (hlen >= hsz) hlen = hsz - 1;
        memcpy(host, p, hlen);
        host[hlen] = '\0';
        *porta = 80;
    }
    snprintf(caminho, csz, "%s", barra ? barra : "/");
    return host[0] != '\0';
}

static char *http_ler_tudo(int fd) {
    size_t cap = 8192, n = 0;
    char *buf = (char *)malloc(cap);
    for (;;) {
        if (n + 4096 + 1 > cap) { cap *= 2; buf = (char *)realloc(buf, cap); }
        int r = recv(fd, buf + n, 4096, 0);
        if (r <= 0) break;
        n += (size_t)r;
        buf[n] = '\0';
        /* conexão close: servidor encerra após Content-Length —
           saímos quando recv retorna 0/erro */
    }
    buf[n] = '\0';
    return buf;
}

static char *http_decodificar_chunked(const char *corpo) {
    size_t cap = strlen(corpo) + 1, n = 0;
    char *out = (char *)malloc(cap);
    out[0] = '\0';
    const char *p = corpo;
    for (;;) {
        const char *eol = strstr(p, "\r\n");
        if (!eol) break;
        /* tamanho do chunk em hex (antes de ';' se houver) */
        char hexbuf[32];
        size_t hl = (size_t)(eol - p);
        if (hl >= sizeof(hexbuf)) break;
        memcpy(hexbuf, p, hl);
        hexbuf[hl] = '\0';
        char *semi = strchr(hexbuf, ';');
        if (semi) *semi = '\0';
        unsigned long tam = strtoul(hexbuf, NULL, 16);
        if (tam == 0) break;
        p = eol + 2;
        if (strlen(p) < tam + 2) {
            /* truncado */
            size_t rest = strlen(p);
            if (n + rest + 1 > cap) { cap = n + rest + 1; out = (char *)realloc(out, cap); }
            memcpy(out + n, p, rest);
            n += rest;
            out[n] = '\0';
            break;
        }
        if (n + tam + 1 > cap) { cap = n + tam + 2; out = (char *)realloc(out, cap); }
        memcpy(out + n, p, tam);
        n += tam;
        out[n] = '\0';
        p += tam + 2; /* dados + CRLF */
    }
    return out;
}

static char *http_extrair_corpo(const char *resposta) {
    const char *sep = strstr(resposta, "\r\n\r\n");
    if (!sep) return porsol_strdup(resposta);
    const char *corpo = sep + 4;
    const char *cab = resposta;
    size_t cab_len = (size_t)(sep - resposta);
    /* chunked? */
    char *low = porsol_strndup(cab, cab_len);
    for (char *p = low; *p; p++) *p = (char)ascii_lower((unsigned char)*p);
    int chunked = strstr(low, "transfer-encoding: chunked") != NULL;
    free(low);
    if (chunked) {
        return http_decodificar_chunked(corpo);
    }
    /* content-length */
    const char *cl = strstr(low, "content-length:");
    (void)cl;
    /* refaz lowercase para busca */
    char *low2 = porsol_strndup(cab, cab_len);
    for (char *p = low2; *p; p++) *p = (char)ascii_lower((unsigned char)*p);
    cl = strstr(low2, "content-length:");
    if (cl) {
        long tam = strtol(cl + 15, NULL, 10);
        if (tam >= 0 && (size_t)tam <= strlen(corpo)) {
            free(low2);
            return porsol_strndup(corpo, (size_t)tam);
        }
    }
    free(low2);
    return porsol_strdup(corpo);
}

static char *http_request_sync(const char *metodo, const char *url,
                               const char *corpo, int *ok) {
    *ok = 0;
    if (winsock_init() != 0) return NULL;
    char host[256], caminho[2048];
    int porta = 80;
    if (!http_parse_url(url, host, sizeof(host), &porta, caminho, sizeof(caminho))) {
        fprintf(stderr, "HTTP: URL não suportada (apenas http://): %s\n", url);
        return NULL;
    }
    /* conecta */
    struct addrinfo hints, *res = NULL;
    memset(&hints, 0, sizeof(hints));
    hints.ai_family = AF_UNSPEC;
    hints.ai_socktype = SOCK_STREAM;
    char portabuf[16];
    snprintf(portabuf, sizeof(portabuf), "%d", porta);
    if (getaddrinfo(host, portabuf, &hints, &res) != 0 || !res) {
        fprintf(stderr, "HTTP: falha ao resolver %s\n", host);
        return NULL;
    }
    int fd = socket(res->ai_family, res->ai_socktype, res->ai_protocol);
    if (fd < 0) { freeaddrinfo(res); return NULL; }
    /* timeouts */
#ifdef _WIN32
    DWORD tv = 30000;
    setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, (const char *)&tv, sizeof(tv));
    setsockopt(fd, SOL_SOCKET, SO_SNDTIMEO, (const char *)&tv, sizeof(tv));
#else
    struct timeval tv = { 30, 0 };
    setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, &tv, sizeof(tv));
    setsockopt(fd, SOL_SOCKET, SO_SNDTIMEO, &tv, sizeof(tv));
#endif
    if (connect(fd, res->ai_addr, (int)res->ai_addrlen) != 0) {
        fprintf(stderr, "HTTP: falha ao conectar em %s:%d\n", host, porta);
        close_sock(fd);
        freeaddrinfo(res);
        return NULL;
    }
    freeaddrinfo(res);
    /* monta requisição */
    char hosthdr[300];
    if (porta == 80) snprintf(hosthdr, sizeof(hosthdr), "%s", host);
    else snprintf(hosthdr, sizeof(hosthdr), "%s:%d", host, porta);
    size_t corpo_len = corpo ? strlen(corpo) : 0;
    size_t req_cap = strlen(metodo) + strlen(caminho) + corpo_len + 512;
    char *req = (char *)malloc(req_cap);
    int rn = snprintf(req, req_cap,
        "%s %s HTTP/1.1\r\nHost: %s\r\nUser-Agent: PorDoSol/1.0\r\n"
        "Accept: */*\r\nContent-Length: %zu\r\nConnection: close\r\n\r\n",
        metodo, caminho, hosthdr, corpo_len);
    if (corpo && corpo_len) {
        memcpy(req + rn, corpo, corpo_len);
        rn += (int)corpo_len;
    }
    req[rn] = '\0';
    size_t sent = 0;
    while (sent < (size_t)rn) {
        int n = send(fd, req + sent, (int)((size_t)rn - sent), 0);
        if (n <= 0) { free(req); close_sock(fd); return NULL; }
        sent += (size_t)n;
    }
    free(req);
    char *resp = http_ler_tudo(fd);
    close_sock(fd);
    if (!resp) return NULL;
    char *corpo_resp = http_extrair_corpo(resp);
    free(resp);
    *ok = 1;
    return corpo_resp;
}

/* Extrai o corpo de uma resposta HTTP/1.1 completa (cabeçalhos + corpo).
 * Compartilhado com porsol_tls.c para respostas lidas via TLS. */
char *porsol_http_body_from_response(const char *resposta) {
    return http_extrair_corpo(resposta ? resposta : "");
}

#ifdef POR_DO_SOL_HAS_TLS
#include "tls/porsol_tls.h"
#endif

/* Versão pública para o async_runtime: retorna o corpo (malloc) ou NULL. */
char *porsol_http_request(const char *metodo, const char *url,
                          const char *corpo) {
    if (url && !strncmp(url, "https://", 8)) {
        /* https: delega ao runtime TLS (mbedTLS) quando linkado. */
#ifdef POR_DO_SOL_HAS_TLS
        return porsol_https_request(metodo, url, corpo);
#else
        fprintf(stderr, "HTTP: HTTPS requer o runtime TLS (porsol_tls.c) linkado\n");
        return NULL;
#endif
    }
    int ok = 0;
    char *resp = http_request_sync(metodo ? metodo : "GET",
                                   url ? url : "", corpo ? corpo : "", &ok);
    return ok ? resp : NULL;
}

/* ================================================================== */
/* API pública                                                          */
/* ================================================================== */

/* Comparação de textos (UTF-8 bytewise) para o backend LLVM. */
int porsol_texto_cmp(const char *a, const char *b) {
    return strcmp(a ? a : "", b ? b : "");
}

void porsol_native_call(PorsolValue *out, const char *chave, int argc,
                        const PorsolValue *args) {
    if (!out) return;
    *out = v_nulo();
    if (!chave) return;

    /* Construtor: chave "<Classe>::construtor" — args[0] é self. */
    const char *sep = strstr(chave, "::");
    if (sep && !strcmp(sep + 2, "construtor")) {
        PorsolObj *self = self_obj(argc, args);
        if (self) {
            const char *simples = self->classe;
            if (!strcmp(simples, "Lista") || !strcmp(simples, "Fila") ||
                !strcmp(simples, "Pilha") || !strcmp(simples, "Conjunto") ||
                !strcmp(simples, "Dicionario") || !strcmp(simples, "ConstrutorTexto")) {
                ctor_lista(self, argc, args);
            } else if (!strcmp(simples, "Data")) {
                ctor_data(self, argc, args);
            } else if (!strcmp(simples, "Aleatorio")) {
                ctor_aleatorio(self, argc, args);
            } else if (!strcmp(simples, "Horario")) {
                ctor_horario(self, argc, args);
            } else if (!strcmp(simples, "Regex")) {
                ctor_regex(self, argc, args);
            } else if (!strcmp(simples, "ClienteHttp")) {
                ctor_cliente_http(self, argc, args);
            }
            /* Mutex, Cronometro, SocketTcp, ServidorTcp: estado já
             * inicializado em porsol_obj_new; construtor é no-op. */
        }
        return;
    }

    /* Tenta como método de instância: se args[0] é objeto e a chave
     * corresponde a um método conhecido de instância, despacha. */
    PorsolObj *self = self_obj(argc, args);
    if (self && argc >= 1) {
        /* Heurística: métodos de instância conhecidos começam com o
         * nome simples da classe do objeto OU são chaves nativas com
         * prefixo da classe. Verificamos se o prefixo da chave bate. */
        const char *prefixo = self->classe;
        size_t plen = strlen(prefixo);
        if (!strncmp(chave, prefixo, plen) && chave[plen] == ':') {
            /* ajusta argc/args sem self */
            despachar_instancia(chave, self, argc - 1, args + 1, out);
            return;
        }
        /* Chaves nativas explícitas de instância (ex.: "Lista::Ordenar")
         * também começam com o nome simples — cobertas acima. */
    }

    despachar_estatico(chave, argc, args, out);
}
