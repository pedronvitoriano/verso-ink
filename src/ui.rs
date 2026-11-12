//! Telas do Verso Ink: início (relógio + 4 módulos), módulo e repouso.
//! Direção visual: tipografia grande, poucos elementos, muito respiro, alto contraste.

use crate::clock;
use crate::fb::{Framebuffer, Rect};
use crate::text::{Face, Fonts};

pub const INK: u8 = 0x00;
pub const PAPER: u8 = 0xFF;
pub const MUTED: u8 = 0x55;

const MARGIN: i32 = 72;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Module {
    Ler,
    Jogar,
    Foco,
    Claude,
    Mais,
}

impl Module {
    pub const ALL: [Module; 5] = [Module::Ler, Module::Jogar, Module::Foco, Module::Claude, Module::Mais];
    pub fn title(self) -> &'static str {
        match self {
            Module::Ler => "Ler",
            Module::Jogar => "Jogar",
            Module::Foco => "Foco",
            Module::Claude => "Claude",
            Module::Mais => "Mais",
        }
    }
    pub fn subtitle(self) -> &'static str {
        match self {
            Module::Ler => "Biblioteca e Verso One",
            Module::Jogar => "Xadrez, sudoku e mais 14",
            Module::Foco => "Pomodoro e o seu dia",
            Module::Claude => "Conversa ao vivo",
            Module::Mais => "Novos módulos",
        }
    }
    pub fn plan(self) -> [&'static str; 3] {
        match self {
            Module::Ler => [
                "Seus EPUB, PDF e quadrinhos,",
                "com o motor do Verso One",
                "e o que o Claude escrever para você.",
            ],
            Module::Jogar => [
                "Xadrez contra o Claude,",
                "sudoku e ficção interativa,",
                "tudo funcionando sem Wi-Fi.",
            ],
            Module::Foco => [
                "Pomodoro em tela cheia",
                "e as três prioridades do dia,",
                "sem celular por perto.",
            ],
            Module::Claude => [
                "Converse por toque,",
                "acompanhe as sessões ao vivo",
                "e receba textos para ler.",
            ],
            Module::Mais => ["", "", ""],
        }
    }
}


pub struct Status {
    pub battery: Option<u8>,
    pub charging: bool,
    pub wifi: bool,
    pub offset_min: i64,
}

// ---------- Geometria ----------

pub fn date_rect() -> Rect {
    Rect::new(MARGIN, 96, 1072 - 2 * MARGIN - 220, 60)
}
/// Faixa que muda a cada minuto (hora, clima e resumo): o que é redesenhado e atualizado.
pub fn clock_rect() -> Rect {
    Rect::new(0, 150, 1072, chips_rect().y + chips_rect().h + SH - 150)
}
/// Só a hora (à esquerda; à direita fica a ilustração do dia).
fn time_rect() -> Rect {
    Rect::new(MARGIN - 8, 150, 610, 300)
}
fn chips_rect() -> Rect {
    Rect::new(MARGIN, 466, 1072 - 2 * MARGIN, 84)
}
/// Bloco 0 (Ler, com a capa) ocupa a largura toda; os outros formam uma grade 2x2 abaixo.
pub fn tile_rect(i: usize) -> Rect {
    let (top, gap, wide_h, bottom) = (590, 26, 300, 1300);
    let full_w = 1072 - 2 * MARGIN;
    if i == 0 {
        return Rect::new(MARGIN, top, full_w, wide_h);
    }
    let grid_top = top + wide_h + gap + 4;
    let w = (full_w - gap) / 2;
    let h = (bottom - grid_top - gap) / 2;
    let k = (i - 1) as i32;
    let (col, row) = (k % 2, k / 2);
    Rect::new(MARGIN + col * (w + gap), grid_top + row * (h + gap), w, h)
}
/// Faixa do topo (data e bateria): tocar abre Luz e tela.
pub fn header_rect() -> Rect {
    Rect::new(0, 60, 1072, 120)
}
pub fn minus_rect() -> Rect {
    Rect::new(1072 - MARGIN - 2 * 168 - 28, 476, 168, 168)
}
pub fn plus_rect() -> Rect {
    Rect::new(1072 - MARGIN - 168, 476, 168, 168)
}
pub fn bar_rect() -> Rect {
    Rect::new(MARGIN, 712, 1072 - 2 * MARGIN, 96)
}
pub fn night_rect() -> Rect {
    Rect::new(MARGIN - 16, 900, 1072 - 2 * MARGIN + 32, 170)
}
pub fn settings_row_rect() -> Rect {
    Rect::new(MARGIN, 1220, 1072 - 2 * MARGIN, 84)
}
/// Wi-Fi direto no painel (trocar de rede sem passar pelos ajustes).
pub fn wifi_row_rect() -> Rect {
    Rect::new(MARGIN, 1114, 1072 - 2 * MARGIN, 84)
}
pub fn low_battery_ok_rect() -> Rect {
    Rect::new(MARGIN, 1150, 1072 - 2 * MARGIN, 120)
}
pub fn back_rect() -> Rect {
    Rect::new(MARGIN - 16, 72, 230, 88)
}
/// Canto do gesto de emergência no boot.
pub fn gate_corner() -> Rect {
    Rect::new(0, 0, 260, 260)
}

// ---------- Primitivas ----------

/// Sombra chapada embaixo de cartões e botões (o "3D" plano do sistema) e a sua altura.
pub const SHADOW: u8 = 0x2E;
pub const SH: i32 = 7;
/// Superfície cinza clara (cartões em destaque, fundo dos ícones).
pub const SURFACE: u8 = 0xE6;

