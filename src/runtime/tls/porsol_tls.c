/* porsol_tls.c — cliente HTTPS mínimo sobre mbedTLS.
 *
 * Socket TCP próprio + BIO customizado; handshake TLS 1.2/1.3 com
 * validação de cadeia contra o CA bundle Mozilla embutido
 * (porsol_cacert.c) e verificação de hostname (SNI).
 *
 * Compilar junto com os fontes de tls/mbedtls/library e
 * tls/porsol_cacert.c. Requer -I tls/mbedtls/include.
 */
#include "porsol_tls.h"
#include "porsol_runtime.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "mbedtls/ssl.h"
#include "mbedtls/net_sockets.h"
#include "mbedtls/entropy.h"
#include "mbedtls/ctr_drbg.h"
#include "mbedtls/x509_crt.h"
#include "mbedtls/error.h"

#ifdef _WIN32
#ifndef _CRT_SECURE_NO_WARNINGS
#define _CRT_SECURE_NO_WARNINGS
#endif
#include <winsock2.h>
#include <ws2tcpip.h>
#include <windows.h>
typedef SOCKET tls_sock_t;
#define TLS_SOCK_INVALID INVALID_SOCKET
static int tls_winsock_init(void) {
    static volatile LONG iniciado = 0;
    if (InterlockedCompareExchange(&iniciado, 1, 0) == 0) {
        WSADATA wsa;
        if (WSAStartup(MAKEWORD(2, 2), &wsa) != 0) {
            InterlockedExchange(&iniciado, -1);
            return -1;
        }
    }
    return iniciado == 1 ? 0 : -1;
}
static void tls_close_sock(tls_sock_t fd) { closesocket(fd); }
#else
#include <unistd.h>
#include <sys/socket.h>
#include <sys/time.h>
#include <netdb.h>
#include <errno.h>
#include <fcntl.h>
typedef int tls_sock_t;
#define TLS_SOCK_INVALID (-1)
static int tls_winsock_init(void) { return 0; }
static void tls_close_sock(tls_sock_t fd) { close(fd); }
#endif

/* ---------- socket ---------- */

static tls_sock_t tls_conectar(const char *host, int porta) {
    struct addrinfo hints, *res = NULL;
    char portabuf[16];
    tls_sock_t fd = TLS_SOCK_INVALID;
    memset(&hints, 0, sizeof(hints));
    hints.ai_family = AF_UNSPEC;
    hints.ai_socktype = SOCK_STREAM;
    snprintf(portabuf, sizeof(portabuf), "%d", porta);
    if (getaddrinfo(host, portabuf, &hints, &res) != 0 || !res) {
        fprintf(stderr, "HTTPS: falha ao resolver %s\n", host);
        return TLS_SOCK_INVALID;
    }
    fd = socket(res->ai_family, res->ai_socktype, res->ai_protocol);
    if (fd == TLS_SOCK_INVALID) {
        freeaddrinfo(res);
        return TLS_SOCK_INVALID;
    }
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
        fprintf(stderr, "HTTPS: falha ao conectar em %s:%d\n", host, porta);
        tls_close_sock(fd);
        freeaddrinfo(res);
        return TLS_SOCK_INVALID;
    }
    freeaddrinfo(res);
    return fd;
}

/* ---------- BIO mbedTLS sobre socket ---------- */

static int tls_bio_send(void *ctx, const unsigned char *buf, size_t len) {
    tls_sock_t fd = (tls_sock_t)(intptr_t)ctx;
    int n = send(fd, (const char *)buf, (int)len, 0);
    if (n < 0) {
#ifdef _WIN32
        int err = WSAGetLastError();
        if (err == WSAEWOULDBLOCK) return MBEDTLS_ERR_SSL_WANT_WRITE;
#else
        if (errno == EAGAIN || errno == EWOULDBLOCK) return MBEDTLS_ERR_SSL_WANT_WRITE;
#endif
        return MBEDTLS_ERR_NET_SEND_FAILED;
    }
    return n;
}

static int tls_bio_recv(void *ctx, unsigned char *buf, size_t len) {
    tls_sock_t fd = (tls_sock_t)(intptr_t)ctx;
    int n = recv(fd, (char *)buf, (int)len, 0);
    if (n < 0) {
#ifdef _WIN32
        int err = WSAGetLastError();
        if (err == WSAEWOULDBLOCK) return MBEDTLS_ERR_SSL_WANT_READ;
#else
        if (errno == EAGAIN || errno == EWOULDBLOCK) return MBEDTLS_ERR_SSL_WANT_READ;
#endif
        return MBEDTLS_ERR_NET_RECV_FAILED;
    }
    return n; /* 0 = conexão fechada */
}

