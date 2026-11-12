#!/usr/bin/env bash
# Funções comuns dos scripts do PC que falam com a ponte do Verso Ink na VPS.
# Endereço e chave em ~/.config/verso-ink/url e ~/.config/verso-ink/token (os mesmos do Kindle).
CONF="${VERSO_INK_CONF:-$HOME/.config/verso-ink}"
PONTE_URL="$(cat "$CONF/url" 2>/dev/null)"
PONTE_TOKEN="$(cat "$CONF/token" 2>/dev/null)"

# ponte MÉTODO CAMINHO [JSON] [segundos]: imprime a resposta; falha sem configuração ou sem rede.
ponte() {
    [ -n "$PONTE_URL" ] && [ -n "$PONTE_TOKEN" ] || return 1
    local args=(-s -f -m "${4:-10}" -X "$1" -H "Authorization: Bearer $PONTE_TOKEN")
    # DNS da operadora falhando: conexão direta ao IP, mantendo o nome (ver ~/.config/verso-ink/resolve).
    local rs; rs="$(cat "$CONF/resolve" 2>/dev/null)"
    [ -n "$rs" ] && args+=(--resolve "$rs")
    [ -n "${3:-}" ] && args+=(-H "Content-Type: application/json" --data-binary "$3")
    curl "${args[@]}" "${PONTE_URL%/}$2"
}
