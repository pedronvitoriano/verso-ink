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
import re
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
    "Bash(git status:*)", "Bash(git diff:*)", "Bash(git log:*)", "Bash(git show:*)", "Bash(git add:*)", "Bash(git commit:*)", "Bash(git revert:*)",
    "Bash(cargo build:*)", "Bash(cargo test:*)", "Bash(cargo check:*)", "Bash(npm test:*)", "Bash(npm run:*)",
    "Bash(ls:*)", "Bash(wc:*)", "Bash(node:*)", "Bash(python3:*)",
])
BLOCKED = "Bash(git push:*),Bash(gh:*),Bash(rm -rf:*),Bash(curl:*)"

# Google pelo Claude do PC (conectores do claude.ai): só leitura no resumo; o rascunho de resposta
# pode criar rascunho, nunca enviar. Tudo o que escreve ou apaga fica bloqueado de vez.
G = "mcp__claude_ai_Gmail__"
C = "mcp__claude_ai_Google_Calendar__"
GOOGLE_READ = [G + t for t in ("search_threads", "get_thread", "get_message", "list_labels", "list_drafts", "get_draft")] + [C + t for t in ("list_calendars", "list_events", "get_event", "search_events")]
GOOGLE_BLOCKED = ",".join([G + t for t in ("send_message", "reply", "forward", "trash_message", "trash_thread", "delete_draft", "delete_label", "mark_message_spam", "mark_thread_spam", "update_draft", "label_message", "label_thread", "unlabel_message", "unlabel_thread", "update_message_labels", "create_label", "update_label", "untrash_message", "untrash_thread")] + [C + t for t in ("create_event", "delete_event", "update_event", "respond_to_event")])
DIGEST_HOUR = 4  # a partir das 4h30 de Manaus, uma vez por dia
MANAUS = -4 * 3600

DIGEST_PROMPT = """Você prepara a parte de agenda e e-mail da Edição da manhã do Pedro, que ele lê no Kindle.
Hoje é {day} (fuso de Manaus, UTC-4). Use só as ferramentas de leitura do Gmail e do Google Calendar; não crie, envie, apague nem altere nada.
1. Agenda: os compromissos de HOJE em todas as agendas dele (inclusive o dia inteiro).
2. E-mails: das últimas 36 horas, só os que pedem algo dele ou importam de verdade (pessoas, trabalho, cobranças, prazos, entrevistas, contas). Ignore promoções, newsletters, notificações automáticas e redes sociais. No máximo 6, do mais importante ao menos.
Responda SÓ com um JSON válido, sem cercas de código:
{{"agenda": [{{"hora": "09:00 ou dia todo", "titulo": "...", "onde": "local ou vazio"}}], "emails": [{{"id": "threadId do Gmail", "de": "nome curto de quem mandou", "assunto": "...", "resumo": "o que é e o que pede, até 150 caracteres", "acao": "responder|ler|pagar|decidir|nenhuma"}}]}}
Português do Brasil, direto. Listas vazias se não houver nada."""

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


def local_day(ts=None):
    return time.strftime("%Y-%m-%d", time.gmtime((ts or time.time()) + MANAUS))


def claude_json(prompt, allowed, blocked, cwd=None, timeout=600):
    """Roda o claude -p e devolve (texto, ok)."""
    cmd = [CLAUDE, "-p", prompt, "--output-format", "json", "--allowedTools", ",".join(allowed), "--disallowedTools", blocked]
    try:
        r = subprocess.run(cmd, cwd=cwd or str(HOME), capture_output=True, text=True, timeout=timeout)
        out = json.loads(r.stdout)
        return out.get("result") or "", not out.get("is_error")
    except subprocess.TimeoutExpired:
        return "passou do tempo", False
    except (json.JSONDecodeError, OSError) as e:
        return f"falhou: {e}", False


def google_digest():
    """Resumo de agenda e e-mails de hoje, mandado à ponte. Uma vez por dia, a partir das 4h30."""
    day = local_day()
    hour = time.gmtime(time.time() + MANAUS)
    if (hour.tm_hour, hour.tm_min) < (DIGEST_HOUR, 30) or conf("google_dia") == day:
        return
    text, ok = claude_json(DIGEST_PROMPT.format(day=day), GOOGLE_READ, GOOGLE_BLOCKED + "," + BLOCKED)
    m = re.search(r"\{.*\}", text, re.S)
    try:
        data = json.loads(m.group(0)) if ok and m else None
    except json.JSONDecodeError:
        data = None
    if not isinstance(data, dict):
        print(f"missoes: resumo do Google falhou: {text[:200]}", flush=True)
        if "limit" in text.lower():
            return  # tenta de novo na próxima volta
        data = {"agenda": [], "emails": [], "erro": "não consegui ler o Google hoje"}
    data["day"] = day
    api("POST", "/google", data, timeout=30)
    (CONF / "google_dia").write_text(day)