/* ---------- URL ---------- */

/* Parse de "https://host[:porta]/caminho" — porta default 443. */
static int tls_parse_url(const char *url, char *host, size_t hsz,
                         int *porta, char *caminho, size_t csz) {
    const char *p = url;
    if (strncmp(p, "https://", 8) == 0) p += 8;
    else if (strncmp(p, "http://", 7) == 0) p += 7; /* tolera */
    else return 0;
    const char *barra = strchr(p, '/');
    size_t host_len = barra ? (size_t)(barra - p) : strlen(p);
    /* separa host:porta */
    char hostport[512];
    if (host_len >= sizeof(hostport)) return 0;
    memcpy(hostport, p, host_len);
    hostport[host_len] = '\0';
    char *dois = strrchr(hostport, ':');
    if (dois) {
        *dois = '\0';
        *porta = atoi(dois + 1);
        if (*porta <= 0) *porta = 443;
    } else {
        *porta = 443;
    }
    if (!hostport[0]) return 0;
    snprintf(host, hsz, "%s", hostport);
    snprintf(caminho, csz, "%s", barra ? barra : "/");
    return 1;
}

/* ---------- leitura da resposta ---------- */

/* Lê a resposta HTTP completa via mbedtls_ssl_read até close_notify/EOF.
 * Retorna buffer alocado ou NULL. */
static char *tls_ler_resposta(mbedtls_ssl_context *ssl) {
    size_t cap = 8192, len = 0;
    char *buf = (char *)malloc(cap);
    if (!buf) return NULL;
    for (;;) {
        if (len + 2048 + 1 > cap) {
            cap *= 2;
            char *nb = (char *)realloc(buf, cap);
            if (!nb) { free(buf); return NULL; }
            buf = nb;
        }
        int n = mbedtls_ssl_read(ssl, (unsigned char *)buf + len, cap - len - 1);
        if (n > 0) {
            len += (size_t)n;
            continue;
        }
        if (n == 0) break; /* close_notify */
        if (n == MBEDTLS_ERR_SSL_WANT_READ || n == MBEDTLS_ERR_SSL_WANT_WRITE) continue;
        /* timeout/erro de rede: se já temos dados, usa o que chegou */
        if (len > 0) break;
        {
            char errbuf[200];
            mbedtls_strerror(n, errbuf, sizeof(errbuf));
            fprintf(stderr, "HTTPS: erro ao ler resposta: %s\n", errbuf);
        }
        free(buf);
        return NULL;
    }
    buf[len] = '\0';
    return buf;
}

/* ---------- API ---------- */

