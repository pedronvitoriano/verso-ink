#!/usr/bin/env bash
# Manda para a Edição da manhã as pendências do Brain: retomadas em andamento e adequações.
# Só títulos e o "Onde parou" curto; nada de conteúdo sensível (fica no PC).
. "$(dirname "$0")/ponte.sh"
BRAIN="${BRAIN:-$HOME/dev/aions-brain/Brain}"
out=""
for f in "$BRAIN"/Pessoal/Handoffs/*.md; do
    [ -f "$f" ] || continue
    grep -q '^status: in-progress' "$f" || continue
    title="$(grep -m1 '^# ' "$f" | sed 's/^# //')"
    out+="- Retomada em andamento: $title"$'\n'
done
if [ -f "$BRAIN/00 Home/Adequações do Brain.md" ]; then
    n="$(grep -c '^- \[ \]' "$BRAIN/00 Home/Adequações do Brain.md" 2>/dev/null || echo 0)"
    [ "$n" -gt 0 ] && out+="- $n adequações do Brain pendentes"$'\n'
fi
[ -n "$out" ] || out="- Nenhuma pendência registrada no Brain."
ponte POST /brain "$(jq -nc --arg t "$out" '{text: $t}')" 15 >/dev/null && echo "resumo do Brain enviado"
