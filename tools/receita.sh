#!/usr/bin/env bash
# Envia receitas para o Kindle (aparecem em Mais → Receitas).
#   tools/receita.sh minha-receita.txt [...]
#   cat receita.txt | tools/receita.sh --nome bolo-de-cenoura
# Formato: "# Título", linha de rendimento, "## Ingredientes" com "- item", "## Preparo" com "1. passo".
set -euo pipefail
IP="${KINDLE_IP:-192.168.0.21}"
KEY="${KINDLE_KEY:-$HOME/.ssh/kindle_ed25519}"
PORT="${KINDLE_PORT:-2223}"
SSH=(ssh -i "$KEY" -p "$PORT" -o IdentitiesOnly=yes -o ConnectTimeout=6 "root@$IP")
DEST=/mnt/us/verso-ink/data/receitas
put() { "${SSH[@]}" "mkdir -p $DEST && cat > '$DEST/$2.txt'" <"$1"; echo "enviada: $2"; }
if [ "${1:-}" = "--nome" ]; then
    tmp="$(mktemp)"; trap 'rm -f "$tmp"' EXIT; cat >"$tmp"
    put "$tmp" "$(printf '%s' "${2:?nome}" | tr -cd 'a-zA-Z0-9-')"
else
    for f in "$@"; do put "$f" "$(basename "$f" .txt | tr -cd 'a-zA-Z0-9-')"; done
fi
