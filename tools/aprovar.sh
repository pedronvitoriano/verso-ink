#!/usr/bin/env bash
# Hook PermissionRequest do Claude Code: com o modo "longe" ligado no Kindle, o pedido vai para
# o Kindle e espera a resposta (até 10 minutos; sem resposta, é negado). Desligado, ou sem rede,
# o Claude Code mostra o diálogo normal no PC.
#
# settings.json:
#   "PermissionRequest": [{"matcher": "*", "hooks": [{"type": "command",
#      "command": "/caminho/verso-ink/tools/aprovar.sh", "timeout": 660}]}]
. "$(dirname "$0")/ponte.sh"
json="$(cat)"
on="$(ponte GET /longe "" 4 | jq -r '.on // false' 2>/dev/null)"
[ "$on" = "true" ] || exit 0

tool="$(printf '%s' "$json" | jq -r '.tool_name // "?"')"
project="$(basename "$(printf '%s' "$json" | jq -r '.cwd // ""')")"
# Resumo legível do que o agente quer fazer (comando, arquivo ou o pedido inteiro, cortado).
summary="$(printf '%s' "$json" | jq -r '.tool_input | (.command // .file_path // .url // (tostring)) ' | cut -c1-600)"
desc="$(printf '%s' "$json" | jq -r '.tool_input.description // empty')"
[ -n "$desc" ] && summary="$desc: $summary"
body="$(jq -nc --arg p "$project" --arg t "$tool" --arg s "$summary" '{project: $p, tool: $t, summary: $s}')"
id="$(ponte POST /aprovacoes "$body" 10 | jq -r '.id // empty')"
[ -n "$id" ] || exit 0

decide() {
    if [ "$1" = "allow" ]; then
        printf '{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"allow"}}}\n'
    else
        printf '{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"deny","message":"%s"}}}\n' "$2"
    fi
    exit 0
}
end=$(( $(date +%s) + 620 ))
while [ "$(date +%s)" -lt "$end" ]; do
    st="$(ponte GET "/aprovacoes/$id" "" 8 | jq -r '.status // empty' 2>/dev/null)"
    case "$st" in
        permitido) decide allow ;;
        negado) decide deny "Negado pelo Kindle (ou sem resposta em 10 minutos)." ;;
    esac
    sleep 3
done
decide deny "Sem resposta em 10 minutos."
