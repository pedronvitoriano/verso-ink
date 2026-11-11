#!/usr/bin/env bash
# Edita o dia do Verso Ink (prioridades, tarefas, lembretes, hábitos) pelo PC.
#
#   tools/dia.sh                 abre o dia no $EDITOR e envia ao salvar
#   tools/dia.sh < meu-dia.txt   envia um arquivo pronto (útil para o Claude)
#
# Formato (uma linha por item):
#   ! Prioridade (até 3)     - Tarefa     @ 14:30 Lembrete     * Hábito     # comentário
set -euo pipefail
IP="${KINDLE_IP:-192.168.0.21}"
KEY="${KINDLE_KEY:-$HOME/.ssh/kindle_ed25519}"
PORT="${KINDLE_PORT:-2223}"
SSH=(ssh -i "$KEY" -p "$PORT" -o IdentitiesOnly=yes -o ConnectTimeout=6 "root@$IP")
FILE=/mnt/us/verso-ink/data/dia.txt

push() { "${SSH[@]}" "mkdir -p /mnt/us/verso-ink/data && cat > $FILE && sync && echo today > /tmp/verso-ink.cmd"; echo "dia enviado"; }

if [ ! -t 0 ]; then
    push
    exit 0
fi
tmp="$(mktemp --suffix=.txt)"
trap 'rm -f "$tmp"' EXIT
"${SSH[@]}" "cat $FILE 2>/dev/null" >"$tmp" || true
if [ ! -s "$tmp" ]; then
    printf '# ! prioridade   - tarefa   @ HH:MM lembrete   * hábito\n! \n- \n@ 09:00 \n* Ler 20 min\n' >"$tmp"
fi
"${EDITOR:-nano}" "$tmp"
push <"$tmp"
