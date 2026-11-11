#!/usr/bin/env python3
"""Ponte do Verso Ink para o Claude Code na VPS.

O Kindle conversa com o Claude por aqui: cada mensagem vira um `claude -p` (com
`--resume` para continuar a conversa). Só aceita pedidos com o token do Kindle.

    GET  /health                      -> {"ok": true, "logged": bool, "kindle": bool}
    GET  /chats                       -> [{"id", "title", "updated", "count", "files"}]
    GET  /chats/<id>                  -> {"id", "title", "messages": [{"role", "text"}], "files": [...]}
    POST /chats[/<id>]                corpo = texto, ou JSON {"text", "context"} -> {"id", "reply", "files"}
    GET  /chats/<id>/files/<nome>     -> o arquivo (o Kindle baixa e abre)
    POST /xadrez/lance                JSON {"fen", "moves", "legal", "color", "level"} -> {"move", "comment"}
    POST /xadrez/revisao              JSON {"moves", "evals", "color", "result", "coach"} -> {"text"}

Cada conversa tem uma pasta própria: o que o Claude cria nela aparece no Kindle. Para ler
arquivos do Kindle, o Claude usa o comando `kindle` (só leitura, pelo túnel SSH).

Configuração: token em /etc/verso-ink/token; porta em VERSO_INK_PORT (padrão 8790).
"""
import json
import os
import re
import secrets
import subprocess
import time
import urllib.parse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

TOKEN = Path(os.environ.get("VERSO_INK_TOKEN_FILE", "/etc/verso-ink/token")).read_text().strip()
DATA = Path(os.environ.get("VERSO_INK_DATA", str(Path.home() / "chats")))
DATA.mkdir(parents=True, exist_ok=True)
CLAUDE = os.environ.get("CLAUDE_BIN", str(Path.home() / ".local/bin/claude"))
KINDLE = os.environ.get("KINDLE_BIN", "/usr/local/bin/kindle")
SYSTEM = (
    "Você está conversando com o Pedro pelo Kindle dele (tela e-ink de 16 tons de cinza, teclado na tela). "
    "Responda em português do Brasil, direto. Pode usar Markdown simples: **negrito**, listas com -, "
    "títulos com # e blocos de código; evite tabelas largas. Prefira respostas curtas e parágrafos curtos.\n"
    "Quando a mensagem trouxer um bloco [Contexto], é o que o Pedro está fazendo no Kindle agora "
    "(livro, página, jogo, posição): use isso para entender a pergunta.\n"
    "Arquivos: a pasta atual é desta conversa. Tudo o que você salvar nela (txt, md, epub, pdf) aparece "
    "para o Pedro no Kindle e pode ser aberto lá; use nomes claros. Para ler arquivos do próprio Kindle, "
    "use o comando `kindle` (só leitura, só em /mnt/us): `kindle ls /mnt/us/documents`, "
    "`kindle cat <arquivo>`, `kindle find /mnt/us -name '*.epub'`, `kindle grep -i termo <arquivo>`. "
    "Livros ficam em /mnt/us/documents; destaques em /mnt/us/documents/My Clippings.txt; o plano do dia "
    "em /mnt/us/verso-ink/data/dia.txt. Para ler um EPUB, copie com `kindle cat livro.epub > livro.epub` "
    "e abra com unzip. Se o comando falhar, o Kindle está dormindo ou sem Wi-Fi."
)
TOOLS = "Read,Write,Edit,Glob,Grep,WebSearch,WebFetch,Bash(kindle:*),Bash(unzip:*),Bash(ls:*),Bash(mkdir:*),Bash(pandoc:*)"
ID_RE = re.compile(r"^[a-f0-9]{16}$")
FILE_RE = re.compile(r"^[^/\\\x00]{1,120}$")


def load(cid):
    p = DATA / f"{cid}.json"
    return json.loads(p.read_text()) if p.exists() else None


