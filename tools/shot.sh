#!/usr/bin/env bash
# Captura a tela do Kindle pelo Wi-Fi e salva em PNG.
# Uso: tools/shot.sh saida.png [IP]
# Observação: captura a memória da tela (o que foi desenhado), não o painel físico.
set -euo pipefail
OUT="${1:?uso: tools/shot.sh saida.png [IP]}"
IP="${2:-${KINDLE_IP:-192.168.0.21}}"
KEY="${KINDLE_KEY:-$HOME/.ssh/kindle_ed25519}"
PORT="${KINDLE_PORT:-2223}"
RAW="$(mktemp)"
trap 'rm -f "$RAW"' EXIT
ssh -i "$KEY" -p "$PORT" -o IdentitiesOnly=yes "root@$IP" 'dd if=/dev/fb0 bs=1088 count=1448 2>/dev/null' >"$RAW"
python3 -I - "$RAW" "$OUT" <<'PY'
import sys
from PIL import Image
raw, out = sys.argv[1], sys.argv[2]
data = open(raw, "rb").read()
Image.frombytes("L", (1088, 1448), data).crop((0, 0, 1072, 1448)).save(out)
PY
echo "$OUT"
