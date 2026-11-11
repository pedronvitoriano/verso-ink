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
    if body.is_some() {
        cmd.args(["-H", "Content-Type: text/plain; charset=utf-8", "--data-binary", "@-"]);
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
            })
        })
        .collect())
}

pub struct Chat {
    pub id: String,
    pub title: String,
    pub messages: Vec<(bool, String)>,
}

pub fn open(id: &str) -> Result<Chat, String> {
    let j = call("GET", &format!("/chats/{id}"), None, 20)?;
    Ok(Chat {
        id: id.to_string(),
        title: j.get("title").and_then(Json::str).unwrap_or("Conversa").to_string(),
        messages: j
            .get("messages")
            .map(|m| m.arr().iter().filter_map(|x| Some((x.get("role")?.str()? == "user", x.get("text")?.str()?.to_string()))).collect())
            .unwrap_or_default(),
    })
}

/// Envia uma mensagem (conversa nova se `id` for None). Devolve o id da conversa.
pub fn send(id: Option<&str>, message: &str) -> Result<String, String> {
    let path = id.map(|i| format!("/chats/{i}")).unwrap_or_else(|| "/chats".into());
    let t0 = std::time::Instant::now();
    let j = call("POST", &path, Some(message), 260)?;
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
                f.draw(fb, Face::Body, 28.0, MARGIN + 2, r.y + 102, MUTED, &format!("{} · {} mensagens", ago(c.updated), c.count));
            }
        }
    }
    let lr = live_rect();
    fb.round_outline(lr, lr.h / 2, 4, INK, PAPER);
    let label = if live == 0 { "Sessões no PC ao vivo".to_string() } else { format!("Sessões no PC ao vivo · {live}") };
    let lw = f.measure(Face::Strong, 36.0, &label);
    f.draw(fb, Face::Strong, 36.0, lr.x + (lr.w - lw) / 2, lr.y + 68, INK, &label);
}

/// Quebra a conversa em linhas e páginas. Cada linha: (texto, é rótulo, é do usuário).
pub fn paginate(f: &Fonts, chat: &Chat) -> Vec<Vec<(String, bool, bool)>> {
    let max_w = W - 2 * MARGIN;
    let per_page = 17;
    let mut lines: Vec<(String, bool, bool)> = Vec::new();
    for (user, text) in &chat.messages {
        lines.push((if *user { "VOCÊ".into() } else { "CLAUDE".into() }, true, *user));
        for para in text.split('\n') {
            let mut cur = String::new();
            for w in para.split_whitespace() {
                let cand = if cur.is_empty() { w.to_string() } else { format!("{cur} {w}") };
                if f.measure(Face::Body, 34.0, &cand) > max_w && !cur.is_empty() {
                    lines.push((std::mem::take(&mut cur), false, *user));
                    cur = w.to_string();
                } else {
                    cur = cand;
                }
            }
            lines.push((cur, false, *user));
        }
        lines.push((String::new(), false, *user));
    }
    let mut pages: Vec<Vec<_>> = lines.chunks(per_page).map(<[_]>::to_vec).collect();
    if pages.is_empty() {
        pages.push(Vec::new());
    }
    pages
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

pub fn draw_chat(fb: &mut Framebuffer, f: &Fonts, chat: &Chat, pages: &[Vec<(String, bool, bool)>], page: usize) {
    fb.fill(fb.screen(), PAPER);
    ui::draw_back(fb, f, false);
    f.draw(fb, Face::Display, 52.0, MARGIN, 250, INK, &fit(f, Face::Display, 52.0, &chat.title, W - 2 * MARGIN));
    let mut y = 340;
    if let Some(lines) = pages.get(page) {
        for (text, label, user) in lines {
            if *label {
                f.draw(fb, Face::Strong, 26.0, MARGIN, y, if *user { MUTED } else { INK }, text);
            } else if !text.is_empty() {
                // Suas mensagens com um fio à esquerda, as do Claude sem.
                if *user {
                    fb.fill(Rect::new(MARGIN - 20, y - 34, 5, 44), MUTED);
                }
                f.draw(fb, Face::Body, 34.0, MARGIN, y, if *user { 0x44 } else { INK }, text);
            }
            y += 54;
        }
    }
    for (r, label, filled, show) in [
        (prev_rect(), "‹ Antes", false, page > 0),
        (reply_rect(), "Responder", true, true),
        (next_rect(), "Depois ›", false, page + 1 < pages.len()),
    ] {
        if !show {
            continue;
        }
        if filled {
            fb.round_rect(r, r.h / 2, INK);
        } else {
            fb.round_outline(r, r.h / 2, 4, INK, PAPER);
        }
        let w = f.measure(Face::Strong, 36.0, label);
        f.draw(fb, Face::Strong, 36.0, r.x + (r.w - w) / 2, r.y + 64, if filled { PAPER } else { INK }, label);
    }
}

pub fn draw_compose_header(fb: &mut Framebuffer, f: &Fonts, title: &str) {
    fb.fill(fb.screen(), PAPER);
    ui::draw_back(fb, f, false);
    f.draw(fb, Face::Strong, 32.0, MARGIN, 250, MUTED, "MENSAGEM PARA O CLAUDE");
    f.draw(fb, Face::Display, 56.0, MARGIN, 340, INK, &fit(f, Face::Display, 56.0, title, W - 2 * MARGIN));
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
