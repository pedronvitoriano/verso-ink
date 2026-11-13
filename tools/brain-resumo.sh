#!/usr/bin/env bash
# Manda para a Edição da manhã e para o Verso só um retrato agregado do Brain.
# Títulos, arquivos pessoais, "onde parou" e qualquer conteúdo sensível ficam no PC.
. "$(dirname "$0")/ponte.sh"
BRAIN="${BRAIN:-$HOME/dev/aions-brain/Brain}"
active=0
for f in "$BRAIN"/Pessoal/Handoffs/*.md; do
    [ -f "$f" ] || continue
    grep -q '^status: in-progress' "$f" || continue
    active=$((active + 1))
done
if [ -f "$BRAIN/00 Home/Adequações do Brain.md" ]; then
    n="$(grep -c '^- \[ \]' "$BRAIN/00 Home/Adequações do Brain.md" 2>/dev/null)"; n="${n:-0}"
else
    n=0
fi
ponte POST /brain "$(jq -nc --argjson active "$active" --argjson adequacies "$n" '{active: $active, adequacies: $adequacies}')" 15 >/dev/null && echo "resumo do Brain enviado"