def save(chat):
    p = DATA / f"{chat['id']}.json"
    tmp = p.with_suffix(".tmp")
    tmp.write_text(json.dumps(chat, ensure_ascii=False))
    tmp.replace(p)


def workdir(cid):
    d = DATA / cid
    d.mkdir(exist_ok=True)
    return d


def files_of(cid):
    d = DATA / cid
    if not d.is_dir():
        return []
    out = []
    for p in sorted(d.iterdir(), key=lambda p: p.stat().st_mtime, reverse=True):
        if p.is_file() and not p.name.startswith(".") and FILE_RE.match(p.name):
            out.append({"name": p.name, "size": p.stat().st_size})
    return out[:40]


def run_claude(prompt, session=None, cwd=None, tools=TOOLS, system=SYSTEM, timeout=300):
    cmd = [CLAUDE, "-p", prompt, "--output-format", "json", "--append-system-prompt", system]
    if tools:
        cmd += ["--allowedTools", tools]
    if session:
        cmd += ["--resume", session]
    r = subprocess.run(cmd, cwd=str(cwd or DATA), capture_output=True, text=True, timeout=timeout)
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
    return (Path.home() / ".claude" / ".credentials.json").exists()


def kindle_online():
    try:
        r = subprocess.run([KINDLE, "ls", "/mnt/us/verso-ink"], capture_output=True, timeout=10)
        return r.returncode == 0
    except Exception:
        return False


def first_json(text):
    """Primeiro objeto JSON dentro da resposta (o modelo às vezes põe texto em volta)."""
    m = re.search(r"\{.*\}", text or "", re.S)
    if not m:
        return None
    try:
        return json.loads(m.group(0))
    except json.JSONDecodeError:
        return None


def chess_move(req):
    legal = [m for m in req.get("legal", []) if re.match(r"^[a-h][1-8][a-h][1-8][qrbn]?$", m)]
    if not legal:
        return {"error": "sem lances legais"}
    prompt = (
        "Você joga xadrez contra o Pedro no Kindle dele, de "
        + ("brancas" if req.get("color") == "w" else "pretas")
        + f". Nível pedido: {req.get('level', 'forte')}.\n"
        f"Posição (FEN): {req.get('fen', '')}\n"
        f"Lances até aqui (UCI): {' '.join(req.get('moves', [])) or '(início)'}\n"
        f"Lances legais seus agora (UCI): {' '.join(legal)}\n"
        "Escolha o seu lance pensando como um bom jogador humano. Responda SÓ com JSON: "
        '{"move": "<um dos lances legais>", "comment": "<uma frase curta, em português, falando com o Pedro>"}'
    )
    text, _, err = run_claude(prompt, tools="", system="Responda só com o JSON pedido.", timeout=180)
    if err:
        return {"error": err}
    j = first_json(text) or {}
    mv = str(j.get("move", "")).strip().lower()
    if mv not in legal:
        return {"error": "o Claude sugeriu um lance ilegal", "raw": (text or "")[:200]}
    return {"move": mv, "comment": str(j.get("comment", ""))[:240]}