/// Cartão do sistema: contorno firme e sombra chapada embaixo. `pressed` afunda o cartão na
/// sombra, como uma tecla.
pub fn card(fb: &mut Framebuffer, r: Rect, radius: i32, fill: u8, pressed: bool) {
    fb.fill(Rect::new(r.x, r.y, r.w, r.h + SH), PAPER);
    let r = if pressed { Rect::new(r.x, r.y + SH, r.w, r.h) } else { r };
    if !pressed {
        fb.round_rect(Rect::new(r.x, r.y + SH, r.w, r.h), radius, SHADOW);
    }
    fb.round_rect(r, radius, INK);
    fb.round_rect(Rect::new(r.x + 4, r.y + 4, r.w - 8, r.h - 8), (radius - 4).max(0), fill);
}

/// Botão do sistema: preto (principal) ou branco, com a sombra chapada.
pub fn button(fb: &mut Framebuffer, f: &Fonts, r: Rect, label: &str, filled: bool) {
    card(fb, r, (r.h / 2).min(40), if filled { INK } else { PAPER }, false);
    let fg = if filled { PAPER } else { INK };
    let mut size = 36.0f32.min(r.h as f32 * 0.42);
    while f.measure(Face::Strong, size, label) > r.w - 36 && size > 22.0 {
        size -= 2.0;
    }
    let tw = f.measure(Face::Strong, size, label);
    f.draw(fb, Face::Strong, size, r.x + (r.w - tw) / 2, r.y + r.h / 2 + (size * 0.36) as i32, fg, label);
}

/// Disco com o ícone ilustrado (assets/sprites/icone-NOME.png). Devolve false sem o desenho.
pub fn icon_disc(fb: &mut Framebuffer, x: i32, y: i32, d: i32, name: &str) -> bool {
    let k = d * 17 / 100;
    if !crate::sprites::exists(&format!("icone-{name}")) {
        return false;
    }
    fb.round_rect(Rect::new(x, y, d, d), d / 2, INK);
    fb.round_rect(Rect::new(x + 4, y + 4, d - 8, d - 8), d / 2 - 4, SURFACE);
    crate::sprites::draw(fb, &format!("icone-{name}"), Rect::new(x + k, y + k, d - 2 * k, d - 2 * k))
}

/// Segmento grosso com borda suavizada (distância ao segmento).
fn line(fb: &mut Framebuffer, (x0, y0): (f32, f32), (x1, y1): (f32, f32), t: f32, v: u8) {
    let (minx, maxx) = (x0.min(x1) - t, x0.max(x1) + t);
    let (miny, maxy) = (y0.min(y1) - t, y0.max(y1) + t);
    let (dx, dy) = (x1 - x0, y1 - y0);
    let len2 = (dx * dx + dy * dy).max(1e-6);
    for y in miny.floor() as i32..=maxy.ceil() as i32 {
        for x in minx.floor() as i32..=maxx.ceil() as i32 {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let u = (((px - x0) * dx + (py - y0) * dy) / len2).clamp(0.0, 1.0);
            let (qx, qy) = (x0 + u * dx - px, y0 + u * dy - py);
            let d = (qx * qx + qy * qy).sqrt() - t / 2.0;
            let a = (0.5 - d).clamp(0.0, 1.0);
            if a > 0.0 {
                fb.blend(x, y, v, (a * 255.0) as u8);
            }
        }
    }
}

fn ring(fb: &mut Framebuffer, cx: f32, cy: f32, r: f32, t: f32, v: u8) {
    let ext = (r + t) as i32 + 2;
    for y in cy as i32 - ext..=cy as i32 + ext {
        for x in cx as i32 - ext..=cx as i32 + ext {
            let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
            let a = (t / 2.0 + 0.5 - (d - r).abs()).clamp(0.0, 1.0);
            if a > 0.0 {
                fb.blend(x, y, v, (a * 255.0) as u8);
            }
        }
    }
}

fn disc(fb: &mut Framebuffer, cx: f32, cy: f32, r: f32, v: u8) {
    let ext = r as i32 + 2;
    for y in cy as i32 - ext..=cy as i32 + ext {
        for x in cx as i32 - ext..=cx as i32 + ext {
            let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
            let a = (r + 0.5 - d).clamp(0.0, 1.0);
            if a > 0.0 {
                fb.blend(x, y, v, (a * 255.0) as u8);
            }
        }
    }
}

