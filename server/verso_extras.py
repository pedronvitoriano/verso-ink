"""Extras da ponte do Verso Ink: caixa de entrada (artigos e fotos mandados do celular),
sessões dos agentes, avisos, aprovações do Claude Code e a Edição da manhã.

Tudo fica em arquivos JSON numa pasta de estado; nada de banco. Rotas (todas com o token,
menos a página /enviar, que só pede o token no próprio navegador):

    GET  /enviar                       página para o celular (link ou foto)
    POST /inbox/link                   JSON {"url"} -> {"id", "title"}
    POST /inbox/foto                   corpo = imagem (JPEG/PNG, até 12 MB) -> {"id"}
    POST /inbox/cifra                  JSON {"title", "text"} -> {"id", "title"}
    GET  /inbox/cifra/<id>             -> texto (título na 1ª linha)
    GET  /inbox                        -> {"artigos": [...], "fotos": [...]}
    GET  /inbox/artigo/<id>            -> texto (título na 1ª linha, parágrafos separados por linha vazia)
    GET  /inbox/foto/<id>              -> a imagem
    DELETE /inbox/<artigo|foto>/<id>
    POST /sessoes                      JSON {"session", "project", "state", "detail"} (hook do PC)
    GET  /sessoes                      -> últimas sessões (estado atual de cada uma)
    POST /avisos                       JSON {"text", "level": "critico"|"info", "minutes"} -> {"id"}
    GET  /avisos                       -> avisos ativos
    DELETE /avisos/<id>
    GET  /longe | POST /longe          {"on": bool}: aprovar o Claude Code pelo Kindle
    POST /aprovacoes                   JSON {"session", "project", "tool", "summary"} (hook do PC) -> {"id"}
    GET  /aprovacoes                   -> pedidos pendentes (para o Kindle)
    GET  /aprovacoes/<id>              -> {"status": "pendente"|"permitido"|"negado"}
    POST /aprovacoes/<id>              JSON {"decision": "permitir"|"negar"} (do Kindle)
    POST /brain                        JSON {"text"}: resumo do Brain mandado pelo PC (para a Edição)
    GET  /edicao                       -> a Edição da manhã mais recente
    POST /edicao/gerar                 gera agora (em segundo plano)
"""
import html
import json
import re
import secrets
import threading
import time
import urllib.parse
import urllib.request
from pathlib import Path

ID_RE = re.compile(r"^[a-f0-9]{12}$")
APPROVAL_TTL = 600  # 10 minutos sem resposta = negado
MANAUS = -4 * 3600


def md_paras(text):
    """Markdown simples em parágrafos de leitura: sem **, crases e #; cada item de lista vira um
    parágrafo com •; linhas comuns seguidas formam um parágrafo."""
    paras, cur = [], []
    for raw in text.splitlines():
        line = re.sub(r"\*\*|__|`", "", raw).strip()
        if line.startswith("#"):
            # Título de seção: parágrafo próprio.
            if cur:
                paras.append(" ".join(cur))
                cur = []
            paras.append(re.sub(r"^#+\s*", "", line))
            continue
        bullet = re.match(r"^(?:[-*•]|\d+[.)])\s+(.*)", line)
        if not line or bullet:
            if cur:
                paras.append(" ".join(cur))
                cur = []
            if bullet:
                paras.append("• " + bullet.group(1))
            continue
        cur.append(line)
    if cur:
        paras.append(" ".join(cur))
    return paras


def new_id():
    return secrets.token_hex(6)


