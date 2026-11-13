#!/usr/bin/env bash
# Instala a ponte do Verso na VPS (rodar como root na VPS). Idempotente.
#   - usuário próprio "versoink", sem sudo
#   - Claude Code e Codex CLI, cada um autenticado na conta do próprio versoink
#   - serviço systemd em 127.0.0.1:8790
#   - nginx: caminho /verso-ink/ dentro de um site HTTPS existente ($SITE), com cópia de segurança
# Depois: fazer os logins uma vez como versoink; a chave de imagens fica em arquivo separado.
set -euo pipefail
SITE="$(readlink -f "${SITE:-/etc/nginx/sites-enabled/jarvis}")"
SRC="$(cd "$(dirname "$0")" && pwd)"

id versoink >/dev/null 2>&1 || useradd -m -s /bin/bash versoink
install -d -m 755 /opt/verso-ink
install -m 755 "$SRC/claude-bridge.py" /opt/verso-ink/claude-bridge.py
install -m 644 "$SRC/verso_extras.py" /opt/verso-ink/verso_extras.py
install -d -m 750 -o root -g versoink /etc/verso-ink
if [ ! -s /etc/verso-ink/token ]; then
    head -c 32 /dev/urandom | od -An -tx1 | tr -d ' \n' >/etc/verso-ink/token
fi
chown root:versoink /etc/verso-ink/token
chmod 640 /etc/verso-ink/token

if [ ! -x /home/versoink/.local/bin/claude ]; then
    su - versoink -c 'curl -fsSL https://claude.ai/install.sh | bash' >/tmp/verso-ink-claude-install.log 2>&1
fi
if command -v npm >/dev/null 2>&1; then
    if [ ! -x /home/versoink/.local/bin/codex ]; then
        su - versoink -c 'npm install --global --prefix /home/versoink/.local @openai/codex@latest' >/tmp/verso-ink-codex-install.log 2>&1
    fi
else
    echo "aviso: Node.js/npm não encontrado; instale-os e rode este script de novo para ativar o Codex" >&2
fi

# Leitura dos arquivos do Kindle pelo túnel (só leitura: o Kindle força o script claude-ler.sh).
install -d -m 700 -o versoink -g versoink /home/versoink/.ssh
if [ ! -f /home/versoink/.ssh/kindle_leitura ]; then
    su - versoink -c 'ssh-keygen -q -t ed25519 -N "" -C kindle-leitura -f ~/.ssh/kindle_leitura'
fi
cat >/usr/local/bin/kindle <<'KINDLE'
#!/bin/sh
# Lê arquivos do Kindle pelo túnel SSH reverso (porta 22230 só no servidor).
# Manda um argumento por linha: o Kindle não interpreta nada como comando de shell.
[ $# -ge 1 ] || { echo "uso: kindle <ls|cat|head|tail|wc|stat|du|grep|find> ..." >&2; exit 2; }
exec ssh -q -T -i "$HOME/.ssh/kindle_leitura" -p 22230 -o BatchMode=yes -o ConnectTimeout=8 \
    -o StrictHostKeyChecking=accept-new -o UserKnownHostsFile="$HOME/.ssh/known_hosts_kindle" \
    root@127.0.0.1 "$(printf '%s\n' "$@")"
KINDLE
chmod 755 /usr/local/bin/kindle
cat >/etc/systemd/system/verso-ink-claude.service <<'EOF'
[Unit]
Description=Verso Ink: ponte do Kindle para o assistente Verso
After=network-online.target

[Service]
User=versoink
WorkingDirectory=/home/versoink
Environment=HOME=/home/versoink
Environment=PATH=/home/versoink/.local/bin:/usr/local/bin:/usr/bin:/bin
ExecStart=/usr/bin/python3 /opt/verso-ink/claude-bridge.py
Restart=on-failure
NoNewPrivileges=true

[Install]
WantedBy=multi-user.target
EOF
systemctl daemon-reload
systemctl enable --now verso-ink-claude.service
systemctl restart verso-ink-claude.service

cat >/etc/nginx/snippets/verso-ink.conf <<'EOF'
# Verso Ink: ponte do Kindle para o Verso (autenticação por token no próprio serviço)
location /verso-ink/ {
    proxy_pass http://127.0.0.1:8790/;
    proxy_read_timeout 600s;
    # Fotos mandadas do celular (o navegador reduz antes; 15 MB é folga).
    client_max_body_size 15m;
}
EOF
BACKUP=""
if ! grep -q "snippets/verso-ink.conf" "$SITE"; then
    BACKUP="/root/$(basename "$SITE").antes-verso-ink.$(date +%s)"
    cp "$SITE" "$BACKUP"
    # Inclui no bloco HTTPS (logo depois da primeira linha "listen 443").
    awk 'done!=1 && /listen 443/ {print; print "    include /etc/nginx/snippets/verso-ink.conf;"; done=1; next} {print}' "$SITE" >"$SITE.tmp"
    cat "$SITE.tmp" >"$SITE" && rm -f "$SITE.tmp"
fi
if ! nginx -t; then
    # Configuração inválida: volta a cópia e não recarrega.
    [ -n "$BACKUP" ] && cp "$BACKUP" "$SITE"
    rm -f /etc/nginx/snippets/verso-ink.conf
    echo "nginx -t falhou; nada foi recarregado"
    exit 1
fi
systemctl reload nginx
echo "chave de leitura do Kindle: $(cat /home/versoink/.ssh/kindle_leitura.pub)"
echo "Login do Claude: su - versoink -c /home/versoink/.local/bin/claude"
if [ -x /home/versoink/.local/bin/codex ]; then
    echo "Login do Codex:  su - versoink -c '/home/versoink/.local/bin/codex login --device-auth'"
fi
echo "Imagens: copie uma chave da OpenAI para /etc/verso-ink/openai-image-api-key (root:versoink, modo 640)."
echo "ok"
