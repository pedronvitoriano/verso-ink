#!/usr/bin/env python3
"""Missões: o Kindle manda, os agentes do Claude Code trabalham neste PC.

Fica rodando (serviço de usuário do systemd `verso-ink-missoes`): pergunta à ponte da VPS se há
missão na fila, roda o `claude -p` na pasta certa e devolve o resultado, que vira um texto para
ler no Kindle e entra na Edição da manhã.

Limites (combinados com o Pedro): os agentes leem, editam e fazem commit LOCAL; nunca push, PR,
publicação ou mensagem. Missões marcadas "esta noite" esperam até as 2h (horário de Manaus).

Configuração: ~/.config/verso-ink/url e token (os mesmos dos outros scripts).
"""
import json
import os
import subprocess
import time
import urllib.request
from pathlib import Path

CONF = Path(os.environ.get("VERSO_INK_CONF", Path.home() / ".config/verso-ink"))
CLAUDE = os.environ.get("CLAUDE_BIN", str(Path.home() / ".local/bin/claude"))
ROOTS = [Path.home() / "Documentos/GitHub", Path.home() / "dev"]
HOME = Path.home()
POLL = 20
MAX_SECONDS = 45 * 60

# Ferramentas liberadas: ler, editar, testar e commit local. Push e publicação bloqueados.
ALLOWED = ",".join([
    "Read", "Edit", "Write", "Glob", "Grep", "WebSearch", "WebFetch", "Skill",
    "Bash(git status:*)", "Bash(git diff:*)", "Bash(git log:*)", "Bash(git add:*)", "Bash(git commit:*)",
    "Bash(cargo build:*)", "Bash(cargo test:*)", "Bash(cargo check:*)", "Bash(npm test:*)", "Bash(npm run:*)",
    "Bash(ls:*)", "Bash(wc:*)", "Bash(node:*)", "Bash(python3:*)",
])
BLOCKED = "Bash(git push:*),Bash(gh:*),Bash(rm -rf:*),Bash(curl:*)"

RULES = (
    "Você é um agente rodando sozinho, sem ninguém olhando, a pedido do Pedro pelo Kindle. "
    "Regras: pode ler, editar, testar e fazer commit LOCAL (sem Co-Authored-By); NUNCA faça push, PR, "
    "publicação, envio de mensagem ou e-mail, nem apague dados. Siga o CLAUDE.md e o Brain da pasta. "
    "No fim, responda com um relatório em português, em linguagem simples, curto (até 25 linhas): o que "
    "fez, o que encontrou, o que ficou pendente e qualquer coisa que dependa do Pedro. Sem tabelas."
)


def conf(name):
    try:
        return (CONF / name).read_text().strip()
    except OSError:
        return ""


_PINNED = False


def pin_dns():
    """DNS da operadora falhando: usa o IP fixo de ~/.config/verso-ink/resolve (host:443:IP)."""
    global _PINNED
    resolve = conf("resolve")
    if _PINNED or resolve.count(":") != 2:
        return
    import socket
    host, _, ip = resolve.split(":")
    real = socket.getaddrinfo
    socket.getaddrinfo = lambda h, *a, **k: real(ip if h == host else h, *a, **k)
    _PINNED = True


def api(method, path, body=None, timeout=20):
    url, tok = conf("url"), conf("token")
    if not url or not tok:
        raise RuntimeError("sem configuração da ponte")
    data = json.dumps(body).encode() if body is not None else None
    # DNS da operadora falhando: usa o IP fixo de ~/.config/verso-ink/resolve (host:443:IP).
    pin_dns()
    req = urllib.request.Request(url.rstrip("/") + path, data=data, method=method, headers={"Authorization": f"Bearer {tok}", "Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return json.loads(r.read() or b"null")


def projects():
    out = []
    for root in ROOTS:
        if root.is_dir():
            for p in sorted(root.iterdir()):
                if (p / ".git").exists():
                    out.append({"name": p.name, "path": str(p)})
    return out


def cwd_for(m, known):
    name = m.get("project") or ""
    for p in known:
        if p["name"] == name:
            return p["path"]
    return str(HOME)


def run(m, known):
    cwd = cwd_for(m, known)
    prompt = f"{RULES}\n\nMissão: {m['title']}\n{m['prompt']}"
    cmd = [CLAUDE, "-p", prompt, "--output-format", "json", "--permission-mode", "acceptEdits", "--allowedTools", ALLOWED, "--disallowedTools", BLOCKED]
    t0 = time.time()
    try:
        r = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, timeout=MAX_SECONDS)
        out = json.loads(r.stdout)
        text = out.get("result") or "(o agente terminou sem relatório)"
        ok = not out.get("is_error")
    except subprocess.TimeoutExpired:
        text, ok = "A missão passou de 45 minutos e foi interrompida.", False
    except (json.JSONDecodeError, OSError) as e:
        text, ok = f"A missão falhou ao rodar: {e}", False
    mins = max(1, round((time.time() - t0) / 60))
    # Limite de uso do Claude: não é falha da missão; volta para a fila e tenta mais tarde.
    if not ok and "limit" in text.lower():
        return {"status": "limite", "result": text[:500], "minutes": mins}
    return {"status": "pronta" if ok else "falhou", "result": text[:20000], "minutes": mins}


def main():
    known = projects()
    last_projects = 0
    while True:
        try:
            if time.time() - last_projects > 600:
                known = projects()
                api("POST", "/missoes/projetos", {"projects": [p["name"] for p in known]})
                last_projects = time.time()
            m = api("POST", "/missoes/proxima", {})
            if m and m.get("id"):
                res = run(m, known)
                api("POST", f"/missoes/{m['id']}", res, timeout=30)
                continue
        except Exception as e:  # rede fora, VPS reiniciando: tenta de novo depois
            print(f"missoes: {e}", flush=True)
        time.sleep(POLL)


if __name__ == "__main__":
    main()
