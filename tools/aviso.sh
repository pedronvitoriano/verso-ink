#!/usr/bin/env bash
# Aviso para o Kindle (aparece na Início e na Edição da manhã).
# Uso: tools/aviso.sh "texto" [critico|info] [minutos]
#   ex.: tools/aviso.sh "NÃO DESCONECTAR O KINDLE até o fim da atualização" critico 120
set -euo pipefail
. "$(dirname "$0")/ponte.sh"
text="${1:?uso: aviso.sh \"texto\" [critico|info] [minutos]}"
level="${2:-info}"
mins="${3:-720}"
body="$(jq -nc --arg t "$text" --arg l "$level" --argjson m "$mins" '{text: $t, level: $l, minutes: $m}')"
ponte POST /avisos "$body" >/dev/null && echo "aviso enviado"
