//! Painel do Verso Ink ligado à VPS: Edição da manhã, avisos dos agentes, aprovações do
//! Claude Code, caixa de entrada (artigos e fotos mandados do celular), porta-retrato e terminal.
//!
//! A rede é sempre pela ponte (chat::call / chat::fetch); o que chega fica guardado em data/,
//! para a tela abrir na hora e funcionar sem Wi-Fi.

use crate::chat::{self, Json};
use crate::fb::{Framebuffer, Rect};
use crate::text::{Face, Fonts};
use crate::ui::{self, INK, MUTED, PAPER};
use std::path::PathBuf;

const W: i32 = 1072;
const MARGIN: i32 = 72;

fn data(name: &str) -> PathBuf {
    let d = crate::home_dir().join("data");
    let _ = std::fs::create_dir_all(&d);
    d.join(name)
}

/// Data de hoje no fuso do Kindle, como "2026-11-12" (o mesmo formato da edição).
pub fn today_iso(offset_min: i64) -> String {
    let n = crate::clock::now(offset_min);
    format!("{:04}-{:02}-{:02}", n.year, n.month, n.day)
}

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

// ---------- Avisos ----------

#[derive(Clone)]
pub struct Notice {
    pub id: String,
    pub text: String,
    pub critical: bool,
}

fn parse_notices(j: &Json) -> Vec<Notice> {
    j.arr()
        .iter()
        .filter_map(|a| {
            let until = a.get("until")?.num()? as i64;
            let n = Notice { id: a.get("id")?.str()?.into(), text: a.get("text")?.str()?.into(), critical: a.get("level").and_then(Json::str) == Some("critico") };
            (until > now()).then_some(n)
        })
        .collect()
}

/// Avisos guardados (os da última consulta, só os que ainda valem).
pub fn notices() -> Vec<Notice> {
    std::fs::read_to_string(data("avisos.json")).ok().and_then(|s| chat::parse(&s)).map(|j| parse_notices(&j)).unwrap_or_default()
}

// ---------- Aprovações ----------

#[derive(Clone)]
pub struct Approval {
    pub id: String,
    pub project: String,
    pub tool: String,
    pub summary: String,
    pub created: i64,
}

/// Estado vindo da VPS numa consulta só (avisos, aprovações, modo longe).
pub struct Sync {
    pub approvals: Vec<Approval>,
    pub away: bool,
}

