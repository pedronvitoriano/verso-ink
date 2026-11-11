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
    # Teste do gesto: injeta um dedo segurando o canto durante o portão (uma vez só).
    if [ -f "$H/settings/dev_gesture_test" ]; then
        rm -f "$H/settings/dev_gesture_test"
        (usleep 1200000; "$RUN/verso-ink" tap $((60 + $$ % 90)) 120 2200 >>"$LOG" 2>&1; log "toque de teste: rc $?") &
        log "teste do gesto: toque injetado no canto"
    fi
    VERSO_INK_HOME="$H" "$RUN/verso-ink" gate
    if [ $? -ne 0 ]; then
        log "portão: pulando, abrindo o Kindle original"
        rm -f /tmp/verso-ink.pid
        amazon_start
        exit 0
    fi
fi

RETURN=""
while :; do
    VERSO_INK_HOME="$H" VERSO_INK_RETURN="$RETURN" "$RUN/verso-ink" run
    rc=$?
    log "verso-ink saiu com $rc"
    # 42 = abrir um livro no KOReader e voltar para o Verso Ink quando ele fechar.
    [ "$rc" -eq 42 ] && [ -f /tmp/verso-ink.open-file ] || break
    BOOK="$(cat /tmp/verso-ink.open-file)"
    rm -f /tmp/verso-ink.open-file
    start=$(date +%s)
    log "KOReader: $BOOK"
    /mnt/us/koreader/koreader.sh "$BOOK" >>"$LOG" 2>&1
    end=$(date +%s)
    # Minutos lidos (início em segundos UTC e duração), para a meta de leitura.
    mkdir -p "$H/data" && echo "$start $((end - start))" >>"$H/data/reading.log"
    log "KOReader fechou depois de $((end - start)) s"
    RETURN=1
done
rm -f /tmp/verso-ink.pid
amazon_start
exit 0
