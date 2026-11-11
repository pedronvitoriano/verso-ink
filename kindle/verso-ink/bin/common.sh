#!/bin/sh
# Funções comuns. Tudo roda a partir de /tmp/verso-ink, nunca de /mnt/us:
# assim o modo USB (que desmonta /mnt/us) continua funcionando com o Verso Ink aberto.

H=/mnt/us/verso-ink
RUN=/tmp/verso-ink
LOG=/tmp/verso-ink.log
SSH_PORT=2223

log() { echo "[$(date -u '+%H:%M:%S')] [sh] $*" >>"$LOG"; }

# Copia binários, chaves e scripts para /tmp (idempotente).
stage() {
    mkdir -p "$RUN/settings/SSH"
    cp "$H/bin/verso-ink" "$H/bin/dropbear" "$H/bin/"*.sh "$RUN/" 2>/dev/null
    cp "$H/settings/SSH/"* "$RUN/settings/SSH/" 2>/dev/null
    chmod +x "$RUN/verso-ink" "$RUN/dropbear" "$RUN/"*.sh
}

# SSH próprio (porta 2223, só chave). É a linha de vida: sobe antes de tudo.
ssh_up() {
    [ -f /tmp/dropbear_versoink.pid ] && kill -0 "$(cat /tmp/dropbear_versoink.pid)" 2>/dev/null && return 0
    (cd "$RUN" && ./dropbear -E -R -s -p "$SSH_PORT" -P /tmp/dropbear_versoink.pid 2>>"$LOG")
    # Preserva a chave do servidor entre boots.
    cp "$RUN/settings/SSH/dropbear_"*_host_key "$H/settings/SSH/" 2>/dev/null
    iptables -C INPUT -p tcp --dport "$SSH_PORT" -m conntrack --ctstate NEW,ESTABLISHED -j ACCEPT 2>/dev/null ||
        iptables -A INPUT -p tcp --dport "$SSH_PORT" -m conntrack --ctstate NEW,ESTABLISHED -j ACCEPT
    iptables -C OUTPUT -p tcp --sport "$SSH_PORT" -m conntrack --ctstate ESTABLISHED -j ACCEPT 2>/dev/null ||
        iptables -A OUTPUT -p tcp --sport "$SSH_PORT" -m conntrack --ctstate ESTABLISHED -j ACCEPT
    log "ssh na porta $SSH_PORT"
}

amazon_stop() {
    # O job do framework manda TERM ao parar; ignoramos para não cair junto (como o KOReader).
    trap "" TERM
    stop lab126_gui >/dev/null 2>&1
    usleep 1250000
    trap - TERM
    # Sem a interface, o blanket fica com a tela de abertura ("splash") carregada e o powerd
    # passa a ignorar o botão de ligar ("Splash screen is on"). Descarregar devolve o botão.
    lipc-set-prop com.lab126.blanket unload splash 2>/dev/null
    log "interface da Amazon parada"
}

amazon_start() {
    # Marca para o job de boot não reabrir o Verso Ink quando a interface subir.
    touch /tmp/verso-ink.no-autostart
    # A interface da Amazon leva cerca de um minuto para desenhar: avisamos na tela.
    "$RUN/verso-ink" splash-amazon 2>>"$LOG"
    # Em primeiro plano: em segundo plano o `start` morre junto com a sessão.
    if start lab126_gui >>"$LOG" 2>&1; then
        log "interface da Amazon retomada"
    else
        log "falha ao retomar a interface da Amazon; tentando de novo"
        sleep 2
        start lab126_gui >>"$LOG" 2>&1
    fi
}