class Extras:
    def __init__(self, base: Path, run_claude):
        self.base = base
        self.run_claude = run_claude
        for d in ("artigos", "fotos", "cifras", "sessoes", "avisos", "aprovacoes", "edicoes", "missoes"):
            (base / d).mkdir(parents=True, exist_ok=True)
        self.lock = threading.Lock()
        threading.Thread(target=self.morning_loop, daemon=True).start()

    # ---------- utilidades ----------

    def read(self, p: Path, default=None):
        try:
            return json.loads(p.read_text())
        except (OSError, json.JSONDecodeError):
            return default

    def write(self, p: Path, obj):
        tmp = p.with_suffix(".tmp")
        tmp.write_text(json.dumps(obj, ensure_ascii=False))
        tmp.replace(p)

    # ---------- artigos ----------

    def fetch_article(self, url):
        """Baixa a página e tira o texto principal. Sem texto suficiente, pede ao Claude."""
        title, paras = url, []
        try:
            req = urllib.request.Request(url, headers={"User-Agent": "Mozilla/5.0 (leitor)"})
            raw = urllib.request.urlopen(req, timeout=20).read(4_000_000)
            page = raw.decode("utf-8", "replace")
            m = re.search(r'<meta[^>]+property="og:title"[^>]+content="([^"]+)"', page) or re.search(r"<title[^>]*>(.*?)</title>", page, re.S | re.I)
            if m:
                title = html.unescape(re.sub(r"\s+", " ", m.group(1))).strip()
            body = page
            art = re.search(r"<article[\s>].*?</article>", page, re.S | re.I)
            if art:
                body = art.group(0)
            body = re.sub(r"<(script|style|nav|footer|header|aside|figure)[\s>].*?</\1>", " ", body, flags=re.S | re.I)
            for p in re.findall(r"<(?:p|h2|h3|li|blockquote)\b[^>]*>(.*?)</(?:p|h2|h3|li|blockquote)>", body, re.S | re.I):
                t = html.unescape(re.sub(r"<[^>]+>", "", p))
                t = re.sub(r"\[\d+\]", "", t)
                t = re.sub(r"\s+", " ", t).strip()
                if len(t) >= 40:
                    paras.append(t)
        except Exception:
            pass
        if sum(len(p.split()) for p in paras) < 150:
            try:
                text, _, err = self.run_claude(
                    f"Leia a página {url} com WebFetch e devolva SÓ o texto do artigo, limpo, em parágrafos "
                    "separados por uma linha vazia. Primeira linha: o título. Sem comentários.",
                    tools="WebFetch",
                    timeout=180,
                )
            except Exception as e:
                text, err = "", str(e)
            if not err and text.strip():
                lines = [l.strip() for l in text.strip().split("\n\n") if l.strip()]
                if lines:
                    title, paras = lines[0].lstrip("# ").strip(), lines[1:]
        return title[:160], paras

    def add_link(self, url):
        if not re.match(r"^https?://", url):
            return None, "link inválido"
        title, paras = self.fetch_article(url)
        if not paras:
            return None, "não consegui ler o texto dessa página"
        aid = new_id()
        site = urllib.parse.urlparse(url).netloc.removeprefix("www.")
        self.write(self.base / "artigos" / f"{aid}.json", {"id": aid, "title": title, "url": url, "site": site, "created": int(time.time()), "paras": paras})
        return {"id": aid, "title": title}, None

    def add_photo(self, data: bytes, ctype: str):
        ext = {"image/png": "png", "image/jpeg": "jpg", "image/jpg": "jpg", "image/gif": "gif"}.get(ctype.split(";")[0].strip().lower())
        if not ext or len(data) < 100:
            return None, "mande uma imagem JPEG ou PNG"
        pid = new_id()
        (self.base / "fotos" / f"{pid}.{ext}").write_bytes(data)
        return {"id": pid}, None

    def add_cifra(self, title, text):
        """Cifra colada pelo Pedro (texto com linhas de acordes e de letra), para o violão."""
        text = str(text).replace("\r\n", "\n").strip()
        title = str(title).strip()[:120] or (text.splitlines()[0][:120] if text else "")
        if len(text) < 20:
            return None, "cole a cifra (acordes e letra)"
        cid = new_id()
        self.write(self.base / "cifras" / f"{cid}.json", {"id": cid, "title": title, "created": int(time.time()), "text": text[:60000]})
        return {"id": cid, "title": title}, None

    def cifra_text(self, cid):
        c = self.read(self.base / "cifras" / f"{cid}.json")
        return (c["title"] + "\n\n" + c["text"] + "\n") if c else None

    def inbox(self):
        arts = []
        for p in (self.base / "artigos").glob("*.json"):
            a = self.read(p, {})
            if a:
                arts.append({k: a[k] for k in ("id", "title", "site", "created")} | {"words": sum(len(x.split()) for x in a.get("paras", []))})
        arts.sort(key=lambda a: a["created"], reverse=True)
        fotos = sorted(({"id": p.stem, "created": int(p.stat().st_mtime)} for p in (self.base / "fotos").glob("*.*")), key=lambda f: f["created"], reverse=True)
        cifras = sorted(({k: c[k] for k in ("id", "title", "created")} for p in (self.base / "cifras").glob("*.json") if (c := self.read(p, {}))), key=lambda c: c["created"], reverse=True)
        return {"artigos": arts[:60], "fotos": fotos[:200], "cifras": cifras[:200]}

    def article_text(self, aid):
        a = self.read(self.base / "artigos" / f"{aid}.json")
        if not a:
            return None
        return a["title"] + "\n\n" + "\n\n".join(a["paras"]) + f"\n\nFonte: {a['site']}\n"

    def photo(self, pid):
        for p in (self.base / "fotos").glob(f"{pid}.*"):
            return p
        return None

    def delete(self, kind, iid):
        if kind == "cifra":
            (self.base / "cifras" / f"{iid}.json").unlink(missing_ok=True)
        elif kind == "artigo":
            (self.base / "artigos" / f"{iid}.json").unlink(missing_ok=True)
        elif kind == "foto" and (p := self.photo(iid)):
            p.unlink(missing_ok=True)

    # ---------- sessões e avisos ----------

    def session_event(self, ev):
        sid = re.sub(r"[^a-zA-Z0-9-]", "", str(ev.get("session", "")))[:36]
        if not sid:
            return
        now = int(time.time())
        rec = {"session": sid, "project": str(ev.get("project", ""))[:60], "state": str(ev.get("state", ""))[:30], "detail": str(ev.get("detail", ""))[:120], "time": now}
        with self.lock:
            cur = self.read(self.base / "sessoes" / "atual.json", {})
            first = cur.get(sid, {}).get("started", now)
            cur[sid] = rec | {"started": first}
            # Guarda só as dos últimos 3 dias.
            cur = {k: v for k, v in cur.items() if now - v["time"] < 3 * 86400}
            self.write(self.base / "sessoes" / "atual.json", cur)
            with open(self.base / "sessoes" / "log.jsonl", "a") as f:
                f.write(json.dumps(rec, ensure_ascii=False) + "\n")

    def sessions(self):
        cur = self.read(self.base / "sessoes" / "atual.json", {})
        return sorted(cur.values(), key=lambda r: r["time"], reverse=True)

    def session_log(self, since):
        out = []
        try:
            for line in (self.base / "sessoes" / "log.jsonl").read_text().splitlines()[-4000:]:
                r = json.loads(line)
                if r["time"] >= since:
                    out.append(r)
        except (OSError, json.JSONDecodeError):
            pass
        return out

    def add_notice(self, n):
        text = str(n.get("text", "")).strip()[:200]
        if not text:
            return None
        nid = new_id()
        mins = max(1, min(int(n.get("minutes", 720)), 7 * 24 * 60))
        rec = {"id": nid, "text": text, "level": "critico" if n.get("level") == "critico" else "info", "created": int(time.time()), "until": int(time.time()) + mins * 60}
        self.write(self.base / "avisos" / f"{nid}.json", rec)
        return rec

    def notices(self):
        now, out = int(time.time()), []
        for p in (self.base / "avisos").glob("*.json"):
            a = self.read(p)
            if not a or a["until"] < now:
                p.unlink(missing_ok=True)
                continue
            out.append(a)
        return sorted(out, key=lambda a: (a["level"] != "critico", -a["created"]))

    def del_notice(self, nid):
        (self.base / "avisos" / f"{nid}.json").unlink(missing_ok=True)

    # ---------- aprovações do Claude Code ----------

    def away(self):
        return bool(self.read(self.base / "longe.json", {}).get("on"))

    def set_away(self, on):
        self.write(self.base / "longe.json", {"on": bool(on), "since": int(time.time())})

    def add_approval(self, req):
        aid = new_id()
        rec = {
            "id": aid,
            "project": str(req.get("project", ""))[:60],
            "tool": str(req.get("tool", ""))[:40],
            "summary": str(req.get("summary", ""))[:600],
            "created": int(time.time()),
            "status": "pendente",
        }
        self.write(self.base / "aprovacoes" / f"{aid}.json", rec)
        return rec

    def approval(self, aid):
        p = self.base / "aprovacoes" / f"{aid}.json"
        a = self.read(p)
        if a and a["status"] == "pendente" and time.time() - a["created"] > APPROVAL_TTL:
            a["status"] = "negado"
            a["reason"] = "sem resposta em 10 minutos"
            self.write(p, a)
        return a

    def pending(self):
        out = []
        for p in (self.base / "aprovacoes").glob("*.json"):
            a = self.approval(p.stem)
            if a and a["status"] == "pendente":
                out.append(a)
            elif a and time.time() - a["created"] > 86400:
                p.unlink(missing_ok=True)
        return sorted(out, key=lambda a: a["created"])

    def decide(self, aid, decision):
        p = self.base / "aprovacoes" / f"{aid}.json"
        a = self.approval(aid)
        if not a or a["status"] != "pendente":
            return a
        a["status"] = "permitido" if decision == "permitir" else "negado"
        a["reason"] = "pelo Kindle"
        self.write(p, a)
        return a

    # ---------- Missões (o Kindle manda, os agentes no PC trabalham) ----------

    def missions(self):
        recs = [self.read(p) for p in (self.base / "missoes").glob("*.json")]
        recs = sorted((r for r in recs if r), key=lambda r: r["created"], reverse=True)
        # Fila antiga (mais de 3 dias sem rodar) some.
        return [r for r in recs if r["status"] != "na fila" or time.time() - r["created"] < 3 * 86400]

    def missions_view(self):
        projs = self.read(self.base / "projetos.json", {}).get("projects", [])
        tpl = [{"id": k, "title": v[0], "desc": v[1], "project": v[2]} for k, v in MISSION_TEMPLATES.items()]
        recent = [{k: r.get(k) for k in ("id", "title", "project", "status", "created", "started", "minutes", "article", "when")} for r in self.missions()[:15]]
        pc = self.read(self.base / "projetos.json", {}).get("time", 0)
        return {"templates": tpl, "projects": projs, "recent": recent, "pc_online": time.time() - pc < 1200}

    def add_mission(self, req):
        tid = str(req.get("template", ""))
        if tid not in MISSION_TEMPLATES:
            return None, "missão desconhecida"
        title, _, needs_project, prompt = MISSION_TEMPLATES[tid]
        project = str(req.get("project", ""))[:80]
        if needs_project and not project:
            return None, "escolha o projeto"
        text = str(req.get("text", "")).strip()[:1500]
        if tid == "livre":
            if len(text) < 5:
                return None, "escreva o que o agente deve fazer"
            title, prompt = f"Missão livre: {text[:60]}", text
        now = int(time.time())
        night = req.get("when") == "noite"
        not_before = now
        if night:
            # Próximas 2h de Manaus.
            local = now + MANAUS
            day_start = local - local % 86400
            target = day_start + 2 * 3600
            if target <= local:
                target += 86400
            not_before = target - MANAUS
        mid = new_id()
        rec = {"id": mid, "template": tid, "title": title + (f" · {project}" if project else ""), "prompt": prompt, "project": project, "status": "na fila", "created": now, "not_before": not_before, "when": "noite" if night else "agora"}
        self.write(self.base / "missoes" / f"{mid}.json", rec)
        return rec, None

    def next_mission(self):
        now = time.time()
        for r in sorted(self.missions(), key=lambda r: r["created"]):
            if r["status"] == "na fila" and r.get("not_before", 0) <= now:
                r["status"] = "rodando"
                r["started"] = int(now)
                self.write(self.base / "missoes" / f"{r['id']}.json", r)
                return {k: r.get(k, "") for k in ("id", "title", "prompt", "project", "kind", "thread")}
        return {}

    def finish_mission(self, mid, res):
        p = self.base / "missoes" / f"{mid}.json"
        r = self.read(p)
        if not r:
            return None
        if res.get("status") == "limite":
            # Limite de uso do Claude: de volta à fila, nova tentativa em 30 minutos.
            r["status"] = "na fila"
            r["not_before"] = int(time.time()) + 1800
            r["note"] = "esperando o limite de uso do Claude"
            self.write(p, r)
            return r
        r["status"] = "pronta" if res.get("status") == "pronta" else "falhou"
        r["minutes"] = int(res.get("minutes", 0))
        r["finished"] = int(time.time())
        text = str(res.get("result", "")).strip()
        # O relatório vira artigo para ler no Kindle.
        aid = new_id()
        paras = md_paras(text) or ["(sem relatório)"]
        self.write(self.base / "artigos" / f"{aid}.json", {"id": aid, "title": f"Missão: {r['title']}", "url": "", "site": "agentes do PC", "created": int(time.time()), "paras": paras})
        r["article"] = aid
        r["summary"] = text[:400]
        self.write(p, r)
        return r

    def set_projects(self, names):
        self.write(self.base / "projetos.json", {"projects": [str(n)[:80] for n in names][:80], "time": int(time.time())})

    # ---------- Agenda e e-mail (o Claude do PC lê o Google e manda o resumo) ----------

    def set_google(self, req):
        day = str(req.get("day", ""))[:10]
        if not re.match(r"^\d{4}-\d{2}-\d{2}$", day):
            return None, "dia inválido"
        clip = lambda v, n: str(v or "").strip()[:n]
        agenda = [{"hora": clip(a.get("hora"), 20), "titulo": clip(a.get("titulo"), 120), "onde": clip(a.get("onde"), 80)} for a in req.get("agenda", []) if isinstance(a, dict) and a.get("titulo")][:12]
        emails = [{"id": clip(e.get("id"), 40), "de": clip(e.get("de"), 60), "assunto": clip(e.get("assunto"), 120), "resumo": clip(e.get("resumo"), 200), "acao": clip(e.get("acao"), 20)} for e in req.get("emails", []) if isinstance(e, dict) and e.get("id")][:8]
        g = {"day": day, "agenda": agenda, "emails": emails, "erro": clip(req.get("erro"), 120), "time": int(time.time())}
        self.write(self.base / "google.json", g)
        # A edição de hoje já saiu? Encaixa a agenda e os e-mails nela.
        ed = self.read(self.base / "edicoes" / f"{day}.json")
        if ed:
            ed["secoes"] = self.with_google(ed.get("secoes", []), day)
            self.write(self.base / "edicoes" / f"{day}.json", ed)
        return {"ok": True}, None

    def google_sections(self, day):
        g = self.read(self.base / "google.json", {})
        gone = set(self.read(self.base / "emails_feitos.json", {}).get("ids", []))
        secs = []
        if g.get("day") == day and g.get("agenda"):
            items = []
            for a in g["agenda"]:
                onde = f" ({a['onde']})" if a.get("onde") else ""
                items.append({"texto": f"{a['hora']} · {a['titulo']}{onde}" if a.get("hora") else a["titulo"] + onde})
            secs.append({"titulo": "Agenda de hoje", "tipo": "agenda", "itens": items})
        mails = []
        # Lembretes de e-mails adiados para hoje (ou antes) vêm primeiro.
        for r in self.read(self.base / "lembretes.json", {}).get("itens", []):
            if r.get("due", "") <= day and r["id"] not in gone:
                mails.append({"texto": "Lembrete: " + r["texto"], "id": "mail:" + r["id"], "thread": r["id"]})
        if g.get("day") == day:
            for e in g.get("emails", []):
                if e["id"] in gone or any(m["thread"] == e["id"] for m in mails):
                    continue
                mails.append({"texto": f"{e['de']}: {e['assunto']}. {e['resumo']}", "id": "mail:" + e["id"], "thread": e["id"]})
        if mails:
            secs.append({"titulo": "E-mails que importam", "tipo": "email", "itens": mails})
        return secs

    def with_google(self, secs, day):
        """Troca as seções de agenda e e-mail pelas do Google, logo depois das dos agentes e avisos."""
        rest = [x for x in secs if x.get("tipo") not in ("agenda", "email")]
        at = 0
        while at < len(rest) and rest[at].get("tipo") in ("aviso", "agente"):
            at += 1
        return rest[:at] + self.google_sections(day) + rest[at:]

    def email_action(self, req):
        """Ações do cartão de e-mail na edição: rascunho (o PC escreve), amanhã (lembrete) ou ignorar."""
        tid = str(req.get("id", "")).removeprefix("mail:")[:40]
        acao = str(req.get("acao", ""))
        if not re.match(r"^[A-Za-z0-9_-]{4,40}$", tid) or acao not in ("rascunho", "lembrar", "ignorar"):
            return None, "pedido inválido"
        day = time.strftime("%Y-%m-%d", time.gmtime(time.time() + MANAUS))
        g = self.read(self.base / "google.json", {})
        mail = next((e for e in g.get("emails", []) if e["id"] == tid), None)
        rems = self.read(self.base / "lembretes.json", {"itens": []})
        rem = next((r for r in rems["itens"] if r["id"] == tid), None)
        texto = f"{mail['de']}: {mail['assunto']}" if mail else (rem["texto"] if rem else "e-mail")
        out = {"ok": True}
        if acao == "rascunho":
            now = int(time.time())
            mid = new_id()
            rec = {"id": mid, "template": "rascunho", "kind": "rascunho", "thread": tid, "title": f"Rascunho para {texto[:60]}", "prompt": "", "project": "", "status": "na fila", "created": now, "not_before": now, "when": "agora"}
            self.write(self.base / "missoes" / f"{mid}.json", rec)
            out["mission"] = mid
        elif acao == "lembrar":
            tomorrow = time.strftime("%Y-%m-%d", time.gmtime(time.time() + MANAUS + 86400))
            rems["itens"] = [r for r in rems["itens"] if r["id"] != tid] + [{"id": tid, "texto": texto, "due": tomorrow}]
            self.write(self.base / "lembretes.json", rems)
        if acao != "lembrar":
            rems["itens"] = [r for r in rems["itens"] if r["id"] != tid]
            self.write(self.base / "lembretes.json", rems)
        # Sai da edição de hoje (o lembrete volta amanhã pela lista de lembretes).
        feitos = self.read(self.base / "emails_feitos.json", {"ids": []})
        if acao != "lembrar":
            feitos["ids"] = (feitos["ids"] + [tid])[-300:]
            self.write(self.base / "emails_feitos.json", feitos)
        ed = self.read(self.base / "edicoes" / f"{day}.json")
        if ed:
            for sec in ed.get("secoes", []):
                sec["itens"] = [i for i in sec.get("itens", []) if i.get("id") != "mail:" + tid]
            ed["secoes"] = [x for x in ed["secoes"] if x.get("itens")]
            self.write(self.base / "edicoes" / f"{day}.json", ed)
        return out, None

    # ---------- Edição da manhã ----------

    def set_brain(self, text):
        self.write(self.base / "brain.json", {"text": str(text)[:20000], "time": int(time.time())})

    def edition(self):
        eds = sorted((self.base / "edicoes").glob("*.json"))
        return self.read(eds[-1]) if eds else None

    def generate_edition(self):
        now = int(time.time())
        local = time.gmtime(now + MANAUS)
        day = time.strftime("%Y-%m-%d", local)
        since = now - 14 * 3600
        log = self.session_log(since)
        # Por sessão: começo, último estado e as últimas ações (sem conteúdo de arquivos).
        by = {}
        for r in log:
            s = by.setdefault(r["session"], {"project": r["project"], "first": r["time"], "events": []})
            s["last"] = r
            if r["detail"]:
                s["events"].append(r["detail"])
        agents = []
        for sid, s in by.items():
            last = s["last"]
            agents.append(
                f"- projeto {s['project']}: de {time.strftime('%H:%M', time.gmtime(s['first'] + MANAUS))} a "
                f"{time.strftime('%H:%M', time.gmtime(last['time'] + MANAUS))}, estado final \"{last['state']}\" "
                f"({last['detail']}); últimas ações: {'; '.join(s['events'][-6:])}"
            )
        for r in self.missions():
            if r.get("finished", 0) >= since:
                agents.append(f"- missão \"{r['title']}\" ({r['status']}, {r.get('minutes', 0)} min): {r.get('summary', '')[:300]}")
        notices = [f"- [{a['level']}] {a['text']}" for a in self.notices()]
        brain = self.read(self.base / "brain.json", {}).get("text", "")
        prompt = f"""Monte a Edição da manhã do Pedro para hoje ({day}), que ele lê no Kindle (e-ink) ao acordar e onde cada item vira um cartão com uma ação.
Responda SÓ com um JSON válido, sem cercas de código, neste formato:
{{"manchete": "uma frase que resume o dia", "secoes": [{{"titulo": "...", "tipo": "agente|agenda|email|tarefa|noticia", "itens": [{{"texto": "...", "url": "https://... (só notícias)", "projeto": "nome (só agentes)"}}]}}]}}
Seções, nesta ordem, só as que tiverem conteúdo (NÃO inclua avisos, agenda nem e-mails; o sistema põe sozinho):
1. "Os agentes durante a noite" (tipo agente) — um item por sessão: o que fez, se terminou, onde parou e se precisa dele. Linguagem comum.
2. "Brain e tarefas" (tipo tarefa) — no máximo 5 pendências do resumo do Brain abaixo.
3. "Na sua área" (tipo noticia) — 3 a 5 notícias de hoje sobre IA, desenvolvimento de software e tecnologia no Brasil, pesquisadas na web; texto com uma linha de resumo e a fonte entre parênteses; "url" com o link da matéria.
Cada "texto" tem no máximo 200 caracteres. Português do Brasil, direto, sem emoji.

[Avisos ativos, para a manchete levar em conta]
{chr(10).join(notices) or "nenhum"}

[Agenda e e-mails de hoje, só para a manchete levar em conta]
{json.dumps(self.read(self.base / "google.json", {}), ensure_ascii=False)[:3000] if self.read(self.base / "google.json", {}).get("day") == day else "ainda não chegaram"}

[Sessões dos agentes nas últimas 14 horas]
{chr(10).join(agents) or "nenhuma sessão registrada"}

[Resumo do Brain]
{brain[:6000] or "sem resumo enviado"}
"""
        try:
            text, _, err = self.run_claude(prompt, tools="WebSearch,WebFetch", timeout=600)
        except Exception as e:
            text, err = "", str(e)
        ed = None
        if not err:
            m = re.search(r"\{.*\}", text, re.S)
            try:
                ed = json.loads(m.group(0)) if m else None
            except json.JSONDecodeError:
                ed = None
        if not ed:
            ed = {"manchete": "A edição de hoje não ficou pronta. Toque em Gerar de novo.", "secoes": []}
        # Itens como objetos {texto, ...}; avisos reais (com id, para dispensar) sempre primeiro.
        secs = []
        for sec in ed.get("secoes", []):
            items = [i if isinstance(i, dict) else {"texto": str(i)} for i in sec.get("itens", [])]
            items = [i for i in items if str(i.get("texto", "")).strip()]
            if items and sec.get("tipo") != "aviso":
                secs.append({"titulo": sec.get("titulo", ""), "tipo": sec.get("tipo", "tarefa"), "itens": items})
        avisos = [{"texto": a["text"], "id": a["id"], "critico": a["level"] == "critico"} for a in self.notices()]
        if avisos:
            secs.insert(0, {"titulo": "Avisos", "tipo": "aviso", "itens": avisos})
        ed["secoes"] = self.with_google(secs, day)
        ed["dia"] = day
        ed["gerada"] = now
        self.write(self.base / "edicoes" / f"{day}.json", ed)
        for old in sorted((self.base / "edicoes").glob("*.json"))[:-14]:
            old.unlink(missing_ok=True)
        return ed

    def morning_loop(self):
        """Às 6h de Manaus, gera a edição do dia (uma vez)."""
        while True:
            local = time.gmtime(time.time() + MANAUS)
            day = time.strftime("%Y-%m-%d", local)
            if local.tm_hour >= 6 and not (self.base / "edicoes" / f"{day}.json").exists():
                try:
                    self.generate_edition()
                except Exception:
                    pass
            time.sleep(120)


