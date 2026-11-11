//! Conversas com o Claude pela ponte da VPS (server/claude-bridge.py), via curl do Kindle.

use crate::apps::fit;
use crate::fb::{Framebuffer, Rect};
use crate::sys::{log, read_trim};
use crate::text::{Face, Fonts};
use crate::ui::{self, INK, MUTED, PAPER};
use std::io::Write;
use std::process::{Command, Stdio};

const MARGIN: i32 = 72;
const W: i32 = 1072;

// ---------- JSON mínimo ----------

#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    pub fn get(&self, k: &str) -> Option<&Json> {
        match self {
            Json::Obj(v) => v.iter().find(|(a, _)| a == k).map(|(_, b)| b),
            _ => None,
        }
    }
    pub fn str(&self) -> Option<&str> {
        if let Json::Str(s) = self { Some(s) } else { None }
    }
    pub fn num(&self) -> Option<f64> {
        if let Json::Num(n) = self { Some(*n) } else { None }
    }
    pub fn arr(&self) -> &[Json] {
        if let Json::Arr(a) = self { a } else { &[] }
    }
}

pub fn parse(s: &str) -> Option<Json> {
    let c: Vec<char> = s.chars().collect();
    let mut i = 0;
    let v = value(&c, &mut i)?;
    Some(v)
}

fn ws(c: &[char], i: &mut usize) {
    while *i < c.len() && c[*i].is_whitespace() {
        *i += 1;
    }
}

fn value(c: &[char], i: &mut usize) -> Option<Json> {
    ws(c, i);
    match *c.get(*i)? {
        '{' => {
            *i += 1;
            let mut v = Vec::new();
            loop {
                ws(c, i);
                if c.get(*i) == Some(&'}') {
                    *i += 1;
                    return Some(Json::Obj(v));
                }
                let Json::Str(k) = value(c, i)? else { return None };
                ws(c, i);
                (c.get(*i) == Some(&':')).then_some(())?;
                *i += 1;
                v.push((k, value(c, i)?));
                ws(c, i);
                match c.get(*i)? {
                    ',' => *i += 1,
                    '}' => {}
                    _ => return None,
                }
            }
        }
        '[' => {
            *i += 1;
            let mut v = Vec::new();
            loop {
                ws(c, i);
                if c.get(*i) == Some(&']') {
                    *i += 1;
                    return Some(Json::Arr(v));
                }
                v.push(value(c, i)?);
                ws(c, i);
                match c.get(*i)? {
                    ',' => *i += 1,
                    ']' => {}
                    _ => return None,
                }
            }
        }
        '"' => {
            *i += 1;
            let mut s = String::new();
            loop {
                let ch = *c.get(*i)?;
                *i += 1;
                match ch {
                    '"' => return Some(Json::Str(s)),
                    '\\' => {
                        let e = *c.get(*i)?;
                        *i += 1;
                        match e {
                            'n' => s.push('\n'),
                            't' => s.push('\t'),
                            'r' => {}
                            'u' => {
                                let hex: String = c.get(*i..*i + 4)?.iter().collect();
                                *i += 4;
                                let mut code = u32::from_str_radix(&hex, 16).ok()?;
                                // Par substituto (emoji etc.).
                                if (0xD800..0xDC00).contains(&code) && c.get(*i) == Some(&'\\') && c.get(*i + 1) == Some(&'u') {
                                    let lo: String = c.get(*i + 2..*i + 6)?.iter().collect();
                                    let lo = u32::from_str_radix(&lo, 16).ok()?;
                                    *i += 6;
                                    code = 0x10000 + ((code - 0xD800) << 10) + (lo - 0xDC00);
                                }
                                s.push(char::from_u32(code).unwrap_or('?'));
                            }
                            other => s.push(other),
                        }
                    }
                    other => s.push(other),
                }
            }
        }
        't' => {
            *i += 4;
            Some(Json::Bool(true))
        }
        'f' => {
            *i += 5;
            Some(Json::Bool(false))
        }
        'n' => {
            *i += 4;
            Some(Json::Null)
        }
        _ => {
            let start = *i;
            while *i < c.len() && (c[*i].is_ascii_digit() || "+-.eE".contains(c[*i])) {
                *i += 1;
            }
            c[start..*i].iter().collect::<String>().parse().ok().map(Json::Num)
        }
    }
}

