//! Conversas com o Verso pela ponte da VPS (server/claude-bridge.py), via curl do Kindle.

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
    pub fn bool(&self) -> Option<bool> {
        if let Json::Bool(b) = self { Some(*b) } else { None }
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

/// Endereço fixo da ponte para o curl (`host:443:IP`, em settings/claude_resolve), para quando
/// o DNS da operadora falha: o nome continua valendo para o HTTPS, a conexão vai direto ao IP.
fn resolve_args() -> Vec<String> {
    read_trim(crate::home_dir().join("settings/claude_resolve").to_str().unwrap_or(""))
        .filter(|s| s.split(':').count() == 3)
        .map(|r| vec!["--resolve".to_string(), r])
        .unwrap_or_default()
}

fn token() -> Option<String> {
    read_trim(crate::home_dir().join("settings/claude_token").to_str()?).filter(|s| !s.is_empty())
}

/// Chamada HTTP pela ponte. Devolve o JSON da resposta ou a mensagem de erro para a tela.
pub fn call(method: &str, path: &str, body: Option<&str>, timeout: u32) -> Result<Json, String> {
    let raw = call_raw(method, path, body, timeout)?;
    parse(&raw).ok_or_else(|| "resposta inválida da ponte".into())
}

/// Como `call`, mas devolve o JSON da resposta em texto (para guardar como veio).
pub fn call_raw(method: &str, path: &str, body: Option<&str>, timeout: u32) -> Result<String, String> {
    let (Some(base), Some(tok)) = (url(), token()) else { return Err("A ponte do Verso ainda não foi configurada.".into()) };
    let mut cmd = Command::new("curl");
    cmd.args(resolve_args());
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
        200 if parsed.is_some() => Ok(json.to_string()),
        200 => Err("resposta inválida da ponte".into()),
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
    /// Motor técnico que escreveu a resposta, sem trocar o nome do assistente.
    pub provider: Option<String>,
    /// Motor que ficou indisponível e fez o Verso continuar no outro.
    pub fallback_from: Option<String>,
    /// A pessoa optou por acrescentar o resumo seguro do Brain.
    pub brain: bool,
    pub brain_sources: Vec<String>,
}

#[derive(Clone)]
pub struct ChatFile {
    pub name: String,
    pub size: u64,
    pub mime: String,
    pub kind: String,
}

pub struct Chat {
    pub id: String,
    pub title: String,
    pub messages: Vec<Msg>,
    /// Arquivos que o Verso criou nesta conversa.
    pub files: Vec<ChatFile>,
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
                            provider: x.get("provider").and_then(Json::str).map(str::to_string),
                            fallback_from: x.get("fallback_from").and_then(Json::str).map(str::to_string),
                            brain: x.get("brain").and_then(Json::bool).unwrap_or(false),
                            brain_sources: x.get("brain_sources").map(Json::arr).unwrap_or(&[]).iter().filter_map(|s| s.str().map(str::to_string)).collect(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default(),
        files: files_of(&j),
    })
}

fn file_kind(name: &str, mime: &str) -> String {
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    if mime.starts_with("image/") || ["png", "jpg", "jpeg", "gif"].contains(&ext.as_str()) {
        "image".into()
    } else {
        "file".into()
    }
}

