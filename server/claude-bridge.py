#!/usr/bin/env python3
"""Ponte do Verso Ink para o Claude Code na VPS.

O Kindle conversa com o Claude por aqui: cada mensagem vira um `claude -p` (com
`--resume` para continuar a conversa). Só aceita pedidos com o token do Kindle.

    GET  /health              -> {"ok": true, "logged": bool}
    GET  /chats               -> [{"id", "title", "updated", "count"}]
    GET  /chats/<id>          -> {"id", "title", "messages": [{"role", "text"}]}
    POST /chats[/<id>]        corpo = texto da mensagem -> {"id", "reply"}

Configuração: token em /etc/verso-ink/token; porta em VERSO_INK_PORT (padrão 8790).
"""
import json
import os
import re
import secrets
import subprocess
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

TOKEN = Path(os.environ.get("VERSO_INK_TOKEN_FILE", "/etc/verso-ink/token")).read_text().strip()
DATA = Path(os.environ.get("VERSO_INK_DATA", str(Path.home() / "chats")))
DATA.mkdir(parents=True, exist_ok=True)
CLAUDE = os.environ.get("CLAUDE_BIN", str(Path.home() / ".local/bin/claude"))
SYSTEM = (
    "Você está conversando com o Pedro pelo Kindle dele (tela e-ink, só texto, teclado na tela). "
    "Responda em português do Brasil, direto e sem Markdown pesado (sem tabelas, sem títulos com #). "
    "Prefira respostas curtas; se o assunto pedir, use parágrafos curtos."
)
ID_RE = re.compile(r"^[a-f0-9]{16}$")


def load(cid):
    p = DATA / f"{cid}.json"
    return json.loads(p.read_text()) if p.exists() else None


def save(chat):
    p = DATA / f"{chat['id']}.json"
    tmp = p.with_suffix(".tmp")
    tmp.write_text(json.dumps(chat, ensure_ascii=False))
    tmp.replace(p)


def ask_claude(message, session):
    cmd = [CLAUDE, "-p", message, "--output-format", "json", "--append-system-prompt", SYSTEM]
    if session:
        cmd += ["--resume", session]
    r = subprocess.run(cmd, cwd=str(DATA), capture_output=True, text=True, timeout=240)
    try:
        out = json.loads(r.stdout)
    except json.JSONDecodeError:
        err = (r.stderr or r.stdout).strip()[-300:]
        if "login" in err.lower() or "auth" in err.lower():
            return None, None, "O Claude Code ainda não fez login na VPS."
        return None, None, f"O Claude não respondeu ({err or 'sem saída'})."
    if out.get("is_error"):
        return None, None, str(out.get("result") or "Erro do Claude.")
    return out.get("result", ""), out.get("session_id"), None


def logged_in():
    cred = Path.home() / ".claude" / ".credentials.json"
    return cred.exists()


class Handler(BaseHTTPRequestHandler):
    server_version = "verso-ink-claude"

    def log_message(self, fmt, *args):  # sem conteúdo das mensagens no log
        pass

    def reply(self, code, obj):
        body = json.dumps(obj, ensure_ascii=False).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def authorized(self):
        got = self.headers.get("Authorization", "").removeprefix("Bearer ").strip()
        if not secrets.compare_digest(got, TOKEN):
            self.reply(401, {"error": "não autorizado"})
            return False
        return True

    def path_parts(self):
        return [p for p in self.path.split("?")[0].split("/") if p]

    def do_GET(self):
        if not self.authorized():
            return
        parts = self.path_parts()
        if parts == ["health"]:
            return self.reply(200, {"ok": True, "logged": logged_in()})
        if parts == ["chats"]:
            chats = []
            for p in DATA.glob("*.json"):
                c = json.loads(p.read_text())
                chats.append({"id": c["id"], "title": c["title"], "updated": c["updated"], "count": len(c["messages"])})
            chats.sort(key=lambda c: c["updated"], reverse=True)
            return self.reply(200, chats[:30])
        if len(parts) == 2 and parts[0] == "chats" and ID_RE.match(parts[1]):
            c = load(parts[1])
            return self.reply(200, c) if c else self.reply(404, {"error": "conversa não encontrada"})
        self.reply(404, {"error": "caminho desconhecido"})

    def do_POST(self):
        if not self.authorized():
            return
        parts = self.path_parts()
        if not parts or parts[0] != "chats" or len(parts) > 2:
            return self.reply(404, {"error": "caminho desconhecido"})
        n = min(int(self.headers.get("Content-Length", "0")), 20000)
        message = self.rfile.read(n).decode("utf-8", "replace").strip()
        if not message:
            return self.reply(400, {"error": "mensagem vazia"})
        if len(parts) == 2:
            if not ID_RE.match(parts[1]) or not (chat := load(parts[1])):
                return self.reply(404, {"error": "conversa não encontrada"})
        else:
            chat = {"id": secrets.token_hex(8), "title": message[:60], "session": None, "messages": [], "updated": 0}
        text, session, err = ask_claude(message, chat.get("session"))
        if err:
            return self.reply(502, {"error": err})
        chat["session"] = session or chat.get("session")
        chat["messages"] += [{"role": "user", "text": message}, {"role": "claude", "text": text}]
        chat["updated"] = int(time.time())
        save(chat)
        self.reply(200, {"id": chat["id"], "reply": text})


if __name__ == "__main__":
    port = int(os.environ.get("VERSO_INK_PORT", "8790"))
    ThreadingHTTPServer(("127.0.0.1", port), Handler).serve_forever()
