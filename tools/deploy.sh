#!/usr/bin/env bash
# Compila e envia o Verso Ink para o Kindle pelo Wi-Fi.
# Uso: tools/deploy.sh [IP|remoto]   (padrão: $KINDLE_IP ou 192.168.0.21)
# "remoto" usa o atalho ssh kindle-remoto (túnel pela VPS, de qualquer rede).
set -euo pipefail
cd "$(dirname "$0")/.."

IP="${1:-${KINDLE_IP:-192.168.0.21}}"
KEY="${KINDLE_KEY:-$HOME/.ssh/kindle_ed25519}"
TARGET=armv7-unknown-linux-musleabi

cargo build --release --target "$TARGET"

# Porta 2223 = SSH do Verso Ink; 2222 = SSH do KOReader (reserva).
PORT=""
if [ "$IP" = remoto ]; then
    KSSH=(ssh -o BatchMode=yes kindle-remoto)
else
for p in 2223 2222; do
    if ssh -i "$KEY" -p "$p" -o IdentitiesOnly=yes -o ConnectTimeout=5 -o BatchMode=yes "root@$IP" true 2>/dev/null; then
        PORT=$p
        break
    fi
done
[ -n "$PORT" ] || { echo "Kindle não responde em $IP (portas 2223/2222)"; exit 1; }
KSSH=(ssh -i "$KEY" -p "$PORT" -o IdentitiesOnly=yes "root@$IP")
fi

STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT
cp -r kindle/verso-ink kindle/extensions "$STAGE/"
mkdir -p "$STAGE/verso-ink/upstart"
cp kindle/upstart/verso-ink.conf "$STAGE/verso-ink/upstart/"
cp "target/$TARGET/release/verso-ink" "$STAGE/verso-ink/bin/"
# Stockfish (GPL) compilado por tools/build-stockfish.sh; fica fora do Git.
[ -f vendor/stockfish ] && cp vendor/stockfish "$STAGE/verso-ink/bin/"
# Rostos dos treinadores (gerados e reduzidos por tools/rostos.sh).
[ -d assets/treinadores ] && cp -r assets/treinadores "$STAGE/verso-ink/"
[ -d assets/sprites ] && cp -r assets/sprites "$STAGE/verso-ink/"
# Dicionários (12 MB) só quando mudaram: compara o tamanho do que já está no Kindle.
if [ -d assets/dict ]; then
    remote=$("${KSSH[@]}" 'wc -c < /mnt/us/verso-ink/dict/pt.tsv 2>/dev/null' || true)
    if [ "${remote// /}" != "$(wc -c < assets/dict/pt.tsv)" ]; then cp -r assets/dict "$STAGE/verso-ink/"; fi
fi

# Envio seguro: tudo vai para uma pasta temporária no Kindle; só depois de conferir a soma do
# programa (sha256) os arquivos substituem os antigos. Uma conexão que cai no meio deixa a versão
# anterior intacta (antes, um envio cortado deixava o programa pela metade e o app caía ao abrir).
SUM=$(sha256sum "target/$TARGET/release/verso-ink" | cut -c1-64)
tar -C "$STAGE" -cf - verso-ink extensions |
    "${KSSH[@]}" "rm -rf /mnt/us/.verso-novo && mkdir -p /mnt/us/.verso-novo && tar -C /mnt/us/.verso-novo -xf - && \
        [ \"\$(sha256sum /mnt/us/.verso-novo/verso-ink/bin/verso-ink | cut -c1-64)\" = $SUM ] && \
        mv /mnt/us/.verso-novo/verso-ink/bin/verso-ink /mnt/us/verso-ink/bin/verso-ink.novo && \
        cp -r /mnt/us/.verso-novo/extensions /mnt/us/ && \
        cp -r /mnt/us/.verso-novo/verso-ink/. /mnt/us/verso-ink/ && \
        mv /mnt/us/verso-ink/bin/verso-ink.novo /mnt/us/verso-ink/bin/verso-ink && chmod +x /mnt/us/verso-ink/bin/* && \
        [ \"\$(sha256sum /mnt/us/verso-ink/bin/verso-ink | cut -c1-64)\" = $SUM ] && \
        rm -rf /mnt/us/.verso-novo && sync && echo 'enviado para /mnt/us (conferido)'" \
    || { echo "ERRO: envio incompleto ou soma diferente; a versão anterior ficou no Kindle"; exit 1; }