fn files_of(j: &Json) -> Vec<ChatFile> {
    j.get("files")
        .map(|f| {
            f.arr()
                .iter()
                .filter_map(|x| {
                    let name = x.get("name")?.str()?.to_string();
                    let mime = x.get("mime").and_then(Json::str).unwrap_or("application/octet-stream").to_string();
                    Some(ChatFile {
                        kind: x.get("kind").and_then(Json::str).map(str::to_string).unwrap_or_else(|| file_kind(&name, &mime)),
                        size: x.get("size")?.num()? as u64,
                        name,
                        mime,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// A ponte está configurada neste Kindle?
pub fn configured() -> bool {
    url().is_some() && token().is_some()
}

/// Baixa `path` da ponte para `dest` (arquivo temporário e troca no fim).
pub fn fetch(path: &str, dest: &std::path::Path, timeout: u32) -> Result<(), String> {
    let (Some(base), Some(tok)) = (url(), token()) else { return Err("A ponte do Verso ainda não foi configurada.".into()) };
    let tmp = dest.with_extension("parcial");
    let code = Command::new("curl")
        .args(resolve_args())
        .args(["-s", "-m", &timeout.to_string(), "-o", &tmp.to_string_lossy(), "-w", "%{http_code}", "-H", &format!("Authorization: Bearer {tok}")])
        .arg(format!("{}{path}", base.trim_end_matches('/')))
        .output()
        .map_err(|_| "curl indisponível".to_string())?;
    if String::from_utf8_lossy(&code.stdout).trim() != "200" {
        let _ = std::fs::remove_file(&tmp);
        return Err("não consegui baixar".into());
    }
    std::fs::rename(&tmp, dest).map_err(|e| e.to_string())
}

/// Pasta onde ficam os arquivos do Verso no Kindle (aparecem também na biblioteca).
pub fn files_dir() -> std::path::PathBuf {
    std::path::PathBuf::from("/mnt/us/documents/Verso")
}

/// Baixa um arquivo da conversa para o Kindle e devolve onde ficou.
pub fn download(id: &str, name: &str) -> Result<std::path::PathBuf, String> {
    let (Some(base), Some(tok)) = (url(), token()) else { return Err("A ponte do Verso ainda não foi configurada.".into()) };
    if name.contains('/') || name.contains("..") {
        return Err("nome de arquivo inválido".into());
    }
    let dir = files_dir();
    let _ = std::fs::create_dir_all(&dir);
    let dest = dir.join(name);
    let tmp = dir.join(format!(".{name}.parcial"));
    let enc: String = name.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect();
    let code = Command::new("curl")
        .args(resolve_args())
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

/// Manda um arquivo do Kindle para a ponte (anexo de uma pergunta). Devolve o id do anexo.
pub fn upload(path: &std::path::Path, name: &str) -> Result<String, String> {
    let (Some(base), Some(tok)) = (url(), token()) else { return Err("A ponte do Verso ainda não foi configurada.".into()) };
    let enc: String = name.bytes().map(|b| if b.is_ascii_alphanumeric() || b".-_ ".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect();
    let out = Command::new("curl")
        .args(resolve_args())
        .args(["-s", "-m", "240", "-X", "POST", "-H", &format!("Authorization: Bearer {tok}"), "-H", "Content-Type: application/octet-stream", "-H", &format!("X-Nome: {enc}"), "-w", "\n%{http_code}", "--data-binary"])
        .arg(format!("@{}", path.display()))
        .arg(format!("{}/anexos", base.trim_end_matches('/')))
        .stderr(Stdio::null())
        .output()
        .map_err(|_| "curl indisponível".to_string())?;
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let (json, code) = text.rsplit_once('\n').unwrap_or(("", "0"));
    let j = parse(json);
    match code.trim() {
        "200" => j.as_ref().and_then(|j| j.get("id")).and_then(Json::str).map(str::to_string).ok_or_else(|| "resposta inválida da ponte".into()),
        "0" => Err("Sem conexão com a VPS. Confira o Wi-Fi.".into()),
        c => Err(j.as_ref().and_then(|j| j.get("error")).and_then(Json::str).map(str::to_string).unwrap_or(format!("Erro {c} ao mandar o anexo."))),
    }
}

pub fn send(id: Option<&str>, message: &str, context: Option<&str>) -> Result<String, String> {
    send_with(id, message, context, &[], false, false)
}

/// Pergunta com anexos, contexto opt-in do Brain e, se pedido, modo de geração de imagem.
pub fn send_with(id: Option<&str>, message: &str, context: Option<&str>, anexos: &[String], brain: bool, image: bool) -> Result<String, String> {
    let path = id.map(|i| format!("/chats/{i}")).unwrap_or_else(|| "/chats".into());
    let t0 = std::time::Instant::now();
    let list: Vec<String> = anexos.iter().map(|a| json_str(a)).collect();
    let mode = if image { "image" } else { "chat" };
    let body = format!(
        "{{\"text\": {}, \"context\": {}, \"anexos\": [{}], \"brain\": {}, \"mode\": {}}}",
        json_str(message),
        json_str(context.unwrap_or("")),
        list.join(","),
        if brain { "true" } else { "false" },
        json_str(mode),
    );
    let j = call("POST", &path, Some(&body), 330)?;
    log(&format!("verso respondeu em {} s", t0.elapsed().as_secs()));
    j.get("id").and_then(Json::str).map(str::to_string).ok_or_else(|| "resposta sem conversa".into())
}

// ---------- Telas ----------

/// Barra "Pergunte ao Verso…" (nova conversa), no alto da lista.
pub fn new_rect() -> Rect {
    Rect::new(MARGIN, 330, W - 2 * MARGIN, 110)
}
pub fn chat_row(i: usize) -> Rect {
    Rect::new(MARGIN, 520 + i as i32 * 124, W - 2 * MARGIN, 108)
}
pub fn live_rect() -> Rect {
    Rect::new(MARGIN, 1300, W - 2 * MARGIN, 100)
}
pub const ROWS: usize = 6;

fn ago(secs: i64) -> String {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    match now - secs {
        s if s < 3600 => format!("há {} min", (s / 60).max(1)),
        s if s < 86_400 => format!("há {} h", s / 3600),
        s if s < 2 * 86_400 => "ontem".to_string(),
        s => format!("há {} dias", s / 86_400),
    }
}

pub fn draw_hub(fb: &mut Framebuffer, f: &Fonts, chats: &Result<Vec<ChatInfo>, String>, live: usize) {
    fb.fill(fb.screen(), PAPER);
    ui::draw_back(fb, f, false);
    crate::sprites::draw(fb, "cab-claude", Rect::new(W - MARGIN - 250, 70, 260, 226));
    f.draw(fb, Face::Display, 120.0, MARGIN - 6, 290, INK, "Verso");
    // Barra de pergunta: o jeito mais rápido de começar.
    let r = new_rect();
    ui::card(fb, r, r.h / 2, PAPER, false);
    let d = r.h - 28;
    let tx = if ui::icon_disc(fb, r.x + 14, r.y + 14, d, "claude") { r.x + 14 + d + 20 } else { r.x + 36 };
    f.draw(fb, Face::Body, 36.0, tx, r.y + r.h / 2 + 13, MUTED, "Pergunte ao Verso…");
    let plus = Rect::new(r.x + r.w - 14 - d, r.y + 14, d, d);
    fb.round_rect(plus, d / 2, INK);
    let pw = f.measure(Face::Display, 44.0, "+");
    f.draw(fb, Face::Display, 44.0, plus.x + (d - pw) / 2, plus.y + d / 2 + 16, PAPER, "+");
    f.draw(fb, Face::Strong, 26.0, MARGIN, 496, MUTED, "CONVERSAS DO VERSO");
    match chats {
        Err(e) => {
            for (k, l) in [e.as_str(), "As conversas ficam na VPS e aparecem aqui."].iter().enumerate() {
                f.draw(fb, Face::Body, 32.0, MARGIN, 580 + k as i32 * 48, INK, l);
            }
        }
        Ok(v) if v.is_empty() => {
            f.draw(fb, Face::Body, 32.0, MARGIN, 580, INK, "Nenhuma conversa ainda. Toque na barra acima.");
        }
        Ok(v) => {
            for (i, c) in v.iter().take(ROWS).enumerate() {
                let r = chat_row(i);
                ui::card(fb, r, 26, PAPER, false);
                let d = 60;
                let tx = if ui::icon_disc(fb, r.x + 20, r.y + (r.h - d) / 2, d, "claude") { r.x + 20 + d + 20 } else { r.x + 28 };
                f.draw(fb, Face::Strong, 32.0, tx, r.y + 48, INK, &fit(f, Face::Strong, 32.0, &c.title, r.x + r.w - 40 - tx));
                let files = if c.files > 0 { format!(" · {} arquivo{}", c.files, if c.files == 1 { "" } else { "s" }) } else { String::new() };
                let n = c.count / 2;
                f.draw(fb, Face::Body, 25.0, tx, r.y + 86, MUTED, &format!("{} · {n} pergunta{}{files}", ago(c.updated), if n == 1 { "" } else { "s" }));
            }
        }
    }
    let lr = live_rect();
    ui::card(fb, lr, lr.h / 2, if live > 0 { INK } else { PAPER }, false);
    let label = if live == 0 { "Agentes no PC ao vivo".to_string() } else { format!("Agentes no PC ao vivo · {live}") };
    let lw = f.measure(Face::Strong, 34.0, &label);
    f.draw(fb, Face::Strong, 34.0, lr.x + (lr.w - lw) / 2, lr.y + 62, if live > 0 { PAPER } else { INK }, &label);
}

/// Uma linha já quebrada para a tela.
#[derive(Clone, Debug, PartialEq)]
pub enum Line {
    /// "VOCÊ" / "VERSO".
    Label(bool),
    /// Motor técnico, uso de Brain e troca de contingência, sem virar resposta do modelo.
    Meta(String),
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
            // Sua mensagem não tem rótulo (o balão à direita já diz); a do Claude tem o ícone.
            Line::Label(true) => 20,
            Line::Label(false) => 62,
            Line::Meta(_) => 34,
            Line::Heading(_) => 62,
            Line::Code(_) => 42,
            Line::Context(_) => 44,
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
pub fn wrap_words(f: &Fonts, face: Face, size: f32, text: &str, max_w: i32) -> Vec<String> {
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
        if !m.user {
            let mut meta = Vec::new();
            if let Some(provider) = m.provider.as_deref() {
                let provider = match provider {
                    "claude" => "Claude",
                    "codex" => "Codex",
                    "imagem" => "Imagem",
                    other => other,
                };
                meta.push(format!("Verso · {provider}"));
            }
            if let Some(from) = m.fallback_from.as_deref() {
                let from = match from { "claude" => "Claude", "codex" => "Codex", other => other };
                meta.push(format!("continuou após {from}"));
            }
            if !m.brain_sources.is_empty() {
                meta.push("Brain incluído".into());
            }
            if !meta.is_empty() {
                lines.push(Line::Meta(meta.join(" · ")));
            }
        }
        if let Some(c) = m.context.as_deref().filter(|c| !c.is_empty()) {
            let first = c.lines().next().unwrap_or("");
            lines.push(Line::Context(fit(f, Face::Body, 24.0, &format!("sobre: {first}"), max_w - 200)));
        }
        if m.user && m.brain {
            lines.push(Line::Context("Brain incluído nesta pergunta".into()));
        }
        // Suas mensagens vão num balão mais estreito, à direita.
        lines.extend(layout_text(f, &m.text, m.user, if m.user { max_w - 200 } else { max_w }));
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
    let user_text = |l: &Line| matches!(l, Line::Text(_, true));
    for (i, l) in lines.iter().enumerate() {
        let h = l.height();
        let base = y + h - 14;
        // Começo de uma mensagem sua: o balão inteiro, alinhado à direita.
        if user_text(l) && (i == 0 || !user_text(&lines[i - 1])) {
            let run: Vec<&Line> = lines[i..].iter().take_while(|x| user_text(x)).collect();
            let bh: i32 = run.iter().map(|x| x.height()).sum();
            let bw = run.iter().map(|x| if let Line::Text(t, _) = x { f.measure(Face::Body, BODY, &t.replace("**", "")) } else { 0 }).max().unwrap_or(0) + 64;
            let r = Rect::new(W - MARGIN - bw + 8, y - 14, bw, bh + 28);
            fb.round_rect(r, 30, ui::SURFACE);
        }
        match l {
            Line::Label(user) => {
                if !*user {
                    let d = 46;
                    let x = if ui::icon_disc(fb, MARGIN - 4, y + 4, d, "claude") { MARGIN + d + 10 } else { MARGIN };
                    f.draw(fb, Face::Strong, 28.0, x, y + 40, INK, "Verso");
                }
            }
            Line::Meta(t) => {
                f.draw(fb, Face::Body, 22.0, MARGIN, base, MUTED, &fit(f, Face::Body, 22.0, t, W - 2 * MARGIN));
            }
            Line::Text(t, user) => {
                if *user {
                    // Linha dentro do balão: começa na borda esquerda dele.
                    let run_start = (0..=i).rev().take_while(|k| user_text(&lines[*k])).last().unwrap_or(i);
                    let run: Vec<&Line> = lines[run_start..].iter().take_while(|x| user_text(x)).collect();
                    let bw = run.iter().map(|x| if let Line::Text(t, _) = x { f.measure(Face::Body, BODY, &t.replace("**", "")) } else { 0 }).max().unwrap_or(0) + 64;
                    draw_rich(fb, f, W - MARGIN - bw + 8 + 32, base, BODY, INK, t);
                } else {
                    draw_rich(fb, f, MARGIN, base, BODY, INK, t);
                }
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
                // O que foi junto (tela, jogo, livro), em cinza acima do balão.
                let w = f.measure(Face::Body, 24.0, t);
                f.draw(fb, Face::Body, 24.0, W - MARGIN - w, base - 8, MUTED, t);
            }
            Line::Blank => {}
        }
        y += h;
    }
}

// Barra de baixo da conversa: ‹  Responder  + Anexar  ›
pub fn prev_rect() -> Rect {
    Rect::new(MARGIN, 1316, 100, 100)
}
pub fn reply_rect() -> Rect {
    Rect::new(MARGIN + 116, 1316, W - 2 * MARGIN - 116 * 2 - 236, 100)
}
pub fn chat_attach_rect() -> Rect {
    Rect::new(W - MARGIN - 116 - 220, 1316, 220, 100)
}
pub fn next_rect() -> Rect {
    Rect::new(W - MARGIN - 100, 1316, 100, 100)
}
/// "Arquivos · N" no alto, quando o Verso criou algum.
pub fn files_rect() -> Rect {
    Rect::new(W - MARGIN - 300, 76, 300, 84)
}

fn pill(fb: &mut Framebuffer, f: &Fonts, r: Rect, label: &str, filled: bool) {
    ui::button(fb, f, r, label, filled);
}

pub fn draw_chat(fb: &mut Framebuffer, f: &Fonts, chat: &Chat, pages: &[Vec<Line>], page: usize) {
    fb.fill(fb.screen(), PAPER);
    ui::draw_back(fb, f, false);
    if !chat.files.is_empty() {
        pill(fb, f, files_rect(), &format!("Arquivos · {}", chat.files.len()), false);
    }
    let kicker = if pages.len() > 1 { format!("CONVERSA · PÁGINA {} DE {}", page + 1, pages.len()) } else { "CONVERSA".to_string() };
    f.draw(fb, Face::Strong, 24.0, MARGIN, 214, MUTED, &kicker);
    f.draw(fb, Face::Display, 50.0, MARGIN, 274, INK, &fit(f, Face::Display, 50.0, &chat.title, W - 2 * MARGIN));
    fb.fill(Rect::new(MARGIN, 300, W - 2 * MARGIN, 2), 0xC8);
    if let Some(lines) = pages.get(page) {
        draw_lines(fb, f, lines, TEXT_TOP);
    }
    if page > 0 {
        pill(fb, f, prev_rect(), "‹", false);
    }
    pill(fb, f, reply_rect(), "Responder", true);
    pill(fb, f, chat_attach_rect(), "+ Anexar", false);
    if page + 1 < pages.len() {
        pill(fb, f, next_rect(), "›", false);
    }
}

// ---------- Arquivos do Verso ----------

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

pub fn opens_as_image(file: &ChatFile) -> bool {
    let ext = file.name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    file.kind == "image" || file.mime.starts_with("image/") || ["png", "jpg", "jpeg", "gif"].contains(&ext.as_str())
}

pub fn draw_files(fb: &mut Framebuffer, f: &Fonts, chat: &Chat, note: &str) {
    fb.fill(fb.screen(), PAPER);
    ui::draw_back(fb, f, false);
    f.draw(fb, Face::Strong, 30.0, MARGIN, 240, MUTED, "ARQUIVOS DO VERSO");
    f.draw(fb, Face::Display, 52.0, MARGIN, 320, INK, &fit(f, Face::Display, 52.0, &chat.title, W - 2 * MARGIN));
    for (i, file) in chat.files.iter().take(FILE_ROWS).enumerate() {
        let r = file_row(i);
        fb.fill(Rect::new(MARGIN, r.y, W - 2 * MARGIN, 3), INK);
        f.draw(fb, Face::Display, 42.0, MARGIN, r.y + 60, INK, &fit(f, Face::Display, 42.0, &file.name, W - 2 * MARGIN));
        let how = if opens_as_image(file) { "abre imagem" } else if opens_in_reader(&file.name) { "abre no KOReader" } else { "abre aqui" };
        f.draw(fb, Face::Body, 28.0, MARGIN + 2, r.y + 100, MUTED, &format!("{} · {how} · fica em documents/Verso", human(file.size)));
    }
    if !note.is_empty() {
        f.draw(fb, Face::Body, 30.0, MARGIN, 1300, INK, &fit(f, Face::Body, 30.0, note, W - 2 * MARGIN));
    }
}

/// Leitor simples de texto/Markdown (arquivos do Verso e análises).
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

/// Imagem recebida ou gerada na conversa, ajustada para a tela e-ink sem esticar.
pub fn draw_image(fb: &mut Framebuffer, f: &Fonts, path: Option<&std::path::Path>, title: &str) {
    fb.fill(fb.screen(), PAPER);
    ui::draw_back(fb, f, false);
    f.draw(fb, Face::Display, 46.0, MARGIN, 250, INK, &fit(f, Face::Display, 46.0, title, W - 2 * MARGIN));
    let Some(path) = path else {
        f.draw(fb, Face::Body, 32.0, MARGIN, 280, MUTED, "Não consegui abrir esta imagem.");
        return;
    };
    let Ok((source_w, source_h)) = image::image_dimensions(path) else {
        f.draw(fb, Face::Body, 32.0, MARGIN, 340, MUTED, "Não consegui ler esta imagem.");
        return;
    };
    if source_w == 0 || source_h == 0 || source_w > 8192 || source_h > 8192 || u64::from(source_w) * u64::from(source_h) > 16_000_000 {
        f.draw(fb, Face::Body, 32.0, MARGIN, 340, MUTED, "Esta imagem é grande demais para o Kindle.");
        return;
    }
    let Ok(image) = image::open(path) else {
        f.draw(fb, Face::Body, 32.0, MARGIN, 340, MUTED, "Não consegui ler esta imagem.");
        return;
    };
    let gray = image.to_luma8();
    let max_w = (W - 2 * MARGIN) as u32;
    let image_top = 300i32;
    let max_h = 1448u32 - image_top as u32 - 48;
    let ratio = (max_w as f32 / gray.width() as f32).min(max_h as f32 / gray.height() as f32).min(1.0);
    let w = ((gray.width() as f32 * ratio).round() as u32).max(1);
    let h = ((gray.height() as f32 * ratio).round() as u32).max(1);
    let resized = image::imageops::resize(&gray, w, h, image::imageops::FilterType::Triangle);
    let x = (W - w as i32) / 2;
    let y = image_top + (max_h as i32 - h as i32) / 2;
    fb.blit(x, y, w as i32, h as i32, resized.as_raw());
}

// ---------- Pergunta ao Verso ----------

/// O que a tela de pergunta mostra.
pub struct Compose<'a> {
    /// Nomes dos arquivos anexados.
    pub attach: &'a [String],
    /// Conversa em andamento (título), ou None para conversa nova.
    pub chat_title: Option<&'a str>,
    /// Última resposta do Verso (quando continua uma conversa sem contexto novo).
    pub last_reply: Option<&'a str>,
    /// Sobre o que é: (título, detalhe), quando veio de um jogo, livro ou tela.
    pub about: Option<(&'a str, &'a str)>,
    pub suggestions: &'a [String],
    pub text: &'a str,
    pub hint: &'a str,
    /// Usa o resumo seguro do Brain sincronizado do PC nesta pergunta.
    pub brain: bool,
    /// A instrução vira uma imagem, em vez de uma resposta de texto.
    pub image: bool,
}

const CARD_Y: i32 = 288;
const CARD_H: i32 = 132;

fn card_rect() -> Rect {
    Rect::new(MARGIN - 8, CARD_Y, W - 2 * MARGIN + 16, CARD_H)
}

/// Botão "Tirar" do cartão de contexto.
pub fn context_rect() -> Rect {
    let c = card_rect();
    Rect::new(c.x + c.w - 24 - 140, c.y + (c.h - 60) / 2, 140, 60)
}

fn chips_top(has_card: bool) -> i32 {
    if has_card { CARD_Y + CARD_H + 30 } else { CARD_Y + 8 }
}

/// Sugestões em fileiras (no máximo duas), medidas com a fonte das pílulas.
pub fn chip_rects(f: &Fonts, suggestions: &[String], has_card: bool) -> Vec<Rect> {
    let (h, gap) = (78, 14);
    let mut v = Vec::new();
    let (mut x, mut y) = (MARGIN - 8, chips_top(has_card));
    for s in suggestions {
        let w = (f.measure(Face::Strong, 30.0, s) + 64).min(W - 2 * MARGIN + 16);
        if x + w > W - MARGIN + 8 {
            x = MARGIN - 8;
            y += h + gap;
            if y > chips_top(has_card) + h + gap {
                break;
            }
        }
        v.push(Rect::new(x, y, w, h));
        x += w + gap;
    }
    v
}

pub fn compose_field_rect() -> Rect {
    Rect::new(MARGIN - 8, 660, W - 2 * MARGIN + 16, 170)
}

/// Campo de texto (redesenhado sozinho enquanto digita).
pub fn draw_compose_field(fb: &mut Framebuffer, f: &Fonts, text: &str, hint: &str, image: bool) {
    let r = compose_field_rect();
    fb.fill(Rect::new(0, r.y - 6, W, 892 - (r.y - 6)), PAPER);
    fb.round_outline(r, 28, 4, INK, PAPER);
    let max_w = r.w - 64;
    if text.is_empty() {
        let placeholder = if image { "Descreva a imagem que quer criar…" } else { "Ou escreva a sua pergunta…" };
        f.draw(fb, Face::Body, 40.0, r.x + 32, r.y + 70, MUTED, placeholder);
        fb.fill(Rect::new(r.x + 30, r.y + 30, 4, 52), INK);
    } else {
        // Até duas linhas; se passar, mostra o final.
        let mut lines = wrap_words(f, Face::Body, 40.0, text, max_w);
        if lines.len() > 2 {
            lines = lines.split_off(lines.len() - 2);
        }
        let mut end = r.x + 32;
        for (k, l) in lines.iter().enumerate() {
            end = f.draw(fb, Face::Body, 40.0, r.x + 32, r.y + 70 + k as i32 * 56, INK, l);
        }
        let ly = r.y + 30 + (lines.len() as i32 - 1) * 56;
        fb.fill(Rect::new(end + 4, ly, 4, 52), INK);
    }
    f.draw(fb, Face::Body, 26.0, MARGIN, r.y + r.h + 40, MUTED, &fit(f, Face::Body, 26.0, hint, W - 2 * MARGIN));
}

pub fn draw_compose(fb: &mut Framebuffer, f: &Fonts, c: &Compose) {
    fb.fill(fb.screen(), PAPER);
    ui::draw_back(fb, f, false);
    // Cabeçalho: a marca do Verso e a pergunta.
    ui::icon(fb, ui::Module::Claude, MARGIN - 4, 194, 60, INK);
    let title = if c.image { "Crie uma imagem" } else if c.chat_title.is_some() { "Continue a conversa" } else { "Pergunte ao Verso" };
    // Respiro entre a fileira de botões (Imagem, Brain, Anexar) e o título.
    f.draw(fb, Face::Display, 56.0, MARGIN + 76, 248, INK, title);
    if let Some(t) = c.chat_title {
        let t = fit(f, Face::Body, 26.0, t, W - 2 * MARGIN - 560);
        let w = f.measure(Face::Body, 26.0, &t);
        f.draw(fb, Face::Body, 26.0, W - MARGIN - 270 - w, 130, MUTED, &t);
    }
    let has_card = c.about.is_some() || c.last_reply.is_some();
    if let Some((what, detail)) = c.about {
        let r = card_rect();
        fb.round_rect(r, 30, INK);
        f.draw(fb, Face::Strong, 24.0, r.x + 32, r.y + 42, 0xBB, "VAI JUNTO O QUE ESTÁ NA TELA");
        let room = context_rect().x - r.x - 56;
        let end = f.draw(fb, Face::Display, 42.0, r.x + 30, r.y + 96, PAPER, &fit(f, Face::Display, 42.0, what, room));
        if !detail.is_empty() && end + 30 < r.x + 32 + room {
            f.draw(fb, Face::Body, 28.0, end + 18, r.y + 94, 0xCC, &fit(f, Face::Body, 28.0, detail, r.x + 32 + room - end - 18));
        }
        let b = context_rect();
        fb.round_outline(b, b.h / 2, 3, PAPER, INK);
        let w = f.measure(Face::Strong, 28.0, "Tirar");
        f.draw(fb, Face::Strong, 28.0, b.x + (b.w - w) / 2, b.y + 40, PAPER, "Tirar");
    } else if let Some(reply) = c.last_reply {
        let r = card_rect();
        fb.round_rect(r, 30, 0xEC);
        f.draw(fb, Face::Strong, 24.0, r.x + 32, r.y + 42, MUTED, "O VERSO DISSE");
        let flat: String = reply.replace("**", "").split_whitespace().collect::<Vec<_>>().join(" ");
        let lines = wrap_words(f, Face::Body, 28.0, &flat, r.w - 64);
        let mut first = lines.first().cloned().unwrap_or_default();
        if lines.len() > 1 {
            first = fit(f, Face::Body, 28.0, &format!("{first}…"), r.w - 64);
        }
        f.draw(fb, Face::Body, 28.0, r.x + 32, r.y + 92, INK, &first);
    }
    // Perguntas prontas: um toque envia.
    let chips = if c.image { Vec::new() } else { chip_rects(f, c.suggestions, has_card) };
    for (s, r) in c.suggestions.iter().zip(&chips) {
        fb.round_outline(*r, r.h / 2, 3, INK, PAPER);
        f.draw(fb, Face::Strong, 30.0, r.x + 32, r.y + 50, INK, s);
    }
    // Espaço sobrando: dica contextual em vez de deixar um vão vazio na tela e-ink.
    let bottom = chips.iter().map(|r| r.y + r.h).max().unwrap_or(CARD_Y);
    if c.image && compose_field_rect().y - bottom > 110 {
        f.draw(fb, Face::Body, 28.0, MARGIN, bottom + 72, MUTED, "Descreva o assunto, o estilo e o enquadramento.");
        f.draw(fb, Face::Body, 28.0, MARGIN, bottom + 112, MUTED, "O resultado aparece em Arquivos quando ficar pronto.");
    } else if !has_card && compose_field_rect().y - bottom > 110 {
        f.draw(fb, Face::Body, 28.0, MARGIN, bottom + 72, MUTED, "Em qualquer tela, puxe de cima para baixo: luz, Wi-Fi");
        f.draw(fb, Face::Body, 28.0, MARGIN, bottom + 112, MUTED, "e o Verso sobre o que está nela.");
    }
    // Contextos e anexos ficam à vista: a pessoa sabe exatamente o que irá junto.
    ui::button(fb, f, image_rect(), if c.image { "Imagem ✓" } else { "Imagem" }, c.image);
    ui::button(fb, f, brain_rect(), if c.brain { "Brain ✓" } else { "Brain" }, c.brain);
    let ar = attach_rect();
    let label = if c.attach.is_empty() { "+ Anexar".to_string() } else { format!("Anexos · {}", c.attach.len()) };
    ui::button(fb, f, ar, &label, !c.attach.is_empty());
    if !c.attach.is_empty() {
        let t = fit(f, Face::Strong, 26.0, &format!("Vai junto: {}", c.attach.join(" · ")), W - 2 * MARGIN);
        let y = compose_field_rect().y - 22;
        fb.fill(Rect::new(MARGIN - 8, y - 34, W - 2 * MARGIN + 16, 46), PAPER);
        f.draw(fb, Face::Strong, 26.0, MARGIN, y, INK, &t);
    }
    draw_compose_field(fb, f, c.text, c.hint, c.image);
}

/// Botões de contexto e mídia da pergunta ao Verso.
pub fn image_rect() -> Rect {
    // Mesma linha e mesmo espaço (20 px) depois do Voltar.
    Rect::new(MARGIN - 16 + 230 + 20, 72, 190, 88)
}

pub fn brain_rect() -> Rect {
    let r = image_rect();
    Rect::new(r.x + r.w + 20, 72, 190, 88)
}

/// Botão "Anexar" da pergunta ao Verso.
pub fn attach_rect() -> Rect {
    Rect::new(W - MARGIN + 16 - 230, 72, 230, 88)
}

/// Anexos possíveis: livros e artigos (arquivo), destaques de um livro (texto), cifras e fotos.
pub struct AttachItem {
    pub label: String,
    pub sub: String,
    pub path: Option<std::path::PathBuf>,
    /// Destaques de um livro: (título, texto) vira um .txt na hora de mandar.
    pub text: Option<(String, String)>,
}

pub fn attach_items() -> Vec<AttachItem> {
    let mut v = Vec::new();
    let lib = crate::apps::Library::scan("");
    for b in lib.books.iter().filter(|b| !b.amazon_only && b.size <= 16_000_000).take(40) {
        v.push(AttachItem { label: b.title.clone(), sub: format!("{} · {} KB", b.kind.to_uppercase(), b.size / 1024), path: Some(b.path.clone()), text: None });
    }
    for b in crate::clippings::load() {
        let text: String = b.clips.iter().map(|c| if c.note { format!("[nota] {}", c.text) } else { c.text.clone() }).collect::<Vec<_>>().join("\n\n");
        v.push(AttachItem { label: format!("Destaques: {}", b.title), sub: format!("{} trechos", b.clips.len()), path: None, text: Some((b.title.clone(), text)) });
    }
    for (dir, kind) in [(crate::home_dir().join("data/violao/cifras"), "Cifra"), (crate::home_dir().join("data/fotos"), "Foto")] {
        let mut fs: Vec<_> = std::fs::read_dir(&dir).map(|it| it.flatten().map(|e| e.path()).filter(|p| p.is_file()).collect()).unwrap_or_default();
        fs.sort();
        for p in fs.into_iter().rev().take(20) {
            let name = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            v.push(AttachItem { label: format!("{kind}: {name}"), sub: kind.to_lowercase(), path: Some(p), text: None });
        }
    }
    v
}

/// Arquivo pronto para mandar (os destaques viram .txt em data/anexos).
pub fn attach_file(it: &AttachItem) -> Option<(std::path::PathBuf, String)> {
    if let Some(p) = &it.path {
        return Some((p.clone(), p.file_name()?.to_string_lossy().to_string()));
    }
    let (title, text) = it.text.as_ref()?;
    let d = crate::home_dir().join("data/anexos");
    let _ = std::fs::create_dir_all(&d);
    let name: String = format!("Destaques - {}", title).chars().filter(|c| !"/\\:*?\"<>|".contains(*c)).take(80).collect::<String>() + ".txt";
    let p = d.join(&name);
    std::fs::write(&p, format!("Destaques do livro {title}\n\n{text}")).ok()?;
    Some((p, name))
}

pub const ATTACH_ROWS: usize = 8;

pub fn attach_row(k: usize) -> Rect {
    Rect::new(MARGIN - 8, 300 + k as i32 * 112, W - 2 * MARGIN + 16, 100)
}

pub fn attach_done_rect() -> Rect {
    Rect::new(MARGIN, 1300, W - 2 * MARGIN - 300, 96)
}

pub fn attach_nav(k: usize) -> Rect {
    Rect::new(W - MARGIN - 280 + k as i32 * 145, 1300, 135, 96)
}

pub fn draw_attach(fb: &mut Framebuffer, f: &Fonts, items: &[AttachItem], sel: &[usize], page: usize) {
    fb.fill(fb.screen(), PAPER);
    ui::draw_back(fb, f, false);
    f.draw(fb, Face::Display, 64.0, MARGIN, 250, INK, "Anexar");
    if items.is_empty() {
        f.draw(fb, Face::Body, 32.0, MARGIN, 360, MUTED, "Nada para anexar ainda: livros, artigos, destaques,");
        f.draw(fb, Face::Body, 32.0, MARGIN, 404, MUTED, "cifras e fotos do Kindle aparecem aqui.");
    }
    for (k, (i, it)) in items.iter().enumerate().skip(page * ATTACH_ROWS).take(ATTACH_ROWS).enumerate() {
        let r = attach_row(k);
        let on = sel.contains(&i);
        ui::card(fb, r, 24, if on { INK } else { PAPER }, false);
        let fg = if on { PAPER } else { INK };
        f.draw(fb, Face::Strong, 30.0, r.x + 26, r.y + 44, fg, &fit(f, Face::Strong, 30.0, &it.label, r.w - 170));
        f.draw(fb, Face::Body, 24.0, r.x + 28, r.y + 80, if on { 0xCC } else { MUTED }, &fit(f, Face::Body, 24.0, &it.sub, r.w - 170));
        let t = if on { "anexado" } else { "anexar" };
        let w = f.measure(Face::Strong, 24.0, t);
        f.draw(fb, Face::Strong, 24.0, r.x + r.w - 26 - w, r.y + 58, if on { PAPER } else { MUTED }, t);
    }
    let label = if sel.is_empty() { "Pronto".to_string() } else { format!("Pronto · {}", sel.len()) };
    ui::button(fb, f, attach_done_rect(), &label, true);
    let pages = items.len().div_ceil(ATTACH_ROWS);
    if page > 0 {
        ui::button(fb, f, attach_nav(0), "‹", false);
    }
    if page + 1 < pages {
        ui::button(fb, f, attach_nav(1), "›", false);
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

#[cfg(test)]
mod render {
    use super::*;
    /// `VERSO_INK_SHOTS=pasta cargo test pergunta_em_imagem -- --ignored`
    #[test]
    #[ignore]
    fn pergunta_em_imagem() {
        let out = std::path::PathBuf::from(std::env::var("VERSO_INK_SHOTS").expect("VERSO_INK_SHOTS"));
        let f = Fonts::new();
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        let cases = [
            ("pergunta-uno", Compose { attach: &[], chat_title: None, last_reply: None, about: Some(("UNO", "Mesa: 7 azul · sua mão: 8 cartas")), suggestions: &s(&["Qual a melhor jogada agora?", "Que cor eu escolho?", "Explique o acúmulo"]), text: "", hint: "Toque numa pergunta pronta, ou escreva e toque em OK.", brain: true, image: false }),
            ("pergunta-nova", Compose { attach: &[], chat_title: None, last_reply: None, about: None, suggestions: &s(&["Resuma o livro que estou lendo", "Organize meu dia", "Um desafio de lógica", "Me ensine algo em 2 minutos"]), text: "", hint: "Descreva a imagem e toque em OK.", brain: false, image: true }),
            ("pergunta-continua", Compose { attach: &[], chat_title: Some("Ola"), last_reply: Some("Olá, Pedro! Em que posso ajudar hoje? Posso ler seus livros e ajudar nos jogos."), about: None, suggestions: &s(&["Explique melhor", "Dê um exemplo", "Resuma em tópicos"]), text: "quero saber sobre o livro", hint: "Toque numa pergunta pronta, ou escreva e toque em OK.", brain: true, image: false }),
        ];
        for (name, c) in cases {
            let mut fb = Framebuffer::memory();
            draw_compose(&mut fb, &f, &c);
            crate::keyboard::draw(&mut fb, &f, &crate::keyboard::layout(crate::keyboard::Mode::Letters, true));
            fb.save_pgm(&out.join(format!("{name}.pgm")));
        }
        // Uma imagem gerada abre sem depender do leitor de texto ou do KOReader.
        let source = out.join(".imagem-fonte.png");
        let sample = image::GrayImage::from_fn(320, 480, |x, y| image::Luma([if (x / 32 + y / 32) % 2 == 0 { 0x40 } else { 0xD0 }]));
        sample.save(&source).unwrap();
        let mut fb = Framebuffer::memory();
        draw_image(&mut fb, &f, Some(&source), "imagem-gerada.png");
        fb.save_pgm(&out.join("imagem-gerada.pgm"));
        std::fs::remove_file(source).unwrap();
    }
}