/// Ícone geométrico de cada módulo, desenhado em um quadrado de lado `s`.
pub fn icon(fb: &mut Framebuffer, m: Module, x: i32, y: i32, s: i32, v: u8) {
    let (x, y, s) = (x as f32, y as f32, s as f32);
    let t = s * 0.075;
    match m {
        Module::Ler => {
            // Livro aberto: duas páginas e a lombada.
            let (top, bot, mid) = (y + s * 0.18, y + s * 0.86, x + s * 0.5);
            line(fb, (x + s * 0.06, top), (mid, top + s * 0.08), t, v);
            line(fb, (mid, top + s * 0.08), (x + s * 0.94, top), t, v);
            line(fb, (x + s * 0.06, bot - s * 0.08), (mid, bot), t, v);
            line(fb, (mid, bot), (x + s * 0.94, bot - s * 0.08), t, v);
            line(fb, (x + s * 0.06, top), (x + s * 0.06, bot - s * 0.08), t, v);
            line(fb, (x + s * 0.94, top), (x + s * 0.94, bot - s * 0.08), t, v);
            line(fb, (mid, top + s * 0.08), (mid, bot), t, v);
        }
        Module::Jogar => {
            // Tabuleiro 3x3 com casas alternadas.
            let c = s * 0.8 / 3.0;
            let ox = x + s * 0.1;
            let oy = y + s * 0.1;
            for i in 0..3 {
                for j in 0..3 {
                    if (i + j) % 2 == 0 {
                        fb.round_rect(
                            Rect::new((ox + i as f32 * c) as i32, (oy + j as f32 * c) as i32, c as i32, c as i32),
                            4,
                            v,
                        );
                    }
                }
            }
            let r = Rect::new(ox as i32, oy as i32, (c * 3.0) as i32, (c * 3.0) as i32);
            for k in 0..=3 {
                let o = k as f32 * c;
                line(fb, (r.x as f32, r.y as f32 + o), (r.x as f32 + c * 3.0, r.y as f32 + o), t * 0.45, v);
                line(fb, (r.x as f32 + o, r.y as f32), (r.x as f32 + o, r.y as f32 + c * 3.0), t * 0.45, v);
            }
        }
        Module::Foco => {
            // Mostrador com um ponteiro: o tempo dedicado.
            let (cx, cy, r) = (x + s * 0.5, y + s * 0.55, s * 0.38);
            ring(fb, cx, cy, r, t, v);
            line(fb, (cx, cy), (cx, cy - r * 0.62), t, v);
            line(fb, (cx, cy), (cx + r * 0.45, cy + r * 0.2), t, v);
            line(fb, (cx - s * 0.12, y + s * 0.06), (cx + s * 0.12, y + s * 0.06), t, v);
            disc(fb, cx, cy, t * 0.9, v);
        }
        Module::Mais => {
            // Sinal de mais dentro de um círculo: espaço para crescer.
            let (cx, cy, r) = (x + s * 0.5, y + s * 0.5, s * 0.42);
            ring(fb, cx, cy, r, t, v);
            line(fb, (cx - r * 0.5, cy), (cx + r * 0.5, cy), t, v);
            line(fb, (cx, cy - r * 0.5), (cx, cy + r * 0.5), t, v);
        }
        Module::Claude => {
            // Faísca: oito raios a partir do centro.
            let (cx, cy) = (x + s * 0.5, y + s * 0.5);
            for k in 0..8 {
                let ang = k as f32 * std::f32::consts::PI / 4.0;
                let r = if k % 2 == 0 { s * 0.44 } else { s * 0.3 };
                line(fb, (cx + ang.cos() * s * 0.08, cy + ang.sin() * s * 0.08), (cx + ang.cos() * r, cy + ang.sin() * r), t * 1.1, v);
            }
        }
    }
}

fn battery_glyph(fb: &mut Framebuffer, x: i32, y: i32, pct: u8, charging: bool) {
    let body = Rect::new(x, y, 64, 32);
    fb.round_outline(body, 8, 4, INK, PAPER);
    fb.round_rect(Rect::new(x + 66, y + 10, 6, 12), 2, INK);
    let w = ((56 * pct.min(100) as i32) / 100).max(4);
    fb.round_rect(Rect::new(x + 4 + 2, y + 6, w - 4, 20), 3, INK);
    if charging {
        // Raio vazado sobre a carga.
        let pts = [(x + 36, y + 3), (x + 24, y + 18), (x + 34, y + 18), (x + 28, y + 29)];
        for p in pts.windows(2) {
            line(fb, (p[0].0 as f32, p[0].1 as f32), (p[1].0 as f32, p[1].1 as f32), 5.0, PAPER);
            line(fb, (p[0].0 as f32, p[0].1 as f32), (p[1].0 as f32, p[1].1 as f32), 2.0, INK);
        }
    }
}

// ---------- Telas ----------

pub fn draw_header(fb: &mut Framebuffer, f: &Fonts, st: &Status) {
    let now = clock::now(st.offset_min);
    let r = date_rect();
    fb.fill(Rect::new(r.x, r.y, 1072 - 2 * MARGIN, r.h), PAPER);
    let date = if now.plausible() { now.date().to_uppercase() } else { "ACERTANDO O RELÓGIO".into() };
    f.draw(fb, Face::Strong, 30.0, r.x, r.y + 44, MUTED, &date);
    if let Some(p) = st.battery {
        let label = format!("{p}%");
        let lw = f.measure(Face::Strong, 30.0, &label);
        let gx = 1072 - MARGIN - 74;
        f.draw(fb, Face::Strong, 30.0, gx - 16 - lw, r.y + 44, INK, &label);
        battery_glyph(fb, gx, r.y + 18, p, st.charging);
    }
}

pub fn draw_clock(fb: &mut Framebuffer, f: &Fonts, st: &Status) {
    let now = clock::now(st.offset_min);
    let r = time_rect();
    fb.fill(r, PAPER);
    let t = if now.plausible() { now.time() } else { "--:--".into() };
    let mut size = 230.0;
    while f.measure(Face::Display, size, &t) > r.w && size > 150.0 {
        size -= 6.0;
    }
    f.draw(fb, Face::Display, size, MARGIN - 10, r.y + 236, INK, &t);
    draw_chips(fb, f, st);
}

/// Dois cartões abaixo da hora: o clima e o resumo do dia (lembrete, leitura, focos).
fn draw_chips(fb: &mut Framebuffer, f: &Fonts, st: &Status) {
    let c = chips_rect();
    fb.fill(Rect::new(c.x, c.y, c.w, c.h + SH), PAPER);
    let gap = 22;
    let weather = crate::weather::load();
    let w0 = if weather.is_some() { (c.w - gap) * 11 / 20 } else { 0 };
    if let Some(w) = weather {
        let r = Rect::new(c.x, c.y, w0, c.h);
        card(fb, r, 26, PAPER, false);
        let icon = if w.rain_pct >= 30 || matches!(w.code, 51..=99) { "chuva" } else { "sol" };
        let tx = if crate::sprites::draw(fb, &format!("icone-{icon}"), Rect::new(r.x + 14, r.y + 10, 62, 62)) { r.x + 88 } else { r.x + 24 };
        let line = if w.rain_pct >= 30 { format!("{}° · {}°/{}° · chuva {}%", w.temp, w.max, w.min, w.rain_pct) } else { format!("{}° · {}°/{}° · {}", w.temp, w.max, w.min, w.describe().to_lowercase()) };
        let line = crate::apps::fit(f, Face::Strong, 27.0, &line, r.x + r.w - 20 - tx);
        f.draw(fb, Face::Strong, 27.0, tx, r.y + 52, INK, &line);
    }
    let x1 = if w0 > 0 { c.x + w0 + gap } else { c.x };
    let r = Rect::new(x1, c.y, c.x + c.w - x1, c.h);
    card(fb, r, 26, PAPER, false);
    let line = crate::data::summary(st.offset_min).line.unwrap_or_else(|| "Um bom momento para ler".into());
    let tx = if crate::sprites::draw(fb, "icone-livro", Rect::new(r.x + 14, r.y + 10, 62, 62)) { r.x + 88 } else { r.x + 24 };
    let line = crate::apps::fit(f, Face::Strong, 27.0, &line, r.x + r.w - 20 - tx);
    f.draw(fb, Face::Strong, 27.0, tx, r.y + 52, INK, &line);
}

