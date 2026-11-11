#!/usr/bin/env bash
# Instala a ponte do Claude na VPS (rodar como root na VPS). Idempotente.
#   - usuário próprio "versoink", sem sudo
#   - Claude Code pelo instalador oficial (~versoink/.local/bin/claude)
#   - serviço systemd em 127.0.0.1:8790
#   - nginx: caminho /verso-ink/ dentro de um site HTTPS existente ($SITE), com cópia de segurança
# Depois: fazer o login uma vez com  su - versoink -c claude  (comando /login).
set -euo pipefail
SITE="$(readlink -f "${SITE:-/etc/nginx/sites-enabled/jarvis}")"
SRC="$(cd "$(dirname "$0")" && pwd)"

id versoink >/dev/null 2>&1 || useradd -m -s /bin/bash versoink
install -d -m 755 /opt/verso-ink
install -m 755 "$SRC/claude-bridge.py" /opt/verso-ink/claude-bridge.py
install -d -m 750 -o root -g versoink /etc/verso-ink
if [ ! -s /etc/verso-ink/token ]; then
    head -c 32 /dev/urandom | od -An -tx1 | tr -d ' \n' >/etc/verso-ink/token
fi
chown root:versoink /etc/verso-ink/token
chmod 640 /etc/verso-ink/token

if [ ! -x /home/versoink/.local/bin/claude ]; then
    su - versoink -c 'curl -fsSL https://claude.ai/install.sh | bash' >/tmp/verso-ink-claude-install.log 2>&1
fi

cat >/etc/systemd/system/verso-ink-claude.service <<'EOF'
[Unit]
Description=Verso Ink: ponte do Kindle para o Claude Code
After=network-online.target

[Service]
User=versoink
WorkingDirectory=/home/versoink
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
# Verso Ink: ponte do Kindle para o Claude (autenticação por token no próprio serviço)
location /verso-ink/ {
    proxy_pass http://127.0.0.1:8790/;
    proxy_read_timeout 300s;
    client_max_body_size 64k;
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
echo "ok"