/// Consulta a VPS: avisos (guarda), modo longe e aprovações pendentes. Sem ponte, nada.
pub fn sync() -> Option<Sync> {
    if !chat::configured() {
        return None;
    }
    if let Ok(j) = chat::call("GET", "/avisos", None, 12) {
        let _ = std::fs::write(data("avisos.json"), json_of_notices(&parse_notices(&j)));
    }
    let away = chat::call("GET", "/longe", None, 12).ok().and_then(|j| j.get("on").and_then(Json::bool)).unwrap_or(false);
    let approvals = if away {
        chat::call("GET", "/aprovacoes", None, 12)
            .map(|j| {
                j.arr()
                    .iter()
                    .filter_map(|a| {
                        Some(Approval {
                            id: a.get("id")?.str()?.into(),
                            project: a.get("project").and_then(Json::str).unwrap_or("").into(),
                            tool: a.get("tool").and_then(Json::str).unwrap_or("").into(),
                            summary: a.get("summary").and_then(Json::str).unwrap_or("").into(),
                            created: a.get("created").and_then(Json::num).unwrap_or(0.0) as i64,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    Some(Sync { approvals, away })
}

fn json_of_notices(v: &[Notice]) -> String {
    let items: Vec<String> = v
        .iter()
        .map(|n| format!("{{\"id\": {}, \"text\": {}, \"level\": \"{}\", \"until\": {}}}", chat::json_str(&n.id), chat::json_str(&n.text), if n.critical { "critico" } else { "info" }, now() + 3600))
        .collect();
    format!("[{}]", items.join(", "))
}

pub fn decide(id: &str, allow: bool) -> Result<(), String> {
    chat::call("POST", &format!("/aprovacoes/{id}"), Some(&format!("{{\"decision\": \"{}\"}}", if allow { "permitir" } else { "negar" })), 15).map(|_| ())
}

pub fn set_away(on: bool) -> Result<bool, String> {
    chat::call("POST", "/longe", Some(&format!("{{\"on\": {on}}}")), 15).map(|j| j.get("on").and_then(Json::bool).unwrap_or(on))
}

pub fn approve_rect(allow: bool) -> Rect {
    let w = (W - 2 * MARGIN - 24) / 2;
    Rect::new(if allow { MARGIN + w + 24 } else { MARGIN }, 1250, w, 110)
}

/// Pedido de permissão de um agente, em tela cheia (interrompe o que estiver aberto).
pub fn draw_approval(fb: &mut Framebuffer, f: &Fonts, a: &Approval, more: usize) {
    fb.fill(fb.screen(), PAPER);
    f.draw(fb, Face::Strong, 30.0, MARGIN, 120, MUTED, "UM AGENTE PEDE PERMISSÃO");
    f.draw(fb, Face::Display, 84.0, MARGIN - 4, 240, INK, &crate::apps::fit(f, Face::Display, 84.0, &a.tool, W - 2 * MARGIN));
    f.draw(fb, Face::Body, 32.0, MARGIN, 296, MUTED, &format!("projeto {}", a.project));
    let lines = chat::wrap_words(f, Face::Body, 32.0, &a.summary, W - 2 * MARGIN - 64);
    let n = lines.len().clamp(2, 16) as i32;
    let r = Rect::new(MARGIN, 340, W - 2 * MARGIN, 60 + n * 44);
    ui::card(fb, r, 30, ui::SURFACE, false);
    let mut y = r.y + 60;
    for l in lines.iter().take(16) {
        f.draw(fb, Face::Body, 32.0, r.x + 32, y, INK, l);
        y += 44;
    }
    let left = (600 - (now() - a.created)).max(0);
    let t = format!("Sem resposta em {} min, o pedido é negado.{}", (left + 59) / 60, if more > 0 { format!(" Mais {more} na fila.") } else { String::new() });
    f.draw(fb, Face::Strong, 28.0, MARGIN, 1200, INK, &t);
    ui::button(fb, f, approve_rect(false), "Negar", false);
    ui::button(fb, f, approve_rect(true), "Permitir", true);
}

// ---------- Edição da manhã ----------

pub struct Edition {
    pub day: String,
    pub headline: String,
    pub sections: Vec<(String, Vec<String>)>,
}

fn parse_edition(j: &Json) -> Option<Edition> {
    Some(Edition {
        day: j.get("dia")?.str()?.into(),
        headline: j.get("manchete").and_then(Json::str).unwrap_or("").into(),
        sections: j
            .get("secoes")?
            .arr()
            .iter()
            .filter_map(|s| Some((s.get("titulo")?.str()?.to_string(), s.get("itens")?.arr().iter().filter_map(|i| i.str().map(str::to_string)).collect())))
            .collect(),
    })
}

/// Edição guardada (a última que chegou).
pub fn edition() -> Option<Edition> {
    std::fs::read_to_string(data("edicao.json")).ok().and_then(|s| chat::parse(&s)).and_then(|j| parse_edition(&j))
}

/// Busca a edição na VPS e guarda. Devolve true se chegou uma nova.
pub fn fetch_edition() -> bool {
    let dest = data("edicao.json");
    let before = std::fs::read_to_string(&dest).unwrap_or_default();
    if chat::fetch("/edicao", &dest, 20).is_err() {
        return false;
    }
    std::fs::read_to_string(&dest).unwrap_or_default() != before
}

/// Linhas da edição já quebradas na largura da tela: (texto, estilo 0 manchete 1 seção 2 item).
fn edition_lines(f: &Fonts, e: &Edition) -> Vec<(String, u8)> {
    let mut v = Vec::new();
    for l in chat::wrap_words(f, Face::Display, 50.0, &e.headline, W - 2 * MARGIN) {
        v.push((l, 0));
    }
    for (title, items) in &e.sections {
        v.push((title.clone(), 1));
        for it in items {
            for (k, l) in chat::wrap_words(f, Face::Body, 32.0, it, W - 2 * MARGIN - 40).into_iter().enumerate() {
                v.push((if k == 0 { format!("• {l}") } else { format!("  {l}") }, 2));
            }
        }
    }
    v
}

fn line_h(style: u8) -> i32 {
    match style {
        0 => 60,
        1 => 86,
        _ => 44,
    }
}

/// Páginas da edição (índices de linhas).
pub fn edition_pages(f: &Fonts, e: &Edition) -> Vec<(usize, usize)> {
    let lines = edition_lines(f, e);
    let (top, bottom) = (330, 1290);
    let mut pages = Vec::new();
    let (mut start, mut y) = (0, top);
    for (i, (_, st)) in lines.iter().enumerate() {
        let h = line_h(*st);
        if y + h > bottom && i > start {
            pages.push((start, i));
            start = i;
            y = top;
        }
        y += h;
    }
    pages.push((start, lines.len()));
    pages
}

pub fn edition_nav(k: usize) -> Rect {
    Rect::new(if k == 0 { MARGIN } else { W - MARGIN - 260 }, 1320, 260, 88)
}

pub fn draw_edition(fb: &mut Framebuffer, f: &Fonts, e: Option<&Edition>, page: usize, note: &str) {
    fb.fill(fb.screen(), PAPER);
    ui::draw_back(fb, f, false);
    crate::sprites::draw(fb, "cab-ler", Rect::new(W - MARGIN - 250, 90, 260, 200));
    f.draw(fb, Face::Strong, 28.0, MARGIN, 220, MUTED, "EDIÇÃO DA MANHÃ");
    let Some(e) = e else {
        f.draw(fb, Face::Display, 60.0, MARGIN, 300, INK, "Ainda não chegou");
        for (k, l) in chat::wrap_words(f, Face::Body, 32.0, "A edição é montada às 6h na VPS, com os agentes da noite, avisos, notícias e o Brain. Com Wi-Fi, ela aparece aqui sozinha.", W - 2 * MARGIN).iter().enumerate() {
            f.draw(fb, Face::Body, 32.0, MARGIN, 380 + k as i32 * 44, MUTED, l);
        }
        if !note.is_empty() {
            f.draw(fb, Face::Strong, 28.0, MARGIN, 620, INK, note);
        }
        return;
    };
    f.draw(fb, Face::Display, 44.0, MARGIN, 280, INK, &day_label(&e.day));
    let lines = edition_lines(f, e);
    let pages = edition_pages(f, e);
    let (a, b) = pages[page.min(pages.len() - 1)];
    let mut y = 330;
    for (text, st) in &lines[a..b] {
        let h = line_h(*st);
        match st {
            0 => f.draw(fb, Face::Display, 50.0, MARGIN, y + 48, INK, text),
            1 => {
                fb.fill(Rect::new(MARGIN, y + 22, W - 2 * MARGIN, 3), INK);
                f.draw(fb, Face::Display, 40.0, MARGIN, y + 72, INK, text)
            }
            _ => f.draw(fb, Face::Body, 32.0, MARGIN + 10, y + 34, INK, text),
        };
        y += h;
    }
    if pages.len() > 1 {
        if page > 0 {
            ui::button(fb, f, edition_nav(0), "‹ Anterior", false);
        }
        if page + 1 < pages.len() {
            ui::button(fb, f, edition_nav(1), "Próxima ›", true);
        }
        let t = format!("{} de {}", page + 1, pages.len());
        let w = f.measure(Face::Strong, 28.0, &t);
        f.draw(fb, Face::Strong, 28.0, (W - w) / 2, 1376, MUTED, &t);
    }
}

fn day_label(day: &str) -> String {
    let parts: Vec<&str> = day.split('-').collect();
    const MESES: [&str; 12] = ["janeiro", "fevereiro", "março", "abril", "maio", "junho", "julho", "agosto", "setembro", "outubro", "novembro", "dezembro"];
    match parts.as_slice() {
        [_, m, d] => format!("{} de {}", d.trim_start_matches('0'), MESES.get(m.parse::<usize>().unwrap_or(1).saturating_sub(1)).unwrap_or(&"")),
        _ => day.to_string(),
    }
}

// ---------- Caixa de entrada: artigos e fotos ----------

pub fn articles_dir() -> PathBuf {
    PathBuf::from("/mnt/us/documents/Artigos")
}

fn photos_dir() -> PathBuf {
    crate::home_dir().join("data/fotos")
}

/// Baixa o que chegou na caixa de entrada: artigos viram .txt na pasta de documentos (abrem no
/// leitor e aparecem na biblioteca), fotos viram PNG em tons de cinza do tamanho da tela.
/// Devolve (artigos novos, fotos novas).
pub fn sync_inbox() -> (usize, usize) {
    let Ok(j) = chat::call("GET", "/inbox", None, 15) else { return (0, 0) };
    let seen_path = data("caixa_vista.txt");
    let mut seen: Vec<String> = std::fs::read_to_string(&seen_path).unwrap_or_default().lines().map(str::to_string).collect();
    let (mut na, mut nf) = (0, 0);
    let _ = std::fs::create_dir_all(articles_dir());
    for a in j.get("artigos").map(Json::arr).unwrap_or(&[]) {
        let (Some(id), Some(title)) = (a.get("id").and_then(Json::str), a.get("title").and_then(Json::str)) else { continue };
        if seen.iter().any(|s| s == id) {
            continue;
        }
        let name: String = title.chars().filter(|c| !"/\\:*?\"<>|".contains(*c)).take(80).collect();
        let dest = articles_dir().join(format!("{}.txt", name.trim()));
        if chat::fetch(&format!("/inbox/artigo/{id}"), &dest, 30).is_ok() {
            seen.push(id.into());
            na += 1;
        }
    }
    let _ = std::fs::create_dir_all(photos_dir());
    for p in j.get("fotos").map(Json::arr).unwrap_or(&[]) {
        let Some(id) = p.get("id").and_then(Json::str) else { continue };
        if seen.iter().any(|s| s == id) {
            continue;
        }
        let raw = photos_dir().join(format!("{id}.orig"));
        if chat::fetch(&format!("/inbox/foto/{id}"), &raw, 60).is_ok() && prepare_photo(&raw, &photos_dir().join(format!("{id}.png"))) {
            seen.push(id.into());
            nf += 1;
        }
        let _ = std::fs::remove_file(&raw);
    }
    let _ = std::fs::write(&seen_path, seen.join("\n"));
    (na, nf)
}

/// Foto para o e-ink: tons de cinza, contraste ajustado e cortada para encher a tela.
fn prepare_photo(src: &std::path::Path, dest: &std::path::Path) -> bool {
    let Ok(img) = image::open(src) else { return false };
    let g = img.to_luma8();
    let (iw, ih) = g.dimensions();
    let (tw, th) = (1072u32, 1448u32);
    let s = (tw as f32 / iw as f32).max(th as f32 / ih as f32);
    let (rw, rh) = ((iw as f32 * s).ceil() as u32, (ih as f32 * s).ceil() as u32);
    let big = image::imageops::resize(&g, rw.max(tw), rh.max(th), image::imageops::FilterType::Triangle);
    let mut out = image::imageops::crop_imm(&big, (rw.max(tw) - tw) / 2, (rh.max(th) - th) / 2, tw, th).to_image();
    // Contraste: estica do 2º ao 98º percentil (fotos de celular ficam lavadas no e-ink).
    let mut hist = [0u32; 256];
    out.pixels().for_each(|p| hist[p[0] as usize] += 1);
    let total = (tw * th) as f32;
    let pct = |q: f32| {
        let mut acc = 0.0;
        for (v, &c) in hist.iter().enumerate() {
            acc += c as f32;
            if acc >= q * total {
                return v as f32;
            }
        }
        255.0
    };
    let (lo, hi) = (pct(0.02), pct(0.98).max(pct(0.02) + 1.0));
    out.pixels_mut().for_each(|p| p[0] = (((p[0] as f32 - lo) / (hi - lo)) * 255.0).clamp(0.0, 255.0) as u8);
    out.save(dest).is_ok()
}

/// Fotos do porta-retrato, da mais nova para a mais antiga.
pub fn photos() -> Vec<PathBuf> {
    let mut v: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(photos_dir())
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "png"))
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    v.sort_by(|a, b| b.0.cmp(&a.0));
    v.into_iter().map(|(_, p)| p).collect()
}

/// Porta-retrato: a foto ocupando a tela inteira. Sem fotos, explica como mandar.
pub fn draw_photo(fb: &mut Framebuffer, f: &Fonts, list: &[PathBuf], k: usize) {
    fb.fill(fb.screen(), PAPER);
    if let Some(p) = list.get(k % list.len().max(1)) {
        if let Ok(img) = image::open(p) {
            let g = img.to_luma8();
            fb.blit(0, 0, g.width() as i32, g.height() as i32, g.as_raw());
            return;
        }
    }
    ui::draw_back(fb, f, false);
    crate::sprites::draw(fb, "cab-mais", Rect::new(W - MARGIN - 250, 90, 260, 200));
    f.draw(fb, Face::Display, 96.0, MARGIN - 4, 380, INK, "Porta-retrato");
    let text = "Abra a página Mandar para o Kindle no celular, escolha uma foto e ela aparece aqui, em tons de cinza. Toque na foto para passar; ela também troca sozinha a cada 15 minutos.";
    for (n, l) in chat::wrap_words(f, Face::Body, 34.0, text, W - 2 * MARGIN).iter().enumerate() {
        f.draw(fb, Face::Body, 34.0, MARGIN, 470 + n as i32 * 48, INK, l);
    }
    send_hint(fb, f, 760);
}

fn send_hint(fb: &mut Framebuffer, f: &Fonts, y: i32) {
    if let Some(u) = send_url() {
        let r = Rect::new(MARGIN, y, W - 2 * MARGIN, 120);
        ui::card(fb, r, 28, ui::SURFACE, false);
        f.draw(fb, Face::Strong, 26.0, r.x + 28, r.y + 44, MUTED, "NO CELULAR, ABRA");
        f.draw(fb, Face::Strong, 30.0, r.x + 28, r.y + 90, INK, &crate::apps::fit(f, Face::Strong, 30.0, &u, r.w - 56));
    }
}

/// Endereço da página de envio (a mesma ponte, caminho /enviar).
pub fn send_url() -> Option<String> {
    let base = std::fs::read_to_string(crate::home_dir().join("settings/claude_url")).ok()?;
    Some(format!("{}/enviar", base.trim().trim_end_matches('/')))
}

/// Artigos guardados: (caminho, título, palavras), do mais novo para o mais antigo.
pub fn articles() -> Vec<(PathBuf, String, usize)> {
    let mut v: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(articles_dir())
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "txt"))
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    v.sort_by(|a, b| b.0.cmp(&a.0));
    v.into_iter()
        .map(|(_, p)| {
            let text = std::fs::read_to_string(&p).unwrap_or_default();
            let title = text.lines().next().unwrap_or("").to_string();
            (p, title, text.split_whitespace().count())
        })
        .collect()
}

pub const ART_ROWS: usize = 6;

pub fn article_row(k: usize) -> Rect {
    Rect::new(MARGIN, 470 + k as i32 * 128, W - 2 * MARGIN, 110)
}

pub fn draw_articles(fb: &mut Framebuffer, f: &Fonts, list: &[(PathBuf, String, usize)], page: usize, note: &str) {
    fb.fill(fb.screen(), PAPER);
    ui::draw_back(fb, f, false);
    crate::sprites::draw(fb, "cab-ler", Rect::new(W - MARGIN - 300, 96, 310, 240));
    f.draw(fb, Face::Display, 130.0, MARGIN - 6, 330, INK, "Artigos");
    let sub = match (note.is_empty(), list.len()) {
        (false, _) => note.to_string(),
        (true, 0) => "Nenhum artigo ainda".to_string(),
        (true, 1) => "1 artigo para ler".to_string(),
        (true, n) => format!("{n} artigos para ler"),
    };
    f.draw(fb, Face::Strong, 28.0, MARGIN, 400, MUTED, &crate::apps::fit(f, Face::Strong, 28.0, &sub, W - 2 * MARGIN));
    if list.is_empty() {
        let text = "Mande um link pela página Mandar para o Kindle e o texto limpo do artigo aparece aqui, para ler no leitor do Verso Ink.";
        for (n, l) in chat::wrap_words(f, Face::Body, 32.0, text, W - 2 * MARGIN).iter().enumerate() {
            f.draw(fb, Face::Body, 32.0, MARGIN, 480 + n as i32 * 44, INK, l);
        }
        send_hint(fb, f, 640);
        return;
    }
    for (k, (_, title, words)) in list.iter().skip(page * ART_ROWS).take(ART_ROWS).enumerate() {
        let r = article_row(k);
        ui::card(fb, r, 26, PAPER, false);
        f.draw(fb, Face::Display, 34.0, r.x + 28, r.y + 50, INK, &crate::apps::fit(f, Face::Display, 34.0, title, r.w - 56));
        let mins = (words / 230).max(1);
        f.draw(fb, Face::Body, 24.0, r.x + 30, r.y + 88, MUTED, &format!("{mins} min de leitura"));
    }
    let pages = list.len().div_ceil(ART_ROWS);
    if pages > 1 {
        if page > 0 {
            ui::button(fb, f, edition_nav(0), "‹ Anterior", false);
        }
        if page + 1 < pages {
            ui::button(fb, f, edition_nav(1), "Próxima ›", false);
        }
    }
}

// ---------- Terminal ----------

/// Comandos de um toque (sem digitar no e-ink).
pub const COMMANDS: [(&str, &str); 6] = [
    ("Espaço", "df -h /mnt/us /var/local 2>/dev/null"),
    ("Bateria", "cat /sys/class/power_supply/*/capacity /sys/class/power_supply/*/status 2>/dev/null; lipc-get-prop com.lab126.powerd status 2>/dev/null | head -5"),
    ("Rede", "ip -4 addr show wlan0 2>/dev/null | grep inet; iwconfig wlan0 2>/dev/null | grep -E 'ESSID|Quality'"),
    ("Memória", "free -m; uptime"),
    ("Registro", "tail -n 40 /tmp/verso-ink.log"),
    ("Processos", "top -b -n 1 | head -20"),
];

pub fn command_rect(k: usize) -> Rect {
    let w = (W - 2 * MARGIN - 2 * 16) / 3;
    Rect::new(MARGIN + (k % 3) as i32 * (w + 16), 200 + (k / 3) as i32 * 100, w, 82)
}

pub fn type_rect() -> Rect {
    Rect::new(MARGIN, 1330, W - 2 * MARGIN, 90)
}

/// Roda um comando no Linux do Kindle e devolve a saída (cortada).
pub fn run(cmd: &str) -> String {
    let out = std::process::Command::new("sh").args(["-c", &format!("{cmd} 2>&1")]).output();
    let mut s = match out {
        Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
        Err(e) => format!("não rodou: {e}"),
    };
    if s.len() > 6000 {
        s = s[s.len() - 6000..].to_string();
    }
    s
}

pub fn draw_terminal(fb: &mut Framebuffer, f: &Fonts, last: &str, output: &str) {
    fb.fill(fb.screen(), PAPER);
    ui::draw_back(fb, f, false);
    let tw = f.measure(Face::Strong, 34.0, "Terminal");
    f.draw(fb, Face::Strong, 34.0, W - MARGIN - tw, 128, INK, "Terminal");
    for (k, (label, _)) in COMMANDS.iter().enumerate() {
        ui::button(fb, f, command_rect(k), label, false);
    }
    let r = Rect::new(MARGIN - 16, 418, W - 2 * MARGIN + 32, 890);
    fb.round_rect(r, 24, 0x1A);
    let prompt = if last.is_empty() { "$ toque num comando ou digite o seu".to_string() } else { format!("$ {last}") };
    f.draw(fb, Face::Strong, 26.0, r.x + 24, r.y + 46, 0xDD, &crate::apps::fit(f, Face::Strong, 26.0, &prompt, r.w - 48));
    let mut y = r.y + 92;
    let lines: Vec<String> = output.lines().flat_map(|l| chat::wrap_words(f, Face::Body, 23.0, &l.replace('\t', "  "), r.w - 48).into_iter().chain(l.trim().is_empty().then(String::new))).collect();
    let fit = ((r.h - 110) / 31) as usize;
    let start = lines.len().saturating_sub(fit);
    for l in &lines[start..] {
        f.draw(fb, Face::Body, 23.0, r.x + 24, y, 0xF0, l);
        y += 31;
    }
    ui::button(fb, f, type_rect(), "Digitar um comando", true);
}
