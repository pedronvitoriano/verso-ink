#!/bin/sh
# Controle do Verso Ink. Pode ser chamado de /mnt/us (KUAL, SSH) ou de /tmp.
#   ctl.sh open          abre agora (pausa a interface da Amazon)
#   ctl.sh close         fecha e volta para a interface da Amazon
#   ctl.sh boot-on       o Kindle passa a ligar no Verso Ink
#   ctl.sh boot-off      o Kindle volta a ligar na interface da Amazon
#   ctl.sh reload        troca pelo binário novo sem passar pela interface da Amazon
#   ctl.sh status        mostra o estado
#   ctl.sh boot          (uso interno do job de inicialização)
H=/mnt/us/verso-ink
JOB=/etc/upstart/verso-ink.conf

stage_and() {
    # Copia para /tmp e continua de lá, desligado do terminal e do KUAL.
    . "$H/bin/common.sh"
    stage
    setsid "$RUN/session.sh" "$@" </dev/null >/dev/null 2>&1 &
}

case "$1" in
    open)
        stage_and
        echo "abrindo"
        ;;
    boot)
        if [ -f /tmp/verso-ink.no-autostart ]; then
            rm -f /tmp/verso-ink.no-autostart
            exit 0
        fi
        stage_and boot
        ;;
    close)
        if [ -f /tmp/verso-ink.pid ]; then
            # O processo da interface é filho da sessão; ao sair, a sessão retoma a Amazon.
            pkill -TERM -f "/tmp/verso-ink/verso-ink run" && echo "fechando" || echo "não estava aberto"
        else
            echo "não estava aberto"
        fi
        ;;
    reload)
        # Mata a sessão antes do binário, para ela não retomar a Amazon no caminho.
        [ -f /tmp/verso-ink.pid ] && kill -9 "$(cat /tmp/verso-ink.pid)" 2>/dev/null
        pkill -TERM -f "/tmp/verso-ink/verso-ink run"
        rm -f /tmp/verso-ink.pid
        sleep 1
        stage_and
        echo "recarregado"
        ;;
    boot-on)
        rm -f "$H/DISABLE"
        mkdir -p "$H/state" && echo 0 >"$H/state/fail_count"
        mntroot rw >/dev/null && cp "$H/upstart/verso-ink.conf" "$JOB" && sync && mntroot ro >/dev/null
        [ -f "$JOB" ] && echo "boot direto ligado" || { echo "falhou ao instalar o job"; exit 1; }
        ;;
    boot-off)
        mntroot rw >/dev/null && rm -f "$JOB" && sync && mntroot ro >/dev/null
        [ -f "$JOB" ] && { echo "falhou ao remover o job"; exit 1; } || echo "boot direto desligado"
        ;;
    status)
        [ -f "$JOB" ] && echo "boot direto: ligado" || echo "boot direto: desligado"
        [ -f "$H/DISABLE" ] && echo "DISABLE: presente (boot direto pulado)"
        echo "falhas seguidas: $(cat "$H/state/fail_count" 2>/dev/null || echo 0)"
        if [ -f /tmp/verso-ink.pid ] && kill -0 "$(cat /tmp/verso-ink.pid)" 2>/dev/null; then
            echo "sessão: aberta"
        else
            echo "sessão: fechada"
        fi
        ;;
    *)
        echo "uso: ctl.sh open|close|boot-on|boot-off|status"
        exit 2
        ;;
esac