# Modelos de missão: id -> (título, descrição, precisa de projeto, instrução para o agente).
MISSION_TEMPLATES = {
    "raiox": ("Raio-x do projeto", "Testes, bugs e pontos fracos, com correções pequenas", True,
              "Faça um raio-x deste projeto: rode os testes e o build, procure bugs e pontos fracos reais, corrija o que for pequeno e seguro (com commit local) e liste o resto em ordem de importância."),
    "pendente": ("Revisar pendências", "Mudanças sem commit e o último trabalho", True,
                 "Revise as mudanças sem commit e os últimos commits deste repositório: aponte bugs, riscos e o que está pela metade. Não faça commit do trabalho dos outros; só relate."),
    "post": ("Post de amanhã", "LinkedIn, no seu tom, sem publicar", False,
             "Use a skill de posts do Pedro para preparar o rascunho do post de amanhã no LinkedIn (sem publicar nada), seguindo o ciclo de melhoria dela. Mostre o rascunho completo no relatório."),
    "vagas": ("Vagas da semana", "As que combinam com o seu currículo", False,
              "Use a skill de vagas do Pedro para listar as melhores vagas abertas desta semana, em ordem de chance, sem se candidatar. Liste no relatório com o link de cada uma."),
    "brain": ("Arrumar o Brain", "Retomadas, pendências e adequações", False,
              "No Brain do Pedro (/home/pedro/dev/aions-brain), revise as retomadas em andamento e as adequações pendentes: diga o que está parado, o que dá para fechar e o que precisa dele. Não abra PR nem altere a parte compartilhada."),
    "livre": ("Missão livre", "Você escreve o que o agente faz", False, ""),
}