// ---------- Rede ----------

fn url() -> Option<String> {
    read_trim(crate::home_dir().join("settings/claude_url").to_str()?).filter(|s| !s.is_empty())
}

fn token() -> Option<String> {
    read_trim(crate::home_dir().join("settings/claude_token").to_str()?).filter(|s| !s.is_empty())
}

/// Chamada HTTP pela ponte. Devolve o JSON da resposta ou a mensagem de erro para a tela.
fn call(method: &str, path: &str, body: Option<&str>, timeout: u32) -> Result<Json, String> {
    let (Some(base), Some(tok)) = (url(), token()) else { return Err("A ponte do Claude ainda não foi configurada.".into()) };
    let mut cmd = Command::new("curl");
    cmd.args(["-s", "-m", &timeout.to_string(), "-X", method, "-H", &format!("Authorization: Bearer {tok}"), "-w", "\n%{http_code}"]);
    if let Some(b) = body {
        let ct = if b.starts_with('{') { "application/json" } else { "text/plain; charset=utf-8" };
        cmd.args(["-H", &format!("Content-Type: {ct}"), "--data-binary", "@-"]);
    }
    cmd.arg(format!("{}{path}", base.trim_end_matches('/')));
    let mut child = cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().map_err(|_| "curl indisponível".to_string())?;
    if let (Some(b), Some(mut stdin)) = (body, child.stdin.take()) {
        let _ = stdin.write_all(b.as_bytes());
    }
    let out = child.wait_with_output().map_err(|_| "falha na rede".to_string())?;
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let (json, code) = text.rsplit_once('\n').unwrap_or(("", "0"));
    let code: u32 = code.trim().parse().unwrap_or(0);
    let parsed = parse(json);
    match code {
        200 => parsed.ok_or_else(|| "resposta inválida da ponte".into()),
        0 => Err("Sem conexão com a VPS. Confira o Wi-Fi.".into()),
        401 => Err("Token do Kindle recusado pela ponte.".into()),
        c => Err(parsed.as_ref().and_then(|j| j.get("error")).and_then(Json::str).map(str::to_string).unwrap_or(format!("Erro {c} da ponte."))),
    }
}

pub struct ChatInfo {
    pub id: String,
    pub title: String,
    pub updated: i64,
    pub count: usize,
    pub files: usize,
}

