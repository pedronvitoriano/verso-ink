#!/bin/sh
# Túnel reverso para a VPS: deixa o SSH do Kindle (porta 2223) acessível de qualquer rede,
# pela VPS (só no próprio servidor: 127.0.0.1:22230). Reconecta sozinho quando o Wi-Fi volta,
# a rede muda ou o Kindle acorda. Liga com settings/tunel/id (chave em formato Dropbear) e
# settings/tunel/.ssh/known_hosts (chave da VPS fixada); desliga criando settings/tunel/desligado.
. "$(dirname "$0")/common.sh"
T="$RUN/tunel"
[ -f "$H/settings/tunel/id" ] || exit 0
mkdir -p "$T/.ssh"
cp "$H/settings/tunel/id" "$T/id" && chmod 600 "$T/id"
cp "$H/settings/tunel/.ssh/known_hosts" "$T/.ssh/known_hosts" 2>/dev/null
HOST="$(cat "$H/settings/tunel/host" 2>/dev/null || echo usuario@servidor)"
REMOTE_PORT="$(cat "$H/settings/tunel/porta" 2>/dev/null || echo 22230)"
export HOME="$T"
echo $$ >/tmp/verso-ink-tunel.pid
log "túnel: laço iniciado ($HOST, porta $REMOTE_PORT)"
while true; do
    if [ -f "$H/settings/tunel/desligado" ]; then
        sleep 60
        continue
    fi
    # -K 30 mantém vivo e percebe queda; sem -y: só aceita a VPS cuja chave está fixada.
    "$RUN/dbclient" -N -T -K 30 -i "$T/id" -R "127.0.0.1:$REMOTE_PORT:127.0.0.1:$SSH_PORT" "$HOST" >>/tmp/verso-ink-tunel.log 2>&1
    sleep 20
done
