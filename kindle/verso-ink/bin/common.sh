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
    cp "$H/bin/verso-ink" "$H/bin/dropbear" "$H/bin/stockfish" "$H/bin/"*.sh "$RUN/" 2>/dev/null
    # Cliente SSH do KOReader, para o túnel (fica em /tmp: o modo USB desmonta /mnt/us).
    [ -x /mnt/us/koreader/dbclient ] && cp /mnt/us/koreader/dbclient "$RUN/dbclient" 2>/dev/null
    cp "$H/settings/SSH/"* "$RUN/settings/SSH/" 2>/dev/null
    chmod +x "$RUN/verso-ink" "$RUN/dropbear" "$RUN/"*.sh "$RUN/stockfish" 2>/dev/null
}

# SSH próprio (porta 2223, só chave). É a linha de vida: sobe antes de tudo.
ssh_up() {
    if [ -f /tmp/dropbear_versoink.pid ] && kill -0 "$(cat /tmp/dropbear_versoink.pid)" 2>/dev/null; then
        tunel_up
        return 0
    fi
    (cd "$RUN" && ./dropbear -E -R -s -p "$SSH_PORT" -P /tmp/dropbear_versoink.pid 2>>"$LOG")
    # Preserva a chave do servidor entre boots.
    cp "$RUN/settings/SSH/dropbear_"*_host_key "$H/settings/SSH/" 2>/dev/null
    iptables -C INPUT -p tcp --dport "$SSH_PORT" -m conntrack --ctstate NEW,ESTABLISHED -j ACCEPT 2>/dev/null ||
        iptables -A INPUT -p tcp --dport "$SSH_PORT" -m conntrack --ctstate NEW,ESTABLISHED -j ACCEPT
    iptables -C OUTPUT -p tcp --sport "$SSH_PORT" -m conntrack --ctstate ESTABLISHED -j ACCEPT 2>/dev/null ||
        iptables -A OUTPUT -p tcp --sport "$SSH_PORT" -m conntrack --ctstate ESTABLISHED -j ACCEPT
    log "ssh na porta $SSH_PORT"
    tunel_up
}

# Túnel reverso para a VPS (acesso de qualquer rede), se configurado. Um só laço por vez.
tunel_up() {
    [ -f "$H/settings/tunel/id" ] || return 0
    [ -f /tmp/verso-ink-tunel.pid ] && kill -0 "$(cat /tmp/verso-ink-tunel.pid)" 2>/dev/null && return 0
    (setsid "$RUN/tunel.sh" </dev/null >/dev/null 2>&1 &)
}

amazon_stop() {
    # Processo parado não morre com TERM: solta antes, ou o `stop` fica esperando.
    amazon_unstick
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

# Troca rápida com o leitor da Amazon: em vez de desligar a interface (que leva ~1 min para
# voltar), ela fica congelada (SIGSTOP no Java do framework, o cvm) enquanto o Verso Ink está na
# tela. Xorg e awesome continuam vivos (como quando a interface está parada) e não atrapalham.
amazon_pids() {
    { pgrep -f "/usr/java/bin/cvm" || pgrep cvm; } 2>/dev/null
    # A barra de status da Amazon (relógio, bateria) é de outro processo, o pillowd: congela
    # junto, senão o relógio dela aparece por cima do Verso Ink a cada minuto.
    pgrep pillowd 2>/dev/null
}

# Congelada de verdade só se o Java ainda existe: com ele morto, sobrar o pillowd não basta
# (a Amazon precisa ser religada, não descongelada).
amazon_frozen() {
    [ -f /tmp/verso-ink.amazon-frozen ] && pgrep -f "/usr/java/bin/cvm" >/dev/null 2>&1
}

# Antes de religar a interface: solta o que tenha ficado parado e esquece a tela guardada.
amazon_unstick() {
    for p in $(amazon_pids); do kill -CONT "$p" 2>/dev/null; done
    rm -f /tmp/verso-ink.amazon-frozen "$AMAZON_TELA"
}

# A tela da Amazon é guardada ao congelar e devolvida ao descongelar: o X lê a própria memória
# da tela, e sem isso a Amazon só repinta o que muda (relógio, bateria), com o Verso por baixo.
AMAZON_TELA=/tmp/verso-ink.amazon-tela

amazon_freeze() {
    for p in $(amazon_pids); do kill -STOP "$p" 2>/dev/null; done
    "$RUN/verso-ink" tela-guardar "$AMAZON_TELA" 2>>"$LOG" || rm -f "$AMAZON_TELA"
    touch /tmp/verso-ink.amazon-frozen
    lipc-set-prop com.lab126.blanket unload splash 2>/dev/null
    log "interface da Amazon congelada"
}

amazon_thaw() {
    [ -f "$AMAZON_TELA" ] && "$RUN/verso-ink" tela-restaurar "$AMAZON_TELA" 2>>"$LOG" && log "tela da Amazon devolvida"
    rm -f "$AMAZON_TELA"
    for p in $(amazon_pids); do kill -CONT "$p" 2>/dev/null; done
    rm -f /tmp/verso-ink.amazon-frozen
    # A interface não sabe que a tela foi redesenhada por nós: pede a tela inicial da Amazon,
    # o que a faz desenhar tudo de novo.
    lipc-set-prop com.lab126.appmgrd start app://com.lab126.KPPMainApp 2>/dev/null
    log "interface da Amazon descongelada"
}

# Espera a pessoa sair da tela inicial da Amazon (abrir um livro) e voltar a ela (tocar em
# Início): é a volta para o Verso Ink. Sem sair da tela inicial em 5 min, volta também.
amazon_wait_home() {
    left=0; t=0
    while :; do
        sleep 2; t=$((t + 2))
        ctx="$(lipc-get-prop com.lab126.appmgrd activeContext 2>/dev/null)"
        case "$ctx" in
            *KPPMainApp*)
                if [ "$left" = 1 ] || [ "$t" -ge "${AMAZON_HOME_TIMEOUT:-300}" ]; then
                    # Um segundo para a tela inicial terminar de desenhar (ela é guardada ao
                    # congelar) e a confirmação de que a pessoa não saiu de novo nesse meio-tempo.
                    sleep 1
                    case "$(lipc-get-prop com.lab126.appmgrd activeContext 2>/dev/null)" in
                        *KPPMainApp*) return 0 ;;
                        *) left=1; continue ;;
                    esac
                fi ;;
            "") ;;
            *) left=1 ;;
        esac
    done
}

amazon_start() {
    amazon_unstick
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