def chess_review(req):
    evals = req.get("evals", [])
    lines = []
    for i, mv in enumerate(req.get("moves", [])):
        ev = evals[i] if i < len(evals) else None
        lines.append(f"{i // 2 + 1}{'.' if i % 2 == 0 else '...'} {mv}" + (f" (avaliação depois: {ev})" if ev is not None else ""))
    prompt = (
        "Revise esta partida de xadrez do Pedro, como um treinador paciente que conversa com ele. "
        f"Ele jogou de {'brancas' if req.get('color') == 'w' else 'pretas'}; resultado: {req.get('result', '?')}; "
        f"adversário: {req.get('coach', '?')}. As avaliações (em centipeões, do ponto de vista das brancas) vêm do Stockfish.\n"
        + "\n".join(lines)
        + "\n\nEscreva em português, com Markdown simples: 1) resumo em duas frases; 2) os 3 momentos que "
        "decidiram a partida, com o lance, o que ele pensou provavelmente, o que era melhor e por quê "
        "(ideia, não só o lance); 3) um padrão para treinar e um exercício concreto. Seja específico."
    )
    text, _, err = run_claude(prompt, tools="", timeout=300)
    return {"error": err} if err else {"text": text}


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
        return [urllib.parse.unquote(p) for p in self.path.split("?")[0].split("/") if p]

    def body(self, limit=60000):
        n = min(int(self.headers.get("Content-Length", "0")), limit)
        return self.rfile.read(n).decode("utf-8", "replace")

    def do_GET(self):
        if not self.authorized():
            return
        parts = self.path_parts()
        if parts == ["health"]:
            return self.reply(200, {"ok": True, "logged": logged_in(), "kindle": kindle_online()})
        if parts == ["chats"]:
            chats = []
            for p in DATA.glob("*.json"):
                c = json.loads(p.read_text())
                chats.append({"id": c["id"], "title": c["title"], "updated": c["updated"], "count": len(c["messages"]), "files": len(files_of(c["id"]))})
            chats.sort(key=lambda c: c["updated"], reverse=True)
            return self.reply(200, chats[:30])
        if len(parts) == 2 and parts[0] == "chats" and ID_RE.match(parts[1]):
            c = load(parts[1])
            if not c:
                return self.reply(404, {"error": "conversa não encontrada"})
            c = {k: v for k, v in c.items() if k != "session"}
            c["files"] = files_of(parts[1])
            return self.reply(200, c)
        if len(parts) == 4 and parts[0] == "chats" and ID_RE.match(parts[1]) and parts[2] == "files" and FILE_RE.match(parts[3]):
            p = DATA / parts[1] / parts[3]
            if not p.is_file() or p.parent != DATA / parts[1]:
                return self.reply(404, {"error": "arquivo não encontrado"})
            data = p.read_bytes()
            self.send_response(200)
            self.send_header("Content-Type", "application/octet-stream")
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)
            return
        self.reply(404, {"error": "caminho desconhecido"})

    def do_POST(self):
        if not self.authorized():
            return
        parts = self.path_parts()
        if parts in (["xadrez", "lance"], ["xadrez", "revisao"]):
            try:
                req = json.loads(self.body())
            except json.JSONDecodeError:
                return self.reply(400, {"error": "JSON inválido"})
            out = chess_move(req) if parts[1] == "lance" else chess_review(req)
            return self.reply(502 if "error" in out else 200, out)
        if not parts or parts[0] != "chats" or len(parts) > 2:
            return self.reply(404, {"error": "caminho desconhecido"})
        raw = self.body(20000).strip()
        context = ""
        if raw.startswith("{"):
            try:
                j = json.loads(raw)
                raw, context = str(j.get("text", "")).strip(), str(j.get("context", "")).strip()
            except json.JSONDecodeError:
                pass
        message = raw
        if not message:
            return self.reply(400, {"error": "mensagem vazia"})
        if len(parts) == 2:
            if not ID_RE.match(parts[1]) or not (chat := load(parts[1])):
                return self.reply(404, {"error": "conversa não encontrada"})
        else:
            chat = {"id": secrets.token_hex(8), "title": message[:60], "session": None, "messages": [], "updated": 0}
        prompt = f"[Contexto]\n{context}\n[/Contexto]\n\n{message}" if context else message
        text, session, err = run_claude(prompt, chat.get("session"), cwd=workdir(chat["id"]))
        if err:
            return self.reply(502, {"error": err})
        chat["session"] = session or chat.get("session")
        user = {"role": "user", "text": message}
        if context:
            user["context"] = context[:2000]
        chat["messages"] += [user, {"role": "claude", "text": text}]
        chat["updated"] = int(time.time())
        save(chat)
        self.reply(200, {"id": chat["id"], "reply": text, "files": files_of(chat["id"])})


if __name__ == "__main__":
    port = int(os.environ.get("VERSO_INK_PORT", "8790"))
    ThreadingHTTPServer(("127.0.0.1", port), Handler).serve_forever()
