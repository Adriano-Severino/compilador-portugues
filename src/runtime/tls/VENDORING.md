# Runtime TLS vendored (mbedTLS)

Este diretório contém o runtime TLS usado pelo backend LLVM para suportar
`https://` em `Sistema.Rede.ClienteHttp`.

## Componentes

- `mbedtls/` — [mbedTLS](https://github.com/Mbed-TLS/mbedtls) **3.6.7**
  (Apache-2.0, ver `mbedtls/LICENSE`), reduzido aos fontes necessários:
  `include/`, `library/` e `3rdparty/everest/` (X25519).
- `porsol_tls.c` / `porsol_tls.h` — glue C: socket TCP + BIO mbedTLS,
  handshake TLS 1.2/1.3, SNI e verificação de hostname.
- `cacert.pem` — bundle de certificados raiz da Mozilla
  (<https://curl.se/ca/cacert.pem>), fonte das raízes confiáveis.
- `porsol_cacert.c` — `cacert.pem` embutido como array C (gerado;
  atualize periodicamente baixando uma versão nova do bundle).

## Build

O compilador pré-compila `libporsol_tls.a` uma vez em
`compilador-portugues/target/tls-cache/` (compilação paralela com clang,
`-ffunction-sections -fdata-sections`) e linka a biblioteca estática
somente quando o programa usa `ClienteHttp`. Programas sem `ClienteHttp`
não carregam código TLS.

Para atualizar o bundle de CAs: baixe `cacert.pem` de
<https://curl.se/ca/cacert.pem>, substitua o arquivo e regenere
`porsol_cacert.c` (array `porsol_cacert_pem` + `porsol_cacert_pem_len`,
com byte NUL final).

## Limitações

- Sem suporte a certificado de cliente (mTLS).
- HTTP/1.1 apenas, sem keep-alive.
- A validação de certificados é obrigatória (VERIFY_REQUIRED); não há
  modo "aceitar qualquer certificado".