/// Ilustração do dia (muda à meia-noite): um personagem diferente a cada dia, ao lado da hora.
fn draw_daily_art(fb: &mut Framebuffer, st: &Status) {
    let day = crate::data::today(st.offset_min);
    let n = (0..40).filter(|k| crate::sprites::exists(&format!("dia-{:02}", k + 1))).count() as i64;
    if n == 0 {
        return;
    }
    let k = day.rem_euclid(n) + 1;
    crate::sprites::draw(fb, &format!("dia-{k:02}"), Rect::new(1072 - MARGIN - 360, 136, 372, 340));
}

/// Largura da capa no bloco Ler (proporção de livro, 2:3).
fn cover_w(r: Rect) -> i32 {
    (r.h - 50) * 2 / 3
}

/// Botão "Continuar" dentro do bloco Ler.
pub fn continue_rect() -> Rect {
    let r = tile_rect(0);
    let x = r.x + 36 + cover_w(r) + 40;
    Rect::new(x, r.y + r.h - 30 - 84, r.x + r.w - 36 - x, 84)
}

/// O que a tela inicial mostra além do fixo: livro em leitura e jogo em andamento.
#[derive(Default)]
pub struct HomeExtra {
    pub book: Option<crate::data::LastBook>,
    pub game: Option<&'static str>,
}

impl HomeExtra {
    pub fn load() -> Self {
        HomeExtra { book: crate::data::last_book(), game: crate::data::game_in_progress() }
    }
}

/// Capa do livro (do EPUB) ou, sem ela, uma capa desenhada com o título.
fn draw_cover(fb: &mut Framebuffer, f: &Fonts, r: Rect, book: Option<&crate::data::LastBook>) {
    fb.round_rect(Rect::new(r.x + SH, r.y + SH, r.w, r.h), 14, SHADOW);
    fb.round_rect(r, 14, INK);
    let inner = Rect::new(r.x + 4, r.y + 4, r.w - 8, r.h - 8);
    // Capa de verdade (EPUB, AZW3/MOBI), do mesmo cache da biblioteca.
    let (w, h) = (inner.w as u32, inner.h as u32);
    let thumb = book.and_then(|b| crate::covers::cached(&b.path, w, h).unwrap_or_else(|| crate::covers::generate(&b.path, w, h)));
    if let Some(t) = thumb {
        fb.fill(inner, PAPER);
        fb.blit(inner.x + (inner.w - t.w as i32) / 2, inner.y + (inner.h - t.h as i32) / 2, t.w as i32, t.h as i32, &t.px);
        return;
    }
    // Capa desenhada: lombada, título e um ornamento.
    fb.round_rect(inner, 10, 0xD4);
    fb.fill(Rect::new(inner.x, inner.y, 22, inner.h), 0x88);
    let full = book.map(|b| b.title.as_str()).unwrap_or("Biblioteca");
    let (title, author) = split_title(full);
    let cx = inner.x + 11 + inner.w / 2;
    fb.fill(Rect::new(cx - 50, inner.y + 30, 100, 4), INK);
    fb.fill(Rect::new(cx - 50, inner.y + inner.h - 34, 100, 4), INK);
    let lines = crate::chat::wrap_words(f, Face::Display, 32.0, title, inner.w - 56);
    let n = lines.len().min(4) as i32;
    let block = n * 40 + if author.is_empty() { 0 } else { 40 };
    let y0 = inner.y + (inner.h - block) / 2 + 30;
    for (k, l) in lines.iter().take(4).enumerate() {
        let w = f.measure(Face::Display, 32.0, l);
        f.draw(fb, Face::Display, 32.0, inner.x + 22 + (inner.w - 22 - w) / 2, y0 + k as i32 * 40, INK, l);
    }
    if !author.is_empty() {
        let a = crate::apps::fit(f, Face::Body, 22.0, author, inner.w - 50);
        let w = f.measure(Face::Body, 22.0, &a);
        f.draw(fb, Face::Body, 22.0, inner.x + 22 + (inner.w - 22 - w) / 2, y0 + n * 40 + 4, 0x44, &a);
    }
}

/// "Título - Autor PDF" → ("Título", "Autor"): nomes de arquivo viram título e autor.
fn split_title(full: &str) -> (&str, &str) {
    let full = full.trim();
    let full = full.strip_suffix(" PDF").or_else(|| full.strip_suffix(" EPUB")).unwrap_or(full);
    match full.split_once(" - ") {
        Some((t, a)) if !t.trim().is_empty() => (t.trim(), a.trim()),
        _ => (full, ""),
    }
}

