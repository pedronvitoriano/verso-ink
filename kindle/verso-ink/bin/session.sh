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
# Com a interface da Amazon congelada (troca rápida), não precisa parar nada.
amazon_frozen || amazon_stop

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
    # Teste da troca rápida pelo SSH: este arquivo faz a saída valer como "abrir a Amazon".
    [ -f /tmp/verso-ink.test-44 ] && rm -f /tmp/verso-ink.test-44 && rc=44
    # 43 = atualizado pelo próprio Kindle: copia os arquivos novos para /tmp e reabre.
    if [ "$rc" -eq 43 ]; then
        stage
        log "atualizado; reabrindo"
        RETURN=""
        continue
    fi
    # 44 = abrir o leitor da Amazon (livros da loja, com DRM) e voltar quando a pessoa tocar em Início.
    if [ "$rc" -eq 44 ]; then
        if amazon_frozen; then
            amazon_thaw
        else
            # Primeira vez: a interface sobe (~1 min). Sem marcar "não reabrir": a sessão continua.
            "$RUN/verso-ink" splash-amazon 2>>"$LOG"
            trap "" TERM
            start lab126_gui >>"$LOG" 2>&1
            trap - TERM
            log "interface da Amazon ligada (troca rápida)"
        fi
        amazon_wait_home
        amazon_freeze
        log "de volta ao Verso Ink"
        RETURN=1
        continue
    fi
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
# Fechando o Verso Ink: a interface da Amazon volta (descongelada, se estava congelada).
if amazon_frozen; then amazon_thaw; else amazon_start; fi
exit 0
