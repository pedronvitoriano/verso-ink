#!/usr/bin/env bash
# Publica uma versão no GitHub com o pacote que o Kindle baixa em Ajustes → Procurar atualização.
# Uso: tools/release.sh   (usa a versão do Cargo.toml; a tag é v<versão>)
set -euo pipefail
cd "$(dirname "$0")/.."
TARGET=armv7-unknown-linux-musleabi
VERSION="$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)"
cargo test --release
cargo build --release --target "$TARGET"

OUT="$(mktemp -d)"
trap 'rm -rf "$OUT"' EXIT
mkdir -p "$OUT/pkg/verso-ink/bin" "$OUT/pkg/verso-ink/upstart"
cp kindle/verso-ink/bin/*.sh "$OUT/pkg/verso-ink/bin/"
cp "target/$TARGET/release/verso-ink" "$OUT/pkg/verso-ink/bin/"
[ -f vendor/stockfish ] || tools/build-stockfish.sh
cp vendor/stockfish "$OUT/pkg/verso-ink/bin/"
# Rostos dos treinadores (reduzidos por tools/rostos.sh).
[ -d assets/treinadores ] && cp -r assets/treinadores "$OUT/pkg/verso-ink/"
[ -d assets/sprites ] && cp -r assets/sprites "$OUT/pkg/verso-ink/"
cp kindle/upstart/verso-ink.conf "$OUT/pkg/verso-ink/upstart/"
cp -r kindle/extensions "$OUT/pkg/"
# O dropbear (SSH) vem do KOReader já instalado no Kindle; não vai no pacote.
tar -C "$OUT/pkg" -czf "$OUT/verso-ink-kindle.tar.gz" verso-ink extensions
(cd "$OUT" && sha256sum verso-ink-kindle.tar.gz >verso-ink-kindle.tar.gz.sha256)
gh release create "v$VERSION" "$OUT/verso-ink-kindle.tar.gz" "$OUT/verso-ink-kindle.tar.gz.sha256" \
    --title "Verso Ink $VERSION" --notes "${NOTES:-Pacote para o Kindle: extrair em /mnt/us, ou usar Ajustes → Procurar atualização.}"
echo "publicado v$VERSION"
