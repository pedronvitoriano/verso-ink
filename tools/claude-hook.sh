#!/usr/bin/env bash
# Ponte Claude Code → Kindle: cada evento de sessão vira uma linha que o Verso Ink mostra ao vivo
# no bloco Claude. Nunca atrasa o Claude Code: envia em segundo plano e sai na hora.
#
# Instalação (hooks do Claude Code, em ~/.claude/settings.json), para os eventos
# SessionStart, UserPromptSubmit, PreToolUse, Notification, Stop e SessionEnd:
#   { "type": "command", "command": "/caminho/para/verso-ink/tools/claude-hook.sh" }
# Variáveis: KINDLE_IP (padrão 192.168.0.21), KINDLE_KEY, KINDLE_PORT (padrão 2223).
IP="${KINDLE_IP:-192.168.0.21}"
KEY="${KINDLE_KEY:-$HOME/.ssh/kindle_ed25519}"
PORT="${KINDLE_PORT:-2223}"

json="$(cat)"
get() { printf '%s' "$json" | jq -r "$1 // empty" 2>/dev/null; }
event="$(get .hook_event_name)"
session="$(get .session_id | tr -cd 'a-zA-Z0-9-' | cut -c1-36)"
project="$(basename "$(get .cwd)")"
[ -n "$session" ] || exit 0

case "$event" in
    SessionStart) state="aberta"; detail="" ;;
    UserPromptSubmit) state="pensando"; detail="" ;;
    PreToolUse)
        state="trabalhando"
        tool="$(get .tool_name)"
        file="$(get .tool_input.file_path)"
        detail="$tool${file:+ · $(basename "$file")}"
        ;;
    Notification) state="precisa de você"; detail="$(get .message | cut -c1-80)" ;;
    Stop) state="sua vez"; detail="terminou o pedido" ;;
    SessionEnd) state="encerrada"; detail="" ;;
    *) exit 0 ;;
esac

line="$(date +%s)|$project|$state|$detail"
# Também para a VPS (vale fora de casa e alimenta a Edição da manhã). Uma vez por sessão, ao
# abrir, manda também o resumo do Brain.
(
    . "$(dirname "$0")/ponte.sh"
    ponte POST /sessoes "$(jq -nc --arg s "$session" --arg p "$project" --arg st "$state" --arg d "$detail" '{session: $s, project: $p, state: $st, detail: $d}')" 5 >/dev/null 2>&1
    [ "$event" = "SessionStart" ] && "$(dirname "$0")/brain-resumo.sh" >/dev/null 2>&1
) &
(
    printf '%s\n' "$line" | ssh -i "$KEY" -p "$PORT" -o IdentitiesOnly=yes -o ConnectTimeout=2 -o BatchMode=yes "root@$IP" \
        "mkdir -p /mnt/us/verso-ink/data/claude && cat > /mnt/us/verso-ink/data/claude/$session.txt" >/dev/null 2>&1
) &
disown 2>/dev/null
exit 0
