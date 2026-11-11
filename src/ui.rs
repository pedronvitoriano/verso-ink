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
            Module::Jogar => "Xadrez, sudoku e mais 11",
            Module::Foco => "Pomodoro e o seu dia",
            Module::Claude => "Conversa e janela ao vivo",
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
pub fn clock_rect() -> Rect {
    Rect::new(MARGIN - 8, 170, 1072 - 2 * MARGIN + 16, 250)
}
/// Bloco 0 (Ler) ocupa a largura toda; os outros formam uma grade 2x2 abaixo.
pub fn tile_rect(i: usize) -> Rect {
    let (top, gap, wide_h, bottom) = (480, 28, 270, 1290);
    let full_w = 1072 - 2 * MARGIN;
    if i == 0 {
        return Rect::new(MARGIN, top, full_w, wide_h);
    }
    let grid_top = top + wide_h + gap;
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
    Rect::new(MARGIN - 16, 1120, 1072 - 2 * MARGIN + 32, 150)
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
fn icon(fb: &mut Framebuffer, m: Module, x: i32, y: i32, s: i32, v: u8) {
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
    let r = clock_rect();
    fb.fill(r, PAPER);
    let t = if now.plausible() { now.time() } else { "--:--".into() };
    f.draw(fb, Face::Display, 236.0, MARGIN - 10, r.y + 200, INK, &t);
    draw_weather(fb, f);
    // Resumo do dia (próximo lembrete, sequências, leitura, focos): redesenhado a cada minuto.
    if let Some(line) = crate::data::summary(st.offset_min).line {
        let line = crate::apps::fit(f, Face::Strong, 28.0, &line, 1072 - 2 * MARGIN);
        f.draw(fb, Face::Strong, 28.0, MARGIN, 414, INK, &line);
    }
}

/// Botão "Continuar" dentro do bloco Ler (abre o último livro direto no KOReader).
pub fn continue_rect() -> Rect {
    let r = tile_rect(0);
    Rect::new(r.x + r.w - 40 - 300, r.y + 40, 300, 100)
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

pub fn draw_tile(fb: &mut Framebuffer, f: &Fonts, i: usize, pressed: bool, extra: &HomeExtra) {
    let m = Module::ALL[i];
    let r = tile_rect(i);
    // "Ler" é o módulo principal: bloco cheio. Os outros, contorno.
    let filled = (i == 0) ^ pressed;
    let (fg, sub) = if filled { (PAPER, 0xCC) } else { (INK, MUTED) };
    fb.fill(r, PAPER);
    if filled {
        fb.round_rect(r, 40, INK);
    } else {
        fb.round_outline(r, 40, 5, INK, PAPER);
    }
    if i == 0 {
        f.draw(fb, Face::Display, 104.0, r.x + 44, r.y + r.h - 100, fg, m.title());
        let Some(book) = extra.book.as_ref() else {
            icon(fb, m, r.x + r.w - 44 - 128, r.y + (r.h - 128) / 2, 128, fg);
            f.draw(fb, Face::Body, 31.0, r.x + 48, r.y + r.h - 46, sub, m.subtitle());
            return;
        };
        // Livro em leitura: título à esquerda, botão Continuar e porcentagem à direita, barra no pé.
        let c = continue_rect();
        let (bg, ink) = if filled { (PAPER, INK) } else { (INK, PAPER) };
        fb.round_rect(c, c.h / 2, bg);
        let label = "Continuar";
        let tw = f.measure(Face::Strong, 38.0, label);
        f.draw(fb, Face::Strong, 38.0, c.x + (c.w - tw) / 2, c.y + c.h / 2 + 14, ink, label);
        let pct = book.progress.map(|p| format!("{}% lido", (p * 100.0).round() as i32)).unwrap_or_else(|| "Começar".into());
        let pw = f.measure(Face::Body, 31.0, &pct);
        f.draw(fb, Face::Body, 31.0, c.x + c.w - pw, r.y + r.h - 46, sub, &pct);
        let title = crate::apps::fit(f, Face::Body, 31.0, &book.title, c.x + c.w - pw - 40 - (r.x + 48));
        f.draw(fb, Face::Body, 31.0, r.x + 48, r.y + r.h - 46, fg, &title);
        if let Some(p) = book.progress {
            let bar = Rect::new(r.x + 48, r.y + r.h - 24, r.w - 96, 6);
            fb.fill(bar, if filled { 0x55 } else { 0xCC });
            fb.fill(Rect::new(bar.x, bar.y, (bar.w as f32 * p.clamp(0.0, 1.0)) as i32, bar.h), fg);
        }
    } else {
        icon(fb, m, r.x + 36, r.y + 34, 66, fg);
        f.draw(fb, Face::Display, 72.0, r.x + 34, r.y + r.h - 72, fg, m.title());
        let text = if m == Module::Jogar { extra.game.unwrap_or(m.subtitle()) } else { m.subtitle() };
        if m == Module::Jogar && extra.game.is_some() {
            // Ponto indicando partida salva.
            fb.round_rect(Rect::new(r.x + 37, r.y + r.h - 50, 14, 14), 7, fg);
            f.draw(fb, Face::Body, 26.0, r.x + 61, r.y + r.h - 32, fg, text);
        } else {
            f.draw(fb, Face::Body, 26.0, r.x + 37, r.y + r.h - 32, sub, text);
        }
    }
}

pub fn draw_footer(fb: &mut Framebuffer, f: &Fonts, st: &Status) {
    let y = 1290 + 40;
    fb.fill(Rect::new(0, y, 1072, 1448 - y), PAPER);
    f.draw(fb, Face::Display, 34.0, MARGIN, 1384, INK, "Verso Ink");
    let wifi = if st.wifi { "Wi-Fi conectado" } else { "Sem Wi-Fi" };
    let w = f.measure(Face::Body, 28.0, wifi);
    f.draw(fb, Face::Body, 28.0, 1072 - MARGIN - w, 1384, MUTED, wifi);
}

pub fn draw_home(fb: &mut Framebuffer, f: &Fonts, st: &Status, sum: &crate::data::Summary) {
    fb.fill(fb.screen(), PAPER);
    draw_header(fb, f, st);
    draw_clock(fb, f, st);
    let _ = sum; // o resumo é desenhado junto com o relógio (draw_clock)
    fb.fill(Rect::new(MARGIN, 432, 1072 - 2 * MARGIN, 4), INK);
    let extra = HomeExtra::load();
    for i in 0..Module::ALL.len() {
        draw_tile(fb, f, i, false, &extra);
    }
    draw_footer(fb, f, st);
}

pub fn draw_back(fb: &mut Framebuffer, f: &Fonts, pressed: bool) {
    let r = back_rect();
    fb.fill(r, PAPER);
    let (bg, fg) = if pressed { (INK, PAPER) } else { (PAPER, INK) };
    if pressed {
        fb.round_rect(r, 44, bg);
    } else {
        fb.round_outline(r, 44, 4, INK, PAPER);
    }
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
    Rect::new(MARGIN - 16, 560 + k as i32 * 168, 1072 - 2 * MARGIN + 32, 164)
}

fn draw_more(fb: &mut Framebuffer, f: &Fonts, st: &Status) {
    fb.fill(fb.screen(), PAPER);
    draw_back(fb, f, false);
    f.draw(fb, Face::Display, 180.0, MARGIN - 6, 400, INK, "Mais");
    f.draw(fb, Face::Strong, 40.0, MARGIN, 480, MUTED, "Módulos extras, e os que vêm por aí");
    for (k, (name, desc, ready)) in COMING.iter().enumerate() {
        let tag = if *ready { "ABRIR" } else { "EM BREVE" };
        let tw = f.measure(Face::Strong, 24.0, tag);
        let y = 560 + k as i32 * 168;
        fb.fill(Rect::new(MARGIN, y, 1072 - 2 * MARGIN, 3), INK);
        f.draw(fb, Face::Display, 60.0, MARGIN, y + 86, INK, name);
        f.draw(fb, Face::Body, 32.0, MARGIN + 2, y + 136, MUTED, desc);
        let tr = Rect::new(1072 - MARGIN - tw - 44, y + 42, tw + 44, 52);
        if *ready {
            fb.round_rect(tr, 26, INK);
        } else {
            fb.round_outline(tr, 26, 3, INK, PAPER);
        }
        f.draw(fb, Face::Strong, 24.0, tr.x + 22, tr.y + 35, if *ready { PAPER } else { INK }, tag);
    }
    fb.fill(Rect::new(MARGIN, 560 + 4 * 168, 1072 - 2 * MARGIN, 3), INK);
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
    let (cx, cy, rad) = (r.x as f32 + r.w as f32 / 2.0, r.y as f32 + r.h as f32 / 2.0, r.w as f32 / 2.0 - 3.0);
    fb.fill(r, PAPER);
    let fg = if pressed {
        disc(fb, cx, cy, rad, INK);
        PAPER
    } else {
        ring(fb, cx, cy, rad - 3.0, 6.0, INK);
        INK
    };
    let arm = rad * 0.42;
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
    let r = bar_rect();
    fb.fill(r, PAPER);
    let n = l.max.max(1);
    let gap = 6;
    let seg = (r.w - gap * (n - 1)) / n;
    for i in 0..n {
        let sx = r.x + i * (seg + gap);
        let cell = Rect::new(sx, r.y, seg, r.h);
        if i < l.level {
            fb.round_rect(cell, 6, INK);
        } else {
            fb.round_outline(cell, 6, 3, INK, PAPER);
        }
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
    fb.fill(Rect::new(MARGIN, 1100, 1072 - 2 * MARGIN, 3), INK);
    let r = settings_row_rect();
    f.draw(fb, Face::Display, 56.0, MARGIN, r.y + 74, INK, "Ajustes do sistema");
    f.draw(fb, Face::Body, 30.0, MARGIN, r.y + 120, MUTED, "Wi-Fi, endereço, bateria, reiniciar e desligar");
    let (cx, cy) = (r.x + r.w - 40, r.y + r.h / 2);
    line(fb, ((cx - 14) as f32, (cy - 22) as f32), (cx as f32, cy as f32), 6.0, INK);
    line(fb, (cx as f32, cy as f32), ((cx - 14) as f32, (cy + 22) as f32), 6.0, INK);
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

fn draw_weather(fb: &mut Framebuffer, f: &Fonts) {
    let Some(w) = crate::weather::load() else { return };
    let right = 1072 - MARGIN;
    let temp = format!("{}°", w.temp);
    let tw = f.measure(Face::Display, 92.0, &temp);
    f.draw(fb, Face::Display, 92.0, right - tw, 290, INK, &temp);
    let desc = w.describe();
    let line1 = format!("{}° / {}°", w.max, w.min);
    let l1w = f.measure(Face::Strong, 28.0, &line1);
    f.draw(fb, Face::Strong, 28.0, right - l1w, 334, INK, &line1);
    let line2 = if w.rain_pct >= 30 { format!("chuva {}%", w.rain_pct) } else { desc.to_lowercase() };
    let l2w = f.measure(Face::Body, 26.0, &line2);
    f.draw(fb, Face::Body, 26.0, right - l2w, 370, MUTED, &line2);
}

/// Cabeçalho das telas de digitar: rótulo pequeno em cima e título grande.
pub fn draw_field_header(fb: &mut Framebuffer, f: &Fonts, kicker: &str, title: &str) {
    fb.fill(fb.screen(), PAPER);
    draw_back(fb, f, false);
    f.draw(fb, Face::Strong, 32.0, MARGIN, 250, MUTED, kicker);
    f.draw(fb, Face::Display, 64.0, MARGIN, 340, INK, title);
}