pub fn draw_tile(fb: &mut Framebuffer, f: &Fonts, i: usize, pressed: bool, extra: &HomeExtra) {
    let m = Module::ALL[i];
    let r0 = tile_rect(i);
    card(fb, r0, 34, PAPER, pressed);
    let r = if pressed { Rect::new(r0.x, r0.y + SH, r0.w, r0.h) } else { r0 };
    if i == 0 {
        // Ler: a capa é a protagonista; título, progresso e Continuar ao lado.
        let cover = Rect::new(r.x + 36, r.y + 25, cover_w(r), r.h - 50);
        draw_cover(fb, f, cover, extra.book.as_ref());
        let x = r.x + 36 + cover_w(r) + 40;
        let tag = "Ler";
        let tw = f.measure(Face::Strong, 24.0, tag) + 28;
        fb.round_rect(Rect::new(x, r.y + 26, tw, 38), 19, SURFACE);
        f.draw(fb, Face::Strong, 24.0, x + 14, r.y + 53, INK, tag);
        let Some(book) = extra.book.as_ref() else {
            f.draw(fb, Face::Display, 46.0, x, r.y + 126, INK, "Biblioteca");
            for (k, l) in crate::chat::wrap_words(f, Face::Body, 27.0, "Seus livros e o Verso One, sem distração.", r.x + r.w - 36 - x).iter().take(2).enumerate() {
                f.draw(fb, Face::Body, 27.0, x, r.y + 170 + k as i32 * 36, MUTED, l);
            }
            return;
        };
        let (title, author) = split_title(&book.title);
        let room = r.x + r.w - 36 - x;
        // Título numa linha só (o bloco é baixo); o resto vai para a capa.
        let n = 1;
        f.draw(fb, Face::Display, 42.0, x, r.y + 114, INK, &crate::apps::fit(f, Face::Display, 42.0, title, room));
        // Autor e progresso numa linha só, para o botão nunca cobrir nada.
        let pct = book.progress.map(|p| format!("{}% lido", (p * 100.0).round() as i32)).unwrap_or_else(|| "ainda não começou".into());
        let meta = if author.is_empty() { pct } else { format!("{author} · {pct}") };
        let py = r.y + 114 + n * 40;
        f.draw(fb, Face::Body, 27.0, x, py, MUTED, &crate::apps::fit(f, Face::Body, 27.0, &meta, room));
        if let Some(p) = book.progress {
            let bar = Rect::new(x, py + 16, room, 14);
            fb.round_rect(bar, 8, SURFACE);
            fb.round_outline(bar, 8, 2, INK, SURFACE);
            fb.round_rect(Rect::new(bar.x, bar.y, ((bar.w as f32 * p.clamp(0.0, 1.0)) as i32).max(16), bar.h), 8, 0x55);
        }
        let c = continue_rect();
        let c = if pressed { Rect::new(c.x, c.y + SH, c.w, c.h) } else { c };
        button(fb, f, c, if book.progress.is_some_and(|p| p > 0.0) { "Continuar" } else { "Começar a ler" }, true);
    } else {
        let d = 100;
        let has = icon_disc(fb, r.x + 22, r.y + (r.h - d) / 2, d, match m {
            Module::Jogar => "jogar",
            Module::Foco => "foco",
            Module::Claude => "claude",
            Module::Mais => "mais",
            Module::Ler => "ler",
        });
        if !has {
            icon(fb, m, r.x + 36, r.y + (r.h - 66) / 2, 66, INK);
        }
        let x = r.x + 22 + d + 22;
        fb.fill(Rect::new(x - 12, r.y + 26, 3, r.h - 52), 0xBB);
        f.draw(fb, Face::Display, 50.0, x + 4, r.y + r.h / 2 + 4, INK, m.title());
        let text = if m == Module::Jogar { extra.game.unwrap_or(m.subtitle()) } else { m.subtitle() };
        let text = crate::apps::fit(f, Face::Body, 24.0, text, r.x + r.w - 16 - x);
        f.draw(fb, Face::Body, 24.0, x + 6, r.y + r.h / 2 + 42, if m == Module::Jogar && extra.game.is_some() { INK } else { MUTED }, &text);
    }
}

pub fn draw_footer(fb: &mut Framebuffer, f: &Fonts, st: &Status) {
    let y = 1324;
    fb.fill(Rect::new(0, y, 1072, 1448 - y), PAPER);
    let end = f.draw(fb, Face::Display, 34.0, MARGIN, 1394, INK, "Verso Ink");
    if let Some(v) = crate::backup::update_available() {
        // Versão nova encontrada: avisa aqui; instalar fica no Ajustes.
        f.draw(fb, Face::Strong, 28.0, end + 20, 1394, INK, &format!("· versão {v} no Ajustes"));
    }
    let wifi = if st.wifi { "Wi-Fi conectado" } else { "Sem Wi-Fi" };
    let w = f.measure(Face::Strong, 26.0, wifi);
    let has = crate::sprites::exists("icone-wifi");
    let cw = w + 48 + if has { 46 } else { 0 };
    let r = Rect::new(1072 - MARGIN - cw, 1350, cw, 62);
    fb.round_rect(r, 31, INK);
    fb.round_rect(Rect::new(r.x + 3, r.y + 3, r.w - 6, r.h - 6), 28, PAPER);
    let mut tx = r.x + 24;
    if has {
        crate::sprites::draw(fb, "icone-wifi", Rect::new(r.x + 18, r.y + 12, 38, 38));
        tx += 46;
    }
    f.draw(fb, Face::Strong, 26.0, tx, r.y + 41, if st.wifi { INK } else { MUTED }, wifi);
}

pub fn draw_home(fb: &mut Framebuffer, f: &Fonts, st: &Status, sum: &crate::data::Summary) {
    fb.fill(fb.screen(), PAPER);
    draw_header(fb, f, st);
    draw_daily_art(fb, st);
    draw_clock(fb, f, st);
    let _ = sum; // o resumo é desenhado nos cartões abaixo da hora (draw_chips)
    let extra = HomeExtra::load();
    for i in 0..Module::ALL.len() {
        draw_tile(fb, f, i, false, &extra);
    }
    draw_footer(fb, f, st);
}

