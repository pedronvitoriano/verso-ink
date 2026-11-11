#!/usr/bin/env bash
# Envia livros ou textos para o Kindle pelo Wi-Fi. Aparecem em Ler, no topo (mais recentes primeiro).
#
#   tools/send.sh livro.epub outro.pdf          envia arquivos
#   echo "texto" | tools/send.sh --texto "Título"   cria um .txt com o título e o texto
#
# Variáveis: KINDLE_IP (padrão 192.168.0.21), KINDLE_KEY, KINDLE_PORT (padrão 2223).
set -euo pipefail
IP="${KINDLE_IP:-192.168.0.21}"
KEY="${KINDLE_KEY:-$HOME/.ssh/kindle_ed25519}"
PORT="${KINDLE_PORT:-2223}"
DEST="/mnt/us/documents/Verso Ink"
SSH=(ssh -i "$KEY" -p "$PORT" -o IdentitiesOnly=yes -o ConnectTimeout=6 "root@$IP")

put() { # put <arquivo local> <nome no Kindle>
    local q
    q=$(printf '%q' "$DEST/$2")
    "${SSH[@]}" "mkdir -p '$DEST' && cat > $q" <"$1"
    echo "enviado: $2"
}

if [ "${1:-}" = "--texto" ]; then
    title="${2:?uso: echo texto | tools/send.sh --texto \"Título\"}"
    tmp="$(mktemp)"
    trap 'rm -f "$tmp"' EXIT
    { printf '%s\n\n' "$title"; cat; } >"$tmp"
    safe="$(printf '%s' "$title" | tr '/\\:*?"<>|' '_________')"
    put "$tmp" "$safe.txt"
else
    [ $# -gt 0 ] || { echo "uso: tools/send.sh arquivo [...]  |  echo texto | tools/send.sh --texto \"Título\""; exit 2; }
    for f in "$@"; do
        put "$f" "$(basename "$f")"
    done
fi
# Se a tela Ler estiver aberta, ela se atualiza.
"${SSH[@]}" 'sync; [ -f /tmp/verso-ink.pid ] && echo refresh > /tmp/verso-ink.cmd; true'
