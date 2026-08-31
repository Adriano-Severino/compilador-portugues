#!/bin/bash
set -e

VERSION=${1:-"0.1.4"}
OUTPUT_DIR=${2:-"dist"}
SKIP_TESTS=${3:-0}

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_DIR"

echo "=============================================="
echo " Compilador Por do Sol - Build Release v$VERSION"
echo "=============================================="

if [ "$SKIP_TESTS" -eq 0 ]; then
    echo -e "\n[1/3] Executando testes..."
    cargo test
    echo "✓ Todos os testes passaram!"
else
    echo -e "\n[1/3] Testes ignorados."
fi

echo -e "\n[2/3] Compilando binários em modo Release..."
cargo build --release --bin compilador --bin interpretador
echo "✓ Binários compilados com sucesso!"

OS_NAME="linux"
if [[ "$OSTYPE" == "darwin"* ]]; then
    OS_NAME="macos"
fi

PKG_NAME="compilador-portugues-v${VERSION}-${OS_NAME}-x64"
DIST_DIR="${PROJECT_DIR}/${OUTPUT_DIR}"
PKG_DIR="${DIST_DIR}/${PKG_NAME}"

rm -rf "$PKG_DIR"
mkdir -p "${PKG_DIR}/bin"

cp target/release/compilador "${PKG_DIR}/bin/"
cp target/release/interpretador "${PKG_DIR}/bin/"

for doc in README.md LICENSE agent.md; do
    if [ -f "$doc" ]; then
        cp "$doc" "${PKG_DIR}/"
    fi
done

echo -e "\n[3/3] Criando arquivo tar.gz..."
mkdir -p "$DIST_DIR"
tar -czf "${DIST_DIR}/${PKG_NAME}.tar.gz" -C "$DIST_DIR" "$PKG_NAME"

cd "$DIST_DIR"
if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "${PKG_NAME}.tar.gz" > "${PKG_NAME}.tar.gz.sha256"
else
    shasum -a 256 "${PKG_NAME}.tar.gz" > "${PKG_NAME}.tar.gz.sha256"
fi

echo "=============================================="
echo " Release gerada com sucesso!"
echo " Arquivo: ${DIST_DIR}/${PKG_NAME}.tar.gz"
echo " Checksum: ${DIST_DIR}/${PKG_NAME}.tar.gz.sha256"
echo "=============================================="