char *porsol_https_request(const char *metodo, const char *url,
                           const char *corpo) {
    char host[256], caminho[2048];
    int porta = 443;
    tls_sock_t fd = TLS_SOCK_INVALID;
    mbedtls_entropy_context entropy;
    mbedtls_ctr_drbg_context ctr_drbg;
    mbedtls_ssl_context ssl;
    mbedtls_ssl_config conf;
    mbedtls_x509_crt cacert;
    char *resultado = NULL;
    int ret;

    if (!url || !tls_parse_url(url, host, sizeof(host), &porta, caminho, sizeof(caminho))) {
        fprintf(stderr, "HTTPS: URL não suportada: %s\n", url ? url : "(nulo)");
        return NULL;
    }
    if (tls_winsock_init() != 0) {
        fprintf(stderr, "HTTPS: falha ao inicializar Winsock\n");
        return NULL;
    }
    fd = tls_conectar(host, porta);
    if (fd == TLS_SOCK_INVALID) return NULL;

    mbedtls_entropy_init(&entropy);
    mbedtls_ctr_drbg_init(&ctr_drbg);
    mbedtls_ssl_init(&ssl);
    mbedtls_ssl_config_init(&conf);
    mbedtls_x509_crt_init(&cacert);

    ret = mbedtls_ctr_drbg_seed(&ctr_drbg, mbedtls_entropy_func, &entropy,
                                NULL, 0);
    if (ret != 0) { fprintf(stderr, "HTTPS: ctr_drbg_seed falhou\n"); goto fim; }
    ret = mbedtls_ssl_config_defaults(&conf, MBEDTLS_SSL_IS_CLIENT,
                                      MBEDTLS_SSL_TRANSPORT_STREAM,
                                      MBEDTLS_SSL_PRESET_DEFAULT);
    if (ret != 0) { fprintf(stderr, "HTTPS: ssl_config_defaults falhou\n"); goto fim; }
    ret = mbedtls_x509_crt_parse(&cacert, porsol_cacert_pem, porsol_cacert_pem_len);
    if (ret < 0) {
        char errbuf[200];
        mbedtls_strerror(ret, errbuf, sizeof(errbuf));
        fprintf(stderr, "HTTPS: CA bundle inválido: %s\n", errbuf);
        goto fim;
    }
    mbedtls_ssl_conf_authmode(&conf, MBEDTLS_SSL_VERIFY_REQUIRED);
    mbedtls_ssl_conf_ca_chain(&conf, &cacert, NULL);
    mbedtls_ssl_conf_rng(&conf, mbedtls_ctr_drbg_random, &ctr_drbg);
    ret = mbedtls_ssl_setup(&ssl, &conf);
    if (ret != 0) {
        char errbuf[200];
        mbedtls_strerror(ret, errbuf, sizeof(errbuf));
        fprintf(stderr, "HTTPS: ssl_setup falhou: %s\n", errbuf);
        goto fim;
    }
    ret = mbedtls_ssl_set_hostname(&ssl, host);
    if (ret != 0) {
        char errbuf[200];
        mbedtls_strerror(ret, errbuf, sizeof(errbuf));
        fprintf(stderr, "HTTPS: set_hostname falhou: %s\n", errbuf);
        goto fim;
    }
    mbedtls_ssl_set_bio(&ssl, (void *)(intptr_t)fd, tls_bio_send, tls_bio_recv, NULL);

    /* handshake com loop WANT_READ/WRITE */
    while ((ret = mbedtls_ssl_handshake(&ssl)) != 0) {
        if (ret == MBEDTLS_ERR_SSL_WANT_READ || ret == MBEDTLS_ERR_SSL_WANT_WRITE) continue;
        char errbuf[300];
        mbedtls_strerror(ret, errbuf, sizeof(errbuf));
        fprintf(stderr, "HTTPS: handshake TLS falhou (%s): %s\n", host, errbuf);
        goto fim;
    }

    /* monta e envia a requisição HTTP */
    {
        size_t corpo_len = corpo ? strlen(corpo) : 0;
        char hosthdr[300];
        if (porta == 443) snprintf(hosthdr, sizeof(hosthdr), "%s", host);
        else snprintf(hosthdr, sizeof(hosthdr), "%s:%d", host, porta);
        size_t req_cap = strlen(metodo) + strlen(caminho) + corpo_len + 512;
        char *req = (char *)malloc(req_cap);
        int rn = snprintf(req, req_cap,
            "%s %s HTTP/1.1\r\nHost: %s\r\nUser-Agent: PorDoSol/1.0\r\n"
            "Accept: */*\r\nContent-Length: %zu\r\nConnection: close\r\n\r\n",
            metodo ? metodo : "GET", caminho, hosthdr, corpo_len);
        if (corpo && corpo_len) {
            memcpy(req + rn, corpo, corpo_len);
            rn += (int)corpo_len;
        }
        size_t sent = 0;
        int falhou = 0;
        while (sent < (size_t)rn) {
            int n = mbedtls_ssl_write(&ssl, (const unsigned char *)req + sent,
                                      (size_t)rn - sent);
            if (n <= 0) {
                if (n == MBEDTLS_ERR_SSL_WANT_READ || n == MBEDTLS_ERR_SSL_WANT_WRITE) continue;
                falhou = 1;
                break;
            }
            sent += (size_t)n;
        }
        free(req);
        if (falhou) { fprintf(stderr, "HTTPS: falha ao enviar requisição\n"); goto fim; }
    }

    {
        char *resposta = tls_ler_resposta(&ssl);
        if (!resposta) fprintf(stderr, "HTTPS: falha ao ler resposta\n");
        if (resposta) {
            resultado = porsol_http_body_from_response(resposta);
            free(resposta);
        }
    }

fim:
    mbedtls_x509_crt_free(&cacert);
    mbedtls_ssl_free(&ssl);
    mbedtls_ssl_config_free(&conf);
    mbedtls_ctr_drbg_free(&ctr_drbg);
    mbedtls_entropy_free(&entropy);
    tls_close_sock(fd);
    return resultado;
}