/// Texto como string JSON (com aspas).
pub fn json_str(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => {}
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

pub fn list() -> Result<Vec<ChatInfo>, String> {
    let j = call("GET", "/chats", None, 20)?;
    Ok(j.arr()
        .iter()
        .filter_map(|c| {
            Some(ChatInfo {
                id: c.get("id")?.str()?.to_string(),
                title: c.get("title")?.str()?.to_string(),
                updated: c.get("updated")?.num()? as i64,
                count: c.get("count").and_then(Json::num).unwrap_or(0.0) as usize,
                files: c.get("files").and_then(Json::num).unwrap_or(0.0) as usize,
            })
        })
        .collect())
}

pub struct Msg {
    pub user: bool,
    pub text: String,
    /// Contexto enviado junto (o que você estava fazendo no Kindle).
    pub context: Option<String>,
}

pub struct Chat {
    pub id: String,
    pub title: String,
    pub messages: Vec<Msg>,
    /// Arquivos que o Claude criou nesta conversa: (nome, tamanho).
    pub files: Vec<(String, u64)>,
}

pub fn open(id: &str) -> Result<Chat, String> {
    let j = call("GET", &format!("/chats/{id}"), None, 20)?;
    Ok(Chat {
        id: id.to_string(),
        title: j.get("title").and_then(Json::str).unwrap_or("Conversa").to_string(),
        messages: j
            .get("messages")
            .map(|m| {
                m.arr()
                    .iter()
                    .filter_map(|x| {
                        Some(Msg {
                            user: x.get("role")?.str()? == "user",
                            text: x.get("text")?.str()?.to_string(),
                            context: x.get("context").and_then(Json::str).map(str::to_string),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default(),
        files: files_of(&j),
    })
}

fn files_of(j: &Json) -> Vec<(String, u64)> {
    j.get("files").map(|f| f.arr().iter().filter_map(|x| Some((x.get("name")?.str()?.to_string(), x.get("size")?.num()? as u64))).collect()).unwrap_or_default()
}

/// Pasta onde ficam os arquivos do Claude no Kindle (aparecem também na biblioteca).
pub fn files_dir() -> std::path::PathBuf {
    std::path::PathBuf::from("/mnt/us/documents/Claude")
}

/// Baixa um arquivo da conversa para o Kindle e devolve onde ficou.
pub fn download(id: &str, name: &str) -> Result<std::path::PathBuf, String> {
    let (Some(base), Some(tok)) = (url(), token()) else { return Err("A ponte do Claude ainda não foi configurada.".into()) };
    if name.contains('/') || name.contains("..") {
        return Err("nome de arquivo inválido".into());
    }
    let dir = files_dir();
    let _ = std::fs::create_dir_all(&dir);
    let dest = dir.join(name);
    let tmp = dir.join(format!(".{name}.parcial"));
    let enc: String = name.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect();
    let code = Command::new("curl")
        .args(["-s", "-m", "120", "-o", &tmp.to_string_lossy(), "-w", "%{http_code}", "-H", &format!("Authorization: Bearer {tok}")])
        .arg(format!("{}/chats/{id}/files/{enc}", base.trim_end_matches('/')))
        .output()
        .map_err(|_| "curl indisponível".to_string())?;
    let code = String::from_utf8_lossy(&code.stdout).trim().to_string();
    if code != "200" {
        let _ = std::fs::remove_file(&tmp);
        return Err(if code == "000" { "Sem conexão com a VPS.".into() } else { format!("Erro {code} ao baixar.") });
    }
    std::fs::rename(&tmp, &dest).map_err(|e| e.to_string())?;
    Ok(dest)
}

/// Envia uma mensagem (conversa nova se `id` for None), com o contexto do que você estava
/// fazendo no Kindle, se houver. Devolve o id da conversa.
pub fn send(id: Option<&str>, message: &str, context: Option<&str>) -> Result<String, String> {
    let path = id.map(|i| format!("/chats/{i}")).unwrap_or_else(|| "/chats".into());
    let t0 = std::time::Instant::now();
    let body = format!("{{\"text\": {}, \"context\": {}}}", json_str(message), json_str(context.unwrap_or("")));
    let j = call("POST", &path, Some(&body), 330)?;
    log(&format!("claude respondeu em {} s", t0.elapsed().as_secs()));
    j.get("id").and_then(Json::str).map(str::to_string).ok_or_else(|| "resposta sem conversa".into())
}

// ---------- Telas ----------

pub fn new_rect() -> Rect {
    Rect::new(W - MARGIN - 330, 76, 330, 84)
}
pub fn chat_row(i: usize) -> Rect {
    Rect::new(MARGIN - 16, 470 + i as i32 * 124, W - 2 * MARGIN + 32, 120)
}
pub fn live_rect() -> Rect {
    Rect::new(MARGIN, 1250, W - 2 * MARGIN, 110)
}
pub const ROWS: usize = 6;

fn ago(secs: i64) -> String {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    match now - secs {
        s if s < 3600 => format!("há {} min", (s / 60).max(1)),
        s if s < 86_400 => format!("há {} h", s / 3600),
        s => format!("há {} dias", s / 86_400),
    }
}

pub fn draw_hub(fb: &mut Framebuffer, f: &Fonts, chats: &Result<Vec<ChatInfo>, String>, live: usize) {
    fb.fill(fb.screen(), PAPER);
    ui::draw_back(fb, f, false);
    let r = new_rect();
    fb.round_rect(r, r.h / 2, INK);
    let w = f.measure(Face::Strong, 32.0, "+ Nova conversa");
    f.draw(fb, Face::Strong, 32.0, r.x + (r.w - w) / 2, r.y + 54, PAPER, "+ Nova conversa");
    f.draw(fb, Face::Display, 150.0, MARGIN - 6, 330, INK, "Claude");
    f.draw(fb, Face::Strong, 30.0, MARGIN, 420, MUTED, "CONVERSAS");
    match chats {
        Err(e) => {
            for (k, l) in [e.as_str(), "As conversas ficam na VPS e aparecem aqui."].iter().enumerate() {
                f.draw(fb, Face::Body, 36.0, MARGIN, 520 + k as i32 * 56, INK, l);
            }
        }
        Ok(v) if v.is_empty() => {
            f.draw(fb, Face::Body, 36.0, MARGIN, 520, INK, "Nenhuma conversa ainda. Toque em Nova conversa.");
        }
        Ok(v) => {
            for (i, c) in v.iter().take(ROWS).enumerate() {
                let r = chat_row(i);
                fb.fill(Rect::new(MARGIN, r.y, W - 2 * MARGIN, 3), INK);
                f.draw(fb, Face::Display, 44.0, MARGIN, r.y + 62, INK, &fit(f, Face::Display, 44.0, &c.title, W - 2 * MARGIN));
                let files = if c.files > 0 { format!(" · {} arquivo{}", c.files, if c.files == 1 { "" } else { "s" }) } else { String::new() };
                f.draw(fb, Face::Body, 28.0, MARGIN + 2, r.y + 102, MUTED, &format!("{} · {} mensagens{files}", ago(c.updated), c.count));
            }
        }
    }
    f.draw(fb, Face::Body, 26.0, MARGIN, 1222, MUTED, "Dica: em qualquer tela, deslize de cima para baixo para perguntar ao Claude sobre ela.");
    let lr = live_rect();
    fb.round_outline(lr, lr.h / 2, 4, INK, PAPER);
    let label = if live == 0 { "Sessões no PC ao vivo".to_string() } else { format!("Sessões no PC ao vivo · {live}") };
    let lw = f.measure(Face::Strong, 36.0, &label);
    f.draw(fb, Face::Strong, 36.0, lr.x + (lr.w - lw) / 2, lr.y + 68, INK, &label);
}

/// Uma linha já quebrada para a tela.
#[derive(Clone, Debug, PartialEq)]
pub enum Line {
    /// "VOCÊ" / "CLAUDE".
    Label(bool),
    /// Texto comum (com **negrito** marcado); `user` = mensagem sua.
    Text(String, bool),
    Heading(String),
    /// Item de lista: (texto, é a primeira linha do item).
    Bullet(String, bool),
    Code(String),
    /// Contexto que foi junto com a sua mensagem (resumido).
    Context(String),
    Blank,
}

impl Line {
    fn height(&self) -> i32 {
        match self {
            Line::Label(_) => 44,
            Line::Heading(_) => 62,
            Line::Code(_) => 42,
            Line::Context(_) => 40,
            Line::Blank => 22,
            _ => 52,
        }
    }
}

const BODY: f32 = 34.0;
const TEXT_TOP: i32 = 330;
const TEXT_BOTTOM: i32 = 1290;

fn strip_bold(s: &str) -> String {
    s.replace("**", "")
}

/// Quebra um parágrafo em linhas que cabem em `max_w` (medindo sem os marcadores de negrito).
fn wrap_words(f: &Fonts, face: Face, size: f32, text: &str, max_w: i32) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    // Se uma linha abre negrito e não fecha, a próxima continua em negrito.
    for w in text.split_whitespace() {
        let cand = if cur.is_empty() { w.to_string() } else { format!("{cur} {w}") };
        if f.measure(face, size, &strip_bold(&cand)) > max_w && !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
            cur = w.to_string();
        } else {
            cur = cand;
        }
    }
    out.push(cur);
    // Fecha e reabre negrito entre linhas para cada linha ser desenhável sozinha.
    let mut open = false;
    for l in out.iter_mut() {
        if open {
            l.insert_str(0, "**");
        }
        if l.matches("**").count() % 2 == 1 {
            l.push_str("**");
            open = true;
        } else {
            open = false;
        }
    }
    out
}

/// Markdown simples → linhas: títulos (#), listas (-, *, 1.), blocos de código (```), negrito.
pub fn layout_text(f: &Fonts, text: &str, user: bool, max_w: i32) -> Vec<Line> {
    let mut lines = Vec::new();
    let mut in_code = false;
    for raw in text.split('\n') {
        let t = raw.trim_end();
        if t.trim_start().starts_with("```") {
            in_code = !in_code;
            continue;
        }
        if in_code {
            let mut rest = t.replace('\t', "    ");
            while !rest.is_empty() {
                let mut cut = rest.chars().count();
                while cut > 1 && f.measure(Face::Body, 28.0, &rest.chars().take(cut).collect::<String>()) > max_w - 24 {
                    cut -= 1;
                }
                lines.push(Line::Code(rest.chars().take(cut).collect()));
                rest = rest.chars().skip(cut).collect();
            }
            if t.is_empty() {
                lines.push(Line::Code(String::new()));
            }
            continue;
        }
        let tt = t.trim_start();
        if tt.is_empty() {
            if lines.last() != Some(&Line::Blank) {
                lines.push(Line::Blank);
            }
            continue;
        }
        if let Some(h) = tt.strip_prefix("### ").or_else(|| tt.strip_prefix("## ")).or_else(|| tt.strip_prefix("# ")) {
            for l in wrap_words(f, Face::Display, 40.0, &strip_bold(h), max_w) {
                lines.push(Line::Heading(l));
            }
            continue;
        }
        let bullet = tt.strip_prefix("- ").or_else(|| tt.strip_prefix("* ")).or_else(|| tt.strip_prefix("• ")).map(|b| ("•".to_string(), b)).or_else(|| {
            let n: String = tt.chars().take_while(|c| c.is_ascii_digit()).collect();
            (!n.is_empty() && tt[n.len()..].starts_with(". ")).then(|| (format!("{n}."), &tt[n.len() + 2..]))
        });
        if let Some((mark, b)) = bullet {
            for (k, l) in wrap_words(f, Face::Body, BODY, b, max_w - 44).into_iter().enumerate() {
                lines.push(Line::Bullet(if k == 0 { format!("{mark}\u{1}{l}") } else { l }, k == 0));
            }
            continue;
        }
        for l in wrap_words(f, Face::Body, BODY, tt, max_w) {
            lines.push(Line::Text(l, user));
        }
    }
    while lines.last() == Some(&Line::Blank) {
        lines.pop();
    }
    lines
}

/// Divide em páginas pela altura real das linhas.
pub fn pages_of(lines: Vec<Line>) -> Vec<Vec<Line>> {
    let mut pages = vec![Vec::new()];
    let mut h = 0;
    for l in lines {
        if h + l.height() > TEXT_BOTTOM - TEXT_TOP && !pages.last().unwrap().is_empty() {
            pages.push(Vec::new());
            h = 0;
            if l == Line::Blank {
                continue;
            }
        }
        h += l.height();
        pages.last_mut().unwrap().push(l);
    }
    pages
}

/// Conversa inteira em páginas.
pub fn paginate(f: &Fonts, chat: &Chat) -> Vec<Vec<Line>> {
    let max_w = W - 2 * MARGIN;
    let mut lines = Vec::new();
    for m in &chat.messages {
        lines.push(Line::Label(m.user));
        if let Some(c) = m.context.as_deref().filter(|c| !c.is_empty()) {
            let first = c.lines().next().unwrap_or("");
            lines.push(Line::Context(fit(f, Face::Body, 26.0, &format!("Com contexto: {first}"), max_w)));
        }
        lines.extend(layout_text(f, &m.text, m.user, max_w));
        lines.push(Line::Blank);
        lines.push(Line::Blank);
    }
    pages_of(lines)
}

/// Desenha um trecho com **negrito**: alterna as faces a cada marcador.
fn draw_rich(fb: &mut Framebuffer, f: &Fonts, x: i32, y: i32, size: f32, ink: u8, text: &str) {
    let mut cx = x;
    for (k, part) in text.split("**").enumerate() {
        if part.is_empty() {
            continue;
        }
        let face = if k % 2 == 1 { Face::Strong } else { Face::Body };
        cx = f.draw(fb, face, size, cx, y, ink, part);
    }
}

pub fn draw_lines(fb: &mut Framebuffer, f: &Fonts, lines: &[Line], top: i32) {
    let mut y = top;
    for l in lines {
        let h = l.height();
        let base = y + h - 14;
        match l {
            Line::Label(user) => {
                f.draw(fb, Face::Strong, 26.0, MARGIN, base, if *user { MUTED } else { INK }, if *user { "VOCÊ" } else { "CLAUDE" });
            }
            Line::Text(t, user) => {
                if *user {
                    // Suas mensagens com um fio à esquerda.
                    fb.fill(Rect::new(MARGIN - 20, y + 6, 5, h - 6), MUTED);
                }
                draw_rich(fb, f, MARGIN, base, BODY, if *user { 0x44 } else { INK }, t);
            }
            Line::Heading(t) => {
                f.draw(fb, Face::Display, 40.0, MARGIN, base, INK, t);
            }
            Line::Bullet(t, first) => {
                if *first {
                    let (mark, rest) = t.split_once('\u{1}').unwrap_or(("•", t));
                    f.draw(fb, Face::Strong, BODY, MARGIN + 4, base, INK, mark);
                    draw_rich(fb, f, MARGIN + 44, base, BODY, INK, rest);
                } else {
                    draw_rich(fb, f, MARGIN + 44, base, BODY, INK, t);
                }
            }
            Line::Code(t) => {
                fb.fill(Rect::new(MARGIN - 8, y, W - 2 * MARGIN + 16, h), 0xEC);
                f.draw(fb, Face::Body, 28.0, MARGIN + 8, base, INK, t);
            }
            Line::Context(t) => {
                f.draw(fb, Face::Body, 26.0, MARGIN, base, MUTED, t);
            }
            Line::Blank => {}
        }
        y += h;
    }
}

pub fn reply_rect() -> Rect {
    Rect::new(MARGIN + 290, 1316, W - 2 * MARGIN - 580, 100)
}
pub fn prev_rect() -> Rect {
    Rect::new(MARGIN, 1316, 270, 100)
}
pub fn next_rect() -> Rect {
    Rect::new(W - MARGIN - 270, 1316, 270, 100)
}
/// "Arquivos · N" no alto, quando o Claude criou algum.
pub fn files_rect() -> Rect {
    Rect::new(W - MARGIN - 300, 76, 300, 84)
}

fn pill(fb: &mut Framebuffer, f: &Fonts, r: Rect, label: &str, filled: bool) {
    if filled {
        fb.round_rect(r, r.h / 2, INK);
    } else {
        fb.round_outline(r, r.h / 2, 4, INK, PAPER);
    }
    let size = if r.h < 90 { 32.0 } else { 36.0 };
    let w = f.measure(Face::Strong, size, label);
    f.draw(fb, Face::Strong, size, r.x + (r.w - w) / 2, r.y + r.h / 2 + (size * 0.36) as i32, if filled { PAPER } else { INK }, label);
}

pub fn draw_chat(fb: &mut Framebuffer, f: &Fonts, chat: &Chat, pages: &[Vec<Line>], page: usize) {
    fb.fill(fb.screen(), PAPER);
    ui::draw_back(fb, f, false);
    if !chat.files.is_empty() {
        pill(fb, f, files_rect(), &format!("Arquivos · {}", chat.files.len()), false);
    }
    f.draw(fb, Face::Display, 52.0, MARGIN, 250, INK, &fit(f, Face::Display, 52.0, &chat.title, W - 2 * MARGIN));
    if pages.len() > 1 {
        let t = format!("{} de {}", page + 1, pages.len());
        let w = f.measure(Face::Body, 26.0, &t);
        f.draw(fb, Face::Body, 26.0, W - MARGIN - w, 300, MUTED, &t);
    }
    if let Some(lines) = pages.get(page) {
        draw_lines(fb, f, lines, TEXT_TOP);
    }
    if page > 0 {
        pill(fb, f, prev_rect(), "‹ Antes", false);
    }
    pill(fb, f, reply_rect(), "Responder", true);
    if page + 1 < pages.len() {
        pill(fb, f, next_rect(), "Depois ›", false);
    }
}

// ---------- Arquivos do Claude ----------

pub fn file_row(i: usize) -> Rect {
    Rect::new(MARGIN - 16, 400 + i as i32 * 124, W - 2 * MARGIN + 32, 120)
}
pub const FILE_ROWS: usize = 7;

fn human(n: u64) -> String {
    match n {
        0..=1023 => format!("{n} B"),
        1024..=1_048_575 => format!("{} KB", n / 1024),
        _ => format!("{:.1} MB", n as f64 / 1_048_576.0),
    }
}

/// Abre no próprio Verso Ink (texto) ou no KOReader (livro/PDF)?
pub fn opens_in_reader(name: &str) -> bool {
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    ["epub", "pdf", "mobi", "azw3", "cbz", "fb2", "djvu"].contains(&ext.as_str())
}

pub fn draw_files(fb: &mut Framebuffer, f: &Fonts, chat: &Chat, note: &str) {
    fb.fill(fb.screen(), PAPER);
    ui::draw_back(fb, f, false);
    f.draw(fb, Face::Strong, 30.0, MARGIN, 240, MUTED, "ARQUIVOS DO CLAUDE");
    f.draw(fb, Face::Display, 52.0, MARGIN, 320, INK, &fit(f, Face::Display, 52.0, &chat.title, W - 2 * MARGIN));
    for (i, (name, size)) in chat.files.iter().take(FILE_ROWS).enumerate() {
        let r = file_row(i);
        fb.fill(Rect::new(MARGIN, r.y, W - 2 * MARGIN, 3), INK);
        f.draw(fb, Face::Display, 42.0, MARGIN, r.y + 60, INK, &fit(f, Face::Display, 42.0, name, W - 2 * MARGIN));
        let how = if opens_in_reader(name) { "abre no KOReader" } else { "abre aqui" };
        f.draw(fb, Face::Body, 28.0, MARGIN + 2, r.y + 100, MUTED, &format!("{} · {how} · fica em documents/Claude", human(*size)));
    }
    if !note.is_empty() {
        f.draw(fb, Face::Body, 30.0, MARGIN, 1300, INK, &fit(f, Face::Body, 30.0, note, W - 2 * MARGIN));
    }
}

/// Leitor simples de texto/Markdown (arquivos do Claude e análises).
pub fn draw_doc(fb: &mut Framebuffer, f: &Fonts, title: &str, pages: &[Vec<Line>], page: usize) {
    fb.fill(fb.screen(), PAPER);
    ui::draw_back(fb, f, false);
    f.draw(fb, Face::Display, 52.0, MARGIN, 250, INK, &fit(f, Face::Display, 52.0, title, W - 2 * MARGIN));
    if pages.len() > 1 {
        let t = format!("{} de {}", page + 1, pages.len());
        let w = f.measure(Face::Body, 26.0, &t);
        f.draw(fb, Face::Body, 26.0, W - MARGIN - w, 300, MUTED, &t);
    }
    if let Some(lines) = pages.get(page) {
        draw_lines(fb, f, lines, TEXT_TOP);
    }
    if page > 0 {
        pill(fb, f, prev_rect(), "‹ Antes", false);
    }
    if page + 1 < pages.len() {
        pill(fb, f, next_rect(), "Depois ›", false);
    }
}

/// Faixa "vai junto" do contexto (toque nela para tirar).
pub fn context_rect() -> Rect {
    Rect::new(MARGIN - 16, 360, W - 2 * MARGIN + 32, 50)
}

pub fn draw_compose_header(fb: &mut Framebuffer, f: &Fonts, title: &str, context: Option<&str>) {
    fb.fill(fb.screen(), PAPER);
    ui::draw_back(fb, f, false);
    f.draw(fb, Face::Strong, 32.0, MARGIN, 230, MUTED, "MENSAGEM PARA O CLAUDE");
    f.draw(fb, Face::Display, 56.0, MARGIN, 310, INK, &fit(f, Face::Display, 56.0, title, W - 2 * MARGIN));
    if let Some(c) = context.filter(|c| !c.is_empty()) {
        let r = context_rect();
        fb.round_rect(r, 25, 0xE4);
        let first = c.lines().next().unwrap_or("");
        let t = fit(f, Face::Body, 26.0, &format!("Vai junto: {first}  ·  toque para tirar"), r.w - 40);
        f.draw(fb, Face::Body, 26.0, r.x + 20, r.y + 34, INK, &t);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn le_json() {
        let j = parse(r#"{"id":"ab","reply":"Olá\nmundo é 😀","n":[1,2.5],"ok":true}"#).unwrap();
        assert_eq!(j.get("reply").and_then(Json::str), Some("Olá\nmundo é 😀"));
        assert_eq!(j.get("n").unwrap().arr().len(), 2);
        assert_eq!(j.get("ok"), Some(&Json::Bool(true)));
    }
}