pub fn draw_back(fb: &mut Framebuffer, f: &Fonts, pressed: bool) {
    let r0 = back_rect();
    card(fb, r0, r0.h / 2, if pressed { INK } else { PAPER }, pressed);
    let fg = if pressed { PAPER } else { INK };
    let r = if pressed { Rect::new(r0.x, r0.y + SH, r0.w, r0.h) } else { r0 };
    line(fb, (r.x as f32 + 54.0, r.y as f32 + 28.0), (r.x as f32 + 38.0, r.y as f32 + 44.0), 6.0, fg);
    line(fb, (r.x as f32 + 38.0, r.y as f32 + 44.0), (r.x as f32 + 54.0, r.y as f32 + 60.0), 6.0, fg);
    f.draw(fb, Face::Strong, 36.0, r.x + 76, r.y + 57, fg, "Início");
}

/// Ideias de módulos futuros (da conversa de concepção); entram aqui conforme forem feitos.
/// Módulos do Mais: (nome, descrição, pronto?). Os prontos abrem ao toque.
const COMING: [(&str, &str, bool); 4] = [
    ("Receitas", "Aberta na cozinha, sem apagar", true),
    ("Artigos", "Ler depois, sem distração", false),
    ("Terminal", "O Linux do Kindle na tela", false),
    ("Porta-retrato", "Fotos em tons de cinza", false),
];

pub fn more_row(k: usize) -> Rect {
    Rect::new(MARGIN, 540 + k as i32 * 172, 1072 - 2 * MARGIN, 148)
}

fn draw_more(fb: &mut Framebuffer, f: &Fonts, st: &Status) {
    fb.fill(fb.screen(), PAPER);
    draw_back(fb, f, false);
    f.draw(fb, Face::Display, 150.0, MARGIN - 6, 370, INK, "Mais");
    f.draw(fb, Face::Strong, 34.0, MARGIN, 446, MUTED, "Módulos extras, e os que vêm por aí");
    for (k, (name, desc, ready)) in COMING.iter().enumerate() {
        // Pronto: cartão branco com sombra (dá para tocar). Em breve: superfície cinza, sem sombra.
        let r = more_row(k);
        if *ready {
            card(fb, r, 30, PAPER, false);
        } else {
            fb.round_rect(r, 30, SURFACE);
        }
        let ink = if *ready { INK } else { 0x55 };
        f.draw(fb, Face::Display, 54.0, r.x + 34, r.y + 70, ink, name);
        f.draw(fb, Face::Body, 30.0, r.x + 36, r.y + 116, MUTED, desc);
        let tag = if *ready { "Abrir" } else { "Em breve" };
        let tw = f.measure(Face::Strong, 26.0, tag);
        let tr = Rect::new(r.x + r.w - tw - 48 - 28, r.y + (r.h - 56) / 2, tw + 48, 56);
        if *ready {
            fb.round_rect(tr, 28, INK);
        } else {
            fb.round_outline(tr, 28, 3, 0x88, SURFACE);
        }
        f.draw(fb, Face::Strong, 26.0, tr.x + 24, tr.y + 38, if *ready { PAPER } else { 0x55 }, tag);
    }
    draw_footer(fb, f, st);
}

pub fn draw_module(fb: &mut Framebuffer, f: &Fonts, m: Module, st: &Status) {
    if m == Module::Mais {
        return draw_more(fb, f, st);
    }
    fb.fill(fb.screen(), PAPER);
    draw_back(fb, f, false);
    icon(fb, m, MARGIN, 260, 200, INK);
    f.draw(fb, Face::Display, 180.0, MARGIN - 6, 660, INK, m.title());
    f.draw(fb, Face::Strong, 40.0, MARGIN, 740, MUTED, m.subtitle());
    for (k, l) in m.plan().iter().enumerate() {
        f.draw(fb, Face::Body, 44.0, MARGIN, 880 + k as i32 * 64, INK, l);
    }
    // Selo de estado: honesto sobre o que ainda não existe.
    let tag = "EM CONSTRUÇÃO";
    let tw = f.measure(Face::Strong, 28.0, tag);
    let tr = Rect::new(MARGIN, 1120, tw + 56, 64);
    fb.round_outline(tr, 32, 3, INK, PAPER);
    f.draw(fb, Face::Strong, 28.0, tr.x + 28, tr.y + 43, INK, tag);
    draw_footer(fb, f, st);
}

/// Tela de repouso: fica fixa enquanto o Kindle dorme (o e-ink não gasta energia para mantê-la).
/// Clara de propósito: escura parecia aparelho apagado. Mostra o livro em andamento, se houver.
pub fn draw_sleep(fb: &mut Framebuffer, f: &Fonts, st: &Status, book: Option<&crate::data::LastBook>) {
    fb.fill(fb.screen(), PAPER);
    f.draw(fb, Face::Display, 40.0, MARGIN, 150, INK, "Verso Ink");
    icon_moon(fb, MARGIN, 300, 130);
    f.draw(fb, Face::Display, 150.0, MARGIN - 6, 600, INK, "Em repouso");
    f.draw(fb, Face::Body, 42.0, MARGIN, 690, INK, "Aperte o botão de ligar para acordar.");
    if let Some(b) = book {
        fb.fill(Rect::new(MARGIN, 820, 1072 - 2 * MARGIN, 3), INK);
        f.draw(fb, Face::Strong, 32.0, MARGIN, 900, MUTED, "LENDO");
        let title = crate::apps::fit(f, Face::Display, 72.0, &b.title, 1072 - 2 * MARGIN);
        f.draw(fb, Face::Display, 72.0, MARGIN - 4, 990, INK, &title);
        if let Some(p) = b.progress {
            let pct = (p * 100.0).round() as i32;
            let bar = Rect::new(MARGIN, 1040, 1072 - 2 * MARGIN, 28);
            fb.round_outline(bar, 14, 4, INK, PAPER);
            let w = ((bar.w - 12) * pct.clamp(0, 100)) / 100;
            if w > 16 {
                fb.round_rect(Rect::new(bar.x + 6, bar.y + 6, w, bar.h - 12), 8, INK);
            }
            f.draw(fb, Face::Body, 34.0, MARGIN, 1120, INK, &format!("{pct}% lido"));
        }
    }
    let now = clock::now(st.offset_min);
    let since = if now.plausible() { format!("Dormiu às {} · {}", now.time(), now.date()) } else { "Dormindo".into() };
    fb.fill(Rect::new(MARGIN, 1280, 1072 - 2 * MARGIN, 3), INK);
    f.draw(fb, Face::Strong, 30.0, MARGIN, 1350, MUTED, &since);
    if let Some(p) = st.battery {
        let label = format!("{p}%");
        let lw = f.measure(Face::Strong, 30.0, &label);
        let gx = 1072 - MARGIN - 74;
        f.draw(fb, Face::Strong, 30.0, gx - 16 - lw, 1350, INK, &label);
        battery_glyph(fb, gx, 1324, p, st.charging);
    }
}

