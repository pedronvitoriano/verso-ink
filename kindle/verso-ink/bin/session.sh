#!/bin/sh
# Uma sessão do Verso Ink. Uso: session.sh [boot]
# - boot: passa pelo portão (DISABLE, 3 falhas, gesto do canto) antes de abrir.
# Ao sair (fechado pelo ctl.sh ou por falha), a interface da Amazon volta.
. /tmp/verso-ink/common.sh

if [ -f /tmp/verso-ink.pid ] && kill -0 "$(cat /tmp/verso-ink.pid)" 2>/dev/null; then
    log "já aberto, ignorando"
    exit 0
fi
echo $$ >/tmp/verso-ink.pid

ssh_up
# Modo de teste: impede o Kindle de dormir (o powerd esquece isso a cada boot).
[ -f "$H/settings/dev_keep_awake" ] && lipc-set-prop com.lab126.powerd preventScreenSaver 1 && log "modo de teste: sem repouso"
amazon_stop

if [ "$1" = "boot" ]; then
    VERSO_INK_HOME="$H" "$RUN/verso-ink" gate
    if [ $? -ne 0 ]; then
        log "portão: pulando, abrindo o Kindle original"
        rm -f /tmp/verso-ink.pid
        amazon_start
        exit 0
    fi
fi

VERSO_INK_HOME="$H" "$RUN/verso-ink" run
rc=$?
log "verso-ink saiu com $rc"
rm -f /tmp/verso-ink.pid
amazon_start
exit 0
