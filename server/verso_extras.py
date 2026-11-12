"""Extras da ponte do Verso Ink: caixa de entrada (artigos e fotos mandados do celular),
sessões dos agentes, avisos, aprovações do Claude Code e a Edição da manhã.

Tudo fica em arquivos JSON numa pasta de estado; nada de banco. Rotas (todas com o token,
menos a página /enviar, que só pede o token no próprio navegador):

    GET  /enviar                       página para o celular (link ou foto)
    POST /inbox/link                   JSON {"url"} -> {"id", "title"}
    POST /inbox/foto                   corpo = imagem (JPEG/PNG, até 12 MB) -> {"id"}
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


def new_id():
    return secrets.token_hex(6)


class Extras:
    def __init__(self, base: Path, run_claude):
        self.base = base
        self.run_claude = run_claude
        for d in ("artigos", "fotos", "sessoes", "avisos", "aprovacoes", "edicoes"):
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

    def inbox(self):
        arts = []
        for p in (self.base / "artigos").glob("*.json"):
            a = self.read(p, {})
            if a:
                arts.append({k: a[k] for k in ("id", "title", "site", "created")} | {"words": sum(len(x.split()) for x in a.get("paras", []))})
        arts.sort(key=lambda a: a["created"], reverse=True)
        fotos = sorted(({"id": p.stem, "created": int(p.stat().st_mtime)} for p in (self.base / "fotos").glob("*.*")), key=lambda f: f["created"], reverse=True)
        return {"artigos": arts[:60], "fotos": fotos[:200]}

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
        if kind == "artigo":
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
        notices = [f"- [{a['level']}] {a['text']}" for a in self.notices()]
        brain = self.read(self.base / "brain.json", {}).get("text", "")
        prompt = f"""Monte a Edição da manhã do Pedro para hoje ({day}), que ele lê no Kindle (e-ink) ao acordar.
Responda SÓ com um JSON válido, sem cercas de código, neste formato:
{{"manchete": "uma frase que resume o dia", "secoes": [{{"titulo": "...", "itens": ["...", "..."]}}]}}
Seções, nesta ordem, só as que tiverem conteúdo:
1. "Avisos" — os avisos abaixo, os críticos primeiro, em CAIXA ALTA quando forem críticos.
2. "Os agentes durante a noite" — o que cada sessão do Claude Code fez, se terminou, onde parou e se precisa dele. Frases curtas, linguagem comum.
3. "Agenda de hoje" e "E-mails que importam" — use as ferramentas do Google se estiverem disponíveis; se não estiverem, omita estas seções.
4. "Brain e tarefas" — as pendências do resumo do Brain abaixo, no máximo 5.
5. "Na sua área" — 3 a 5 notícias de hoje sobre IA, desenvolvimento de software e tecnologia no Brasil, pesquisadas na web, cada uma com uma linha de resumo e a fonte entre parênteses.
Cada item tem no máximo 200 caracteres. Português do Brasil, direto, sem emoji.

[Avisos]
{chr(10).join(notices) or "nenhum"}

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
            ed = {"manchete": "A edição de hoje não ficou pronta.", "secoes": [{"titulo": "Avisos", "itens": [a["text"] for a in self.notices()] or ["Nenhum aviso."]}]}
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
<h1>Mandar para o Kindle</h1><p class="sub">Links viram artigos para ler no Verso Ink. Fotos vão para o porta-retrato.</p>
<div class="card" id="tok" hidden><label for="t">Chave do Verso Ink</label>
<input id="t" type="password" autocomplete="off" placeholder="cole a chave uma vez">
<button onclick="saveTok()">Guardar neste aparelho</button><p class="muted">Fica só neste navegador.</p></div>
<div class="card"><label for="u">Artigo</label><input id="u" type="url" placeholder="https://…">
<button onclick="sendLink()">Mandar artigo</button></div>
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
function shrink(file){return new Promise((ok,bad)=>{const img=new Image();img.onload=()=>{const s=Math.min(1,1448/Math.max(img.width,img.height));const c=document.createElement('canvas');c.width=Math.round(img.width*s);c.height=Math.round(img.height*s);c.getContext('2d').drawImage(img,0,0,c.width,c.height);c.toBlob(b=>b?ok(b):bad(new Error('imagem')),'image/jpeg',0.86)};img.onerror=()=>bad(new Error('imagem ilegível'));img.src=URL.createObjectURL(file)})}
async function sendPhotos(files){let n=0;for(const f of files){say('Mandando foto '+(n+1)+' de '+files.length+'…');try{const b=await shrink(f);await call('inbox/foto',{method:'POST',headers:{'Content-Type':'image/jpeg'},body:b});n++}catch(e){say('Não deu: '+e.message);return}}say(n==1?'Foto mandada.':n+' fotos mandadas.')}
</script></body></html>"""