pub fn draw_low_battery(fb: &mut Framebuffer, f: &Fonts, st: &Status) {
    fb.fill(fb.screen(), PAPER);
    f.draw(fb, Face::Display, 40.0, MARGIN, 150, INK, "Verso Ink");
    let p = st.battery.unwrap_or(0);
    // Pilha grande quase vazia.
    let body = Rect::new(MARGIN, 300, 360, 170);
    fb.round_outline(body, 28, 12, INK, PAPER);
    fb.round_rect(Rect::new(body.x + body.w + 6, body.y + 52, 26, 66), 8, INK);
    fb.round_rect(Rect::new(body.x + 26, body.y + 26, ((308 * p as i32) / 100).max(20), body.h - 52), 10, INK);
    f.draw(fb, Face::Display, 150.0, MARGIN - 6, 720, INK, "Bateria fraca");
    f.draw(fb, Face::Display, 96.0, MARGIN - 4, 840, INK, &format!("{p}%"));
    f.draw(fb, Face::Body, 42.0, MARGIN, 940, INK, "Ligue o Kindle no carregador.");
    f.draw(fb, Face::Body, 34.0, MARGIN, 1000, MUTED, "Se a carga acabar, ele se desliga sozinho.");
    let r = low_battery_ok_rect();
    fb.round_rect(r, r.h / 2, INK);
    let tw = f.measure(Face::Strong, 44.0, "Entendi");
    f.draw(fb, Face::Strong, 44.0, r.x + (r.w - tw) / 2, r.y + 76, PAPER, "Entendi");
}

/// Lua crescente: dois discos, o segundo "recorta" o primeiro.
fn icon_moon(fb: &mut Framebuffer, x: i32, y: i32, s: i32) {
    let (x, y, s) = (x as f32, y as f32, s as f32);
    disc(fb, x + s * 0.5, y + s * 0.5, s * 0.46, INK);
    disc(fb, x + s * 0.72, y + s * 0.34, s * 0.4, PAPER);
}

/// Mostrada enquanto a interface da Amazon sobe (leva cerca de um minuto).
pub fn draw_amazon_splash(fb: &mut Framebuffer, f: &Fonts) {
    fb.fill(fb.screen(), PAPER);
    f.draw(fb, Face::Display, 40.0, MARGIN, 150, INK, "Verso Ink");
    f.draw(fb, Face::Display, 96.0, MARGIN - 4, 700, INK, "Abrindo o Kindle");
    f.draw(fb, Face::Display, 96.0, MARGIN - 4, 800, INK, "original");
    f.draw(fb, Face::Body, 40.0, MARGIN, 890, MUTED, "Isso leva cerca de um minuto.");
}

pub fn draw_gate_hint(fb: &mut Framebuffer, f: &Fonts) {
    fb.fill(fb.screen(), PAPER);
    let title = "Verso Ink";
    let w = f.measure(Face::Display, 140.0, title);
    f.draw(fb, Face::Display, 140.0, (1072 - w) / 2, 740, INK, title);
    let l1 = "Abrindo…";
    let w1 = f.measure(Face::Strong, 38.0, l1);
    f.draw(fb, Face::Strong, 38.0, (1072 - w1) / 2, 830, MUTED, l1);
    // O canto do gesto de emergência fica marcado com discrição.
    let c = gate_corner();
    line(fb, (c.x as f32 + 48.0, c.y as f32 + 48.0), (c.x as f32 + 128.0, c.y as f32 + 48.0), 5.0, MUTED);
    line(fb, (c.x as f32 + 48.0, c.y as f32 + 48.0), (c.x as f32 + 48.0, c.y as f32 + 128.0), 5.0, MUTED);
    let hint = "Para abrir o Kindle original, segure o canto marcado";
    let hw = f.measure(Face::Body, 28.0, hint);
    f.draw(fb, Face::Body, 28.0, (1072 - hw) / 2, 1360, MUTED, hint);
}

// ---------- Luz e tela ----------

pub struct Light {
    pub level: i32,
    pub max: i32,
    pub night: bool,
}

fn round_button(fb: &mut Framebuffer, r: Rect, pressed: bool, plus: bool) {
    // Tecla redonda com sombra chapada; tocada, afunda na sombra.
    let d = r.w - 10;
    let k = Rect::new(r.x + 5, r.y + 2, d, d);
    card(fb, k, d / 2, if pressed { INK } else { PAPER }, pressed);
    let fg = if pressed { PAPER } else { INK };
    let (cx, cy) = (k.x as f32 + d as f32 / 2.0, k.y as f32 + d as f32 / 2.0 + if pressed { SH as f32 } else { 0.0 });
    let arm = d as f32 * 0.2;
    line(fb, (cx - arm, cy), (cx + arm, cy), 9.0, fg);
    if plus {
        line(fb, (cx, cy - arm), (cx, cy + arm), 9.0, fg);
    }
}

pub fn draw_light_minus(fb: &mut Framebuffer, pressed: bool) {
    round_button(fb, minus_rect(), pressed, false);
}
pub fn draw_light_plus(fb: &mut Framebuffer, pressed: bool) {
    round_button(fb, plus_rect(), pressed, true);
}

