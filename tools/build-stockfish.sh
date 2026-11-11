#!/usr/bin/env bash
# Compila o Stockfish 11 (GPLv3, https://github.com/official-stockfish/Stockfish/tree/sf_11)
# para o Kindle: ARMv7, estático (musl), num contêiner Docker. Saída: vendor/stockfish.
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p vendor
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
curl -sL https://github.com/official-stockfish/Stockfish/archive/refs/tags/sf_11.tar.gz | tar -xz -C "$TMP"
docker run --rm -v "$TMP/Stockfish-sf_11/src:/src" -w /src messense/rust-musl-cross:armv7-musleabi sh -c \
    'make -j"$(nproc)" build ARCH=armv7 COMP=gcc COMPCXX=armv7-unknown-linux-musleabi-g++ EXTRALDFLAGS="-static -latomic" >/dev/null && armv7-unknown-linux-musleabi-strip stockfish'
cp "$TMP/Stockfish-sf_11/src/stockfish" vendor/stockfish
echo "vendor/stockfish pronto"
