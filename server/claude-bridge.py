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

Caixa de entrada, sessões, avisos, aprovações e Edição da manhã: ver verso_extras.py.

Cada conversa tem uma pasta própria: o que o Claude cria nela aparece no Kindle. Para ler
arquivos do Kindle, o Claude usa o comando `kindle` (só leitura, pelo túnel SSH).

Configuração: token em /etc/verso-ink/token; porta em VERSO_INK_PORT (padrão 8790).
"""
import json
import os
import re
import secrets
import subprocess
import threading
import time
import urllib.parse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

from verso_extras import ID_RE as XID_RE, SEND_PAGE, Extras

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


ATTACH = DATA / "_anexos"


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

    def raw(self, limit):
        n = int(self.headers.get("Content-Length", "0"))
        if n > limit:
            return None
        return self.rfile.read(n)

    def send_bytes(self, data, ctype):
        self.send_response(200)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        parts = self.path_parts()
        if parts == ["enviar"]:
            # A página não traz dados: a chave fica só no navegador de quem usa.
            return self.send_bytes(SEND_PAGE.encode(), "text/html; charset=utf-8")
        if not self.authorized():
            return
        if parts == ["inbox"]:
            return self.reply(200, EXTRAS.inbox())
        if len(parts) == 3 and parts[0] == "inbox" and XID_RE.match(parts[2]):
            if parts[1] == "artigo" and (t := EXTRAS.article_text(parts[2])):
                return self.send_bytes(t.encode(), "text/plain; charset=utf-8")
            if parts[1] == "cifra" and (t := EXTRAS.cifra_text(parts[2])):
                return self.send_bytes(t.encode(), "text/plain; charset=utf-8")
            if parts[1] == "foto" and (p := EXTRAS.photo(parts[2])):
                return self.send_bytes(p.read_bytes(), "application/octet-stream")
            return self.reply(404, {"error": "não encontrado"})
        if parts == ["sessoes"]:
            return self.reply(200, EXTRAS.sessions())
        if parts == ["avisos"]:
            return self.reply(200, EXTRAS.notices())
        if parts == ["longe"]:
            return self.reply(200, {"on": EXTRAS.away()})
        if parts == ["aprovacoes"]:
            return self.reply(200, EXTRAS.pending())
        if len(parts) == 2 and parts[0] == "aprovacoes" and XID_RE.match(parts[1]):
            a = EXTRAS.approval(parts[1])
            return self.reply(200, a) if a else self.reply(404, {"error": "não encontrado"})
        if parts == ["missoes"]:
            return self.reply(200, EXTRAS.missions_view())
        if parts == ["semana"]:
            wk = EXTRAS.week()
            return self.reply(200, wk) if wk else self.reply(404, {"error": "a semana ainda não foi montada"})
        if parts == ["edicao"]:
            ed = EXTRAS.edition()
            return self.reply(200, ed) if ed else self.reply(404, {"error": "ainda não há edição"})
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

    def do_DELETE(self):
        if not self.authorized():
            return
        parts = self.path_parts()
        if len(parts) == 3 and parts[0] == "inbox" and parts[1] in ("artigo", "foto", "cifra") and XID_RE.match(parts[2]):
            EXTRAS.delete(parts[1], parts[2])
            return self.reply(200, {"ok": True})
        if len(parts) == 2 and parts[0] == "avisos" and XID_RE.match(parts[1]):
            EXTRAS.del_notice(parts[1])
            return self.reply(200, {"ok": True})
        self.reply(404, {"error": "caminho desconhecido"})

    def json_body(self):
        try:
            return json.loads(self.body())
        except json.JSONDecodeError:
            return None

    def do_POST(self):
        if not self.authorized():
            return
        parts = self.path_parts()
        if parts == ["anexos"]:
            # Arquivo mandado do Kindle para uma pergunta ao Claude (livro, artigo, foto, destaques).
            data = self.raw(16_000_000)
            if data is None:
                return self.reply(413, {"error": "arquivo grande demais (máx. 16 MB)"})
            name = urllib.parse.unquote(self.headers.get("X-Nome", "anexo"))
            name = re.sub(r"[^\w .,()-]", "_", name).strip(" .")[:100] or "anexo"
            aid = secrets.token_hex(6)
            ATTACH.mkdir(exist_ok=True)
            (ATTACH / f"{aid}__{name}").write_bytes(data)
            # Anexo esquecido some em um dia.
            for old in ATTACH.iterdir():
                if time.time() - old.stat().st_mtime > 86400:
                    old.unlink(missing_ok=True)
            return self.reply(200, {"id": f"{aid}__{name}"})
        if parts == ["inbox", "foto"]:
            data = self.raw(12_000_000)
            if data is None:
                return self.reply(413, {"error": "imagem grande demais"})
            out, err = EXTRAS.add_photo(data, self.headers.get("Content-Type", ""))
            return self.reply(400, {"error": err}) if err else self.reply(200, out)
        extras_routes = {("missoes",), ("missoes", "proxima"), ("missoes", "projetos"), ("inbox", "link"), ("inbox", "cifra"), ("sessoes",), ("avisos",), ("longe",), ("aprovacoes",), ("brain",), ("edicao", "gerar"), ("google",), ("edicao", "email"), ("destaques", "perguntas"), ("codigo",), ("edicao", "codigo"), ("aprender", "curriculo"), ("aprender", "licao"), ("semana", "gerar"), ("semana", "respostas"), ("violao", "dica")}
        if tuple(parts) in extras_routes or (len(parts) == 2 and parts[0] in ("aprovacoes", "missoes")):
            req = self.json_body() if tuple(parts) != ("edicao", "gerar") else {}
            if req is None:
                return self.reply(400, {"error": "JSON inválido"})
            if parts == ["missoes"]:
                out, err = EXTRAS.add_mission(req)
                return self.reply(400, {"error": err}) if err else self.reply(200, out)
            if parts == ["missoes", "proxima"]:
                return self.reply(200, EXTRAS.next_mission())
            if parts == ["missoes", "projetos"]:
                EXTRAS.set_projects(req.get("projects", []))
                return self.reply(200, {"ok": True})
            if parts[0] == "missoes":
                if not XID_RE.match(parts[1]):
                    return self.reply(404, {"error": "não encontrado"})
                r = EXTRAS.finish_mission(parts[1], req)
                return self.reply(200, {"ok": True}) if r else self.reply(404, {"error": "não encontrado"})
            if parts == ["inbox", "cifra"]:
                out, err = EXTRAS.add_cifra(req.get("title", ""), req.get("text", ""))
                return self.reply(400, {"error": err}) if err else self.reply(200, out)
            if parts == ["inbox", "link"]:
                out, err = EXTRAS.add_link(str(req.get("url", "")).strip())
                return self.reply(400, {"error": err}) if err else self.reply(200, out)
            if parts == ["sessoes"]:
                EXTRAS.session_event(req)
                return self.reply(200, {"ok": True})
            if parts == ["avisos"]:
                n = EXTRAS.add_notice(req)
                return self.reply(200, n) if n else self.reply(400, {"error": "aviso vazio"})
            if parts == ["longe"]:
                EXTRAS.set_away(req.get("on"))
                return self.reply(200, {"on": EXTRAS.away()})
            if parts == ["aprovacoes"]:
                if not EXTRAS.away():
                    return self.reply(200, {"id": None, "away": False})
                return self.reply(200, EXTRAS.add_approval(req))
            if parts[0] == "aprovacoes":
                if not XID_RE.match(parts[1]):
                    return self.reply(404, {"error": "não encontrado"})
                a = EXTRAS.decide(parts[1], str(req.get("decision", "")))
                return self.reply(200, a) if a else self.reply(404, {"error": "não encontrado"})
            if parts == ["brain"]:
                EXTRAS.set_brain(req.get("text", ""))
                return self.reply(200, {"ok": True})
            if parts == ["destaques", "perguntas"]:
                out, err = EXTRAS.highlight_questions(req)
                return self.reply(502 if err != "nada para preparar" else 400, {"error": err}) if err else self.reply(200, out)
            if parts == ["violao", "dica"]:
                out, err = EXTRAS.guitar_tip(req)
                return self.reply(502, {"error": err}) if err else self.reply(200, out)
            if parts == ["semana", "gerar"]:
                out, err = EXTRAS.week_generate(req)
                return self.reply(502, {"error": err}) if err else self.reply(200, out)
            if parts == ["semana", "respostas"]:
                out, err = EXTRAS.week_answer(req)
                return self.reply(502 if "montada" not in str(err) else 404, {"error": err}) if err else self.reply(200, out)
            if parts == ["aprender", "curriculo"]:
                out, err = EXTRAS.learn_curriculum(req)
                return self.reply(502, {"error": err}) if err else self.reply(200, out)
            if parts == ["aprender", "licao"]:
                out, err = EXTRAS.learn_lesson(req)
                return self.reply(502, {"error": err}) if err else self.reply(200, out)
            if parts == ["codigo"]:
                out, err = EXTRAS.set_code(req)
                return self.reply(400, {"error": err}) if err else self.reply(200, out)
            if parts == ["edicao", "codigo"]:
                out, err = EXTRAS.code_action(req)
                return self.reply(400, {"error": err}) if err else self.reply(200, out)
            if parts == ["google"]:
                out, err = EXTRAS.set_google(req)
                return self.reply(400, {"error": err}) if err else self.reply(200, out)
            if parts == ["edicao", "email"]:
                out, err = EXTRAS.email_action(req)
                return self.reply(400, {"error": err}) if err else self.reply(200, out)
            if parts == ["edicao", "gerar"]:
                threading.Thread(target=EXTRAS.generate_edition, daemon=True).start()
                return self.reply(202, {"ok": True})
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
        attach = []
        if raw.startswith("{"):
            try:
                j = json.loads(raw)
                raw, context = str(j.get("text", "")).strip(), str(j.get("context", "")).strip()
                attach = [str(a) for a in j.get("anexos", []) if re.match(r"^[0-9a-f]{12}__[\w .,()-]{1,100}$", str(a))][:6]
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
        # Anexos: vão para a pasta da conversa, onde o Claude lê.
        names = []
        for a in attach:
            src = ATTACH / a
            if src.is_file():
                name = a.split("__", 1)[1]
                src.replace(workdir(chat["id"]) / name)
                names.append(name)
        if names:
            prompt = f"[Anexos do Kindle, nesta pasta: {', '.join(names)}. Leia antes de responder.]\n\n{prompt}"
        text, session, err = run_claude(prompt, chat.get("session"), cwd=workdir(chat["id"]))
        if err:
            return self.reply(502, {"error": err})
        chat["session"] = session or chat.get("session")
        user = {"role": "user", "text": message}
        if names:
            user["anexos"] = names
        if context:
            user["context"] = context[:2000]
        chat["messages"] += [user, {"role": "claude", "text": text}]
        chat["updated"] = int(time.time())
        save(chat)
        self.reply(200, {"id": chat["id"], "reply": text, "files": files_of(chat["id"])})


EXTRAS = Extras(DATA.parent / "verso-ink-estado", run_claude)


if __name__ == "__main__":
    port = int(os.environ.get("VERSO_INK_PORT", "8790"))
    ThreadingHTTPServer(("127.0.0.1", port), Handler).serve_forever()