CODE_PROMPT = """Você revisa o código que entrou ontem no repositório {name}, para o Pedro ler no Kindle de manhã e decidir com um toque.
Use só leitura (git log, git show, git diff, Read, Grep). Não altere nada.
Commits das últimas 24 horas (mais novo primeiro):
{log}
Para cada commit (no máximo 8, os mais importantes): explique em linguagem comum o que mudou e por quê, avalie o risco (baixo, médio ou alto) e liste até 3 pontos concretos a corrigir ou conferir (bug, caso esquecido, teste faltando), só se forem reais.
Responda SÓ com JSON válido, sem cercas:
{{"resumo": "uma frase sobre o dia no repositório", "commits": [{{"sha": "7 primeiros caracteres", "titulo": "até 70 caracteres", "explicacao": "até 400 caracteres", "risco": "baixo|médio|alto", "pontos": ["..."]}}], "artigo": "texto em Markdown simples para ler com calma: um parágrafo de visão geral e uma seção por commit com o que mudou, o trecho mais importante explicado em palavras e os pontos de atenção"}}
Português do Brasil."""
CODE_HOUR = 3  # a partir das 3h de Manaus (antes da edição das 6h)


def code_digest(known):
    """Revisão do código de ontem, uma vez por dia: um resumo por repositório com commits."""
    day = local_day()
    if time.gmtime(time.time() + MANAUS).tm_hour < CODE_HOUR or conf("codigo_dia") == day:
        return
    repos = []
    for p in known:
        try:
            log = subprocess.run(["git", "log", "--since=24 hours ago", "--no-merges", "--format=%h %s"], cwd=p["path"], capture_output=True, text=True, timeout=20).stdout.strip()
        except (OSError, subprocess.TimeoutExpired):
            continue
        if log:
            repos.append((p, log))
    out = []
    for p, log in sorted(repos, key=lambda r: -len(r[1].splitlines()))[:3]:
        lines = "\n".join(log.splitlines()[:12])
        text, ok = claude_json(CODE_PROMPT.format(name=p["name"], log=lines), ["Read", "Grep", "Glob", "Bash(git log:*)", "Bash(git show:*)", "Bash(git diff:*)"], BLOCKED + ",Edit,Write,Bash(git commit:*),Bash(git reset:*),Bash(git checkout:*)", cwd=p["path"], timeout=900)
        m = re.search(r"\{.*\}", text, re.S)
        try:
            data = json.loads(m.group(0)) if ok and m else None
        except json.JSONDecodeError:
            data = None
        if not isinstance(data, dict):
            if "limit" in text.lower():
                return  # tenta de novo mais tarde
            continue
        data["name"] = p["name"]
        out.append(data)
    api("POST", "/codigo", {"day": day, "repos": out}, timeout=30)
    (CONF / "codigo_dia").write_text(day)


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
    if m.get("kind") == "rascunho":
        return draft(m)
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


def draft(m):
    """Rascunho de resposta a um e-mail: fica nos rascunhos do Gmail, nunca é enviado."""
    t0 = time.time()
    prompt = (
        f"{RULES}\n\nMissão: escrever um rascunho de resposta ao e-mail da conversa do Gmail com threadId {m.get('thread', '')} "
        f"({m.get('title', '')}). Leia a conversa (get_thread), escreva uma resposta curta, educada e direta no tom do Pedro, em português "
        "(ou na língua do e-mail), e crie-a como RASCUNHO na mesma conversa com create_draft. NÃO envie. "
        "Se faltar uma informação que só o Pedro tem, deixe um [colchete] no lugar. No relatório: para quem é, o assunto e o texto do rascunho."
    )
    text, ok = claude_json(prompt, GOOGLE_READ + [G + "create_draft"], GOOGLE_BLOCKED + "," + BLOCKED, timeout=600)
    mins = max(1, round((time.time() - t0) / 60))
    if not ok and "limit" in text.lower():
        return {"status": "limite", "result": text[:500], "minutes": mins}
    return {"status": "pronta" if ok else "falhou", "result": text[:20000] or "(sem relatório)", "minutes": mins}


def main():
    known = projects()
    last_projects = 0
    while True:
        try:
            if time.time() - last_projects > 600:
                known = projects()
                api("POST", "/missoes/projetos", {"projects": [p["name"] for p in known]})
                last_projects = time.time()
            google_digest()
            code_digest(known)
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