/// Nível da luz: número grande e uma barra de segmentos que dá para tocar.
pub fn draw_light_level(fb: &mut Framebuffer, f: &Fonts, l: &Light) {
    fb.fill(Rect::new(MARGIN - 8, 440, 560, 230), PAPER);
    f.draw(fb, Face::Strong, 36.0, MARGIN, 470, MUTED, "LUZ");
    let label = if l.level == 0 { "Apagada".to_string() } else { l.level.to_string() };
    let end = f.draw(fb, Face::Display, if l.level == 0 { 120.0 } else { 170.0 }, MARGIN - 6, 640, INK, &label);
    if l.level > 0 {
        f.draw(fb, Face::Body, 36.0, end + 16, 640, MUTED, &format!("de {}", l.max));
    }
    // Trilho único com a parte acesa preenchida e marcas a cada passo (toque em qualquer ponto).
    let r = bar_rect();
    card(fb, r, r.h / 2, SURFACE, false);
    let n = l.max.max(1);
    let inner = Rect::new(r.x + 10, r.y + 10, r.w - 20, r.h - 20);
    let fill_w = inner.w * l.level / n;
    if fill_w > 0 {
        fb.round_rect(Rect::new(inner.x, inner.y, fill_w.max(inner.h), inner.h), inner.h / 2, INK);
    }
    for i in 1..n {
        let x = inner.x + inner.w * i / n;
        let v = if i <= l.level { 0x88 } else { 0xB8 };
        fb.fill(Rect::new(x - 1, inner.y + inner.h / 2 - 8, 3, 16), v);
    }
}

pub fn draw_night(fb: &mut Framebuffer, f: &Fonts, l: &Light, pressed: bool) {
    let r = night_rect();
    fb.fill(r, PAPER);
    if pressed {
        fb.round_rect(r, 32, 0xDD);
    }
    f.draw(fb, Face::Display, 64.0, MARGIN, r.y + 82, INK, "Modo noturno");
    f.draw(fb, Face::Body, 32.0, MARGIN, r.y + 134, MUTED, "Texto claro sobre fundo escuro");
    // Chave liga/desliga.
    let sw = Rect::new(1072 - MARGIN - 180, r.y + 40, 180, 96);
    if l.night {
        fb.round_rect(sw, 48, INK);
        disc(fb, sw.x as f32 + sw.w as f32 - 48.0, sw.y as f32 + 48.0, 36.0, PAPER);
    } else {
        fb.round_outline(sw, 48, 5, INK, if pressed { 0xDD } else { PAPER });
        disc(fb, sw.x as f32 + 48.0, sw.y as f32 + 48.0, 30.0, INK);
    }
}

pub fn draw_light(fb: &mut Framebuffer, f: &Fonts, l: &Light, st: &Status) {
    fb.fill(fb.screen(), PAPER);
    draw_back(fb, f, false);
    f.draw(fb, Face::Display, 120.0, MARGIN - 4, 360, INK, "Luz e tela");
    draw_light_level(fb, f, l);
    draw_light_minus(fb, false);
    draw_light_plus(fb, false);
    fb.fill(Rect::new(MARGIN, 862, 1072 - 2 * MARGIN, 3), INK);
    draw_night(fb, f, l, false);
    let rows = [
        (wifi_row_rect(), "Wi-Fi", if st.wifi { "Conectado · trocar de rede" } else { "Sem rede · conectar" }),
        (settings_row_rect(), "Ajustes do sistema", "Bateria, cópia, reiniciar"),
    ];
    for ((r, title, sub), icon_name) in rows.into_iter().zip(["wifi", "luz"]) {
        card(fb, r, 28, PAPER, false);
        let d = r.h - 20;
        let tx = if icon_disc(fb, r.x + 12, r.y + 10, d, icon_name) { r.x + 12 + d + 20 } else { r.x + 28 };
        let end = f.draw(fb, Face::Display, 40.0, tx, r.y + 58, INK, title);
        let (cx, cy) = (r.x + r.w - 36, r.y + r.h / 2);
        let room = cx - 40 - (end + 20);
        f.draw(fb, Face::Body, 26.0, end + 20, r.y + 56, MUTED, &crate::apps::fit(f, Face::Body, 26.0, sub, room));
        line(fb, ((cx - 12) as f32, (cy - 18) as f32), (cx as f32, cy as f32), 6.0, INK);
        line(fb, (cx as f32, cy as f32), ((cx - 12) as f32, (cy + 18) as f32), 6.0, INK);
    }
    draw_footer(fb, f, st);
}

/// Traço com pontas arredondadas (para ícones fora deste módulo).
pub fn stroke(fb: &mut Framebuffer, a: (f32, f32), b: (f32, f32), t: f32, v: u8) {
    line(fb, a, b, t, v);
}

/// Sino de lembrete com topo em (cx, top).
pub fn bell(fb: &mut Framebuffer, cx: f32, top: f32) {
    disc(fb, cx, top + 70.0, 62.0, INK);
    fb.fill(Rect::new((cx - 62.0) as i32, (top + 70.0) as i32, 124, 60), INK);
    fb.round_rect(Rect::new((cx - 86.0) as i32, (top + 122.0) as i32, 172, 26), 13, INK);
    disc(fb, cx, top + 166.0, 20.0, INK);
    disc(fb, cx, top + 4.0, 12.0, INK);
}


/// Cabeçalho das telas de digitar: rótulo pequeno em cima e título grande.
pub fn draw_field_header(fb: &mut Framebuffer, f: &Fonts, kicker: &str, title: &str) {
    fb.fill(fb.screen(), PAPER);
    draw_back(fb, f, false);
    f.draw(fb, Face::Strong, 32.0, MARGIN, 250, MUTED, kicker);
    f.draw(fb, Face::Display, 64.0, MARGIN, 340, INK, title);
}