SEND_PAGE = """<!doctype html><html lang="pt-BR"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1"><title>Mandar para o Kindle</title>
<style>
:root{--ink:#111;--muted:#666;--line:#ddd;--bg:#fafaf7}
*{box-sizing:border-box}body{margin:0;font:17px/1.45 -apple-system,system-ui,sans-serif;background:var(--bg);color:var(--ink)}
main{max-width:520px;margin:0 auto;padding:28px 18px 60px}
h1{font-size:30px;margin:0 0 4px}p.sub{color:var(--muted);margin:0 0 26px}
.card{background:#fff;border:2px solid var(--ink);border-radius:18px;padding:18px;margin:0 0 22px;box-shadow:0 6px 0 #2e2e2e}
label{font-weight:700;display:block;margin-bottom:8px}
input[type=url],input[type=password]{width:100%;font:inherit;padding:12px 14px;border:2px solid var(--ink);border-radius:12px}
button,.file{display:block;width:100%;margin-top:12px;font:inherit;font-weight:700;padding:13px;border-radius:999px;border:2px solid var(--ink);background:var(--ink);color:#fff;text-align:center}
.file{background:#fff;color:var(--ink)}input[type=file]{display:none}
#msg{min-height:24px;font-weight:600}.muted{color:var(--muted);font-size:14px}
</style></head><body><main>
<h1>Mandar para o Kindle</h1><p class="sub">Links viram artigos para ler no Verso Ink, cifras vão para o violão e fotos para o porta-retrato.</p>
<div class="card" id="tok" hidden><label for="t">Chave do Verso Ink</label>
<input id="t" type="password" autocomplete="off" placeholder="cole a chave uma vez">
<button onclick="saveTok()">Guardar neste aparelho</button><p class="muted">Fica só neste navegador.</p></div>
<div class="card"><label for="u">Artigo</label><input id="u" type="url" placeholder="https://…">
<button onclick="sendLink()">Mandar artigo</button></div>
<div class="card"><label for="ct">Cifra para o violão</label>
<input id="ct" type="url" style="margin-bottom:10px" placeholder="Nome da música (opcional)">
<textarea id="cx" rows="7" placeholder="Cole aqui a cifra: linhas de acordes e de letra" style="width:100%;font:15px/1.4 ui-monospace,monospace;padding:12px;border:2px solid var(--ink);border-radius:12px"></textarea>
<button onclick="sendCifra()">Mandar cifra</button></div>
<div class="card"><label>Foto</label><label class="file" for="f">Escolher foto</label>
<input id="f" type="file" accept="image/*" multiple onchange="sendPhotos(this.files)"></div>
<p id="msg"></p></main>
<script>
const base=location.pathname.replace(/enviar\\/?$/,'');
let tok='';try{tok=localStorage.getItem('vi-tok')||''}catch(e){}
if(!tok)document.getElementById('tok').hidden=false;
function say(t){document.getElementById('msg').textContent=t}
function saveTok(){tok=document.getElementById('t').value.trim();try{localStorage.setItem('vi-tok',tok)}catch(e){}document.getElementById('tok').hidden=true;say('Chave guardada.')}
async function call(path,opt){opt.headers=Object.assign(opt.headers||{},{Authorization:'Bearer '+tok});const r=await fetch(base+path,opt);const j=await r.json().catch(()=>({}));if(r.status==401){document.getElementById('tok').hidden=false;throw new Error('chave inválida')}if(!r.ok)throw new Error(j.error||('erro '+r.status));return j}
async function sendLink(){const u=document.getElementById('u').value.trim();if(!u)return;say('Lendo a página…');try{const j=await call('inbox/link',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({url:u})});say('Mandado: '+j.title);document.getElementById('u').value=''}catch(e){say('Não deu: '+e.message)}}
async function sendCifra(){const t=document.getElementById('cx').value;if(!t.trim())return;say('Mandando a cifra…');try{const j=await call('inbox/cifra',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({title:document.getElementById('ct').value,text:t})});say('Cifra no Kindle: '+j.title);document.getElementById('cx').value='';document.getElementById('ct').value=''}catch(e){say('Não deu: '+e.message)}}
function shrink(file){return new Promise((ok,bad)=>{const img=new Image();img.onload=()=>{const s=Math.min(1,1448/Math.max(img.width,img.height));const c=document.createElement('canvas');c.width=Math.round(img.width*s);c.height=Math.round(img.height*s);c.getContext('2d').drawImage(img,0,0,c.width,c.height);c.toBlob(b=>b?ok(b):bad(new Error('imagem')),'image/jpeg',0.86)};img.onerror=()=>bad(new Error('imagem ilegível'));img.src=URL.createObjectURL(file)})}
async function sendPhotos(files){let n=0;for(const f of files){say('Mandando foto '+(n+1)+' de '+files.length+'…');try{const b=await shrink(f);await call('inbox/foto',{method:'POST',headers:{'Content-Type':'image/jpeg'},body:b});n++}catch(e){say('Não deu: '+e.message);return}}say(n==1?'Foto mandada.':n+' fotos mandadas.')}
</script></body></html>"""
