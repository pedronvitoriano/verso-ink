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
            Module::Jogar => "Xadrez, sudoku e ficção",
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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Screen {
    Home,
    Module(Module),
    Sleep,
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
    Rect::new(MARGIN - 8, 170, 760, 230)
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
}

pub fn draw_tile(fb: &mut Framebuffer, f: &Fonts, i: usize, pressed: bool) {
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
        icon(fb, m, r.x + r.w - 44 - 128, r.y + (r.h - 128) / 2, 128, fg);
        f.draw(fb, Face::Display, 104.0, r.x + 44, r.y + r.h - 100, fg, m.title());
        f.draw(fb, Face::Body, 31.0, r.x + 48, r.y + r.h - 46, sub, m.subtitle());
    } else {
        icon(fb, m, r.x + 36, r.y + 34, 66, fg);
        f.draw(fb, Face::Display, 72.0, r.x + 34, r.y + r.h - 72, fg, m.title());
        f.draw(fb, Face::Body, 26.0, r.x + 37, r.y + r.h - 32, sub, m.subtitle());
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

pub fn draw_home(fb: &mut Framebuffer, f: &Fonts, st: &Status) {
    fb.fill(fb.screen(), PAPER);
    draw_header(fb, f, st);
    draw_clock(fb, f, st);
    fb.fill(Rect::new(MARGIN, 432, 1072 - 2 * MARGIN, 4), INK);
    for i in 0..Module::ALL.len() {
        draw_tile(fb, f, i, false);
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
const COMING: [(&str, &str); 4] = [
    ("Artigos", "Ler depois, sem distração"),
    ("Terminal", "O Linux do Kindle na tela"),
    ("Porta-retrato", "Fotos em tons de cinza"),
    ("Receitas", "Aberta na cozinha, sem apagar"),
];

fn draw_more(fb: &mut Framebuffer, f: &Fonts, st: &Status) {
    fb.fill(fb.screen(), PAPER);
    draw_back(fb, f, false);
    f.draw(fb, Face::Display, 180.0, MARGIN - 6, 400, INK, "Mais");
    f.draw(fb, Face::Strong, 40.0, MARGIN, 480, MUTED, "O que vem por aí");
    let tag = "EM BREVE";
    let tw = f.measure(Face::Strong, 24.0, tag);
    for (k, (name, desc)) in COMING.iter().enumerate() {
        let y = 560 + k as i32 * 168;
        fb.fill(Rect::new(MARGIN, y, 1072 - 2 * MARGIN, 3), INK);
        f.draw(fb, Face::Display, 60.0, MARGIN, y + 86, INK, name);
        f.draw(fb, Face::Body, 32.0, MARGIN + 2, y + 136, MUTED, desc);
        let tr = Rect::new(1072 - MARGIN - tw - 44, y + 42, tw + 44, 52);
        fb.round_outline(tr, 26, 3, INK, PAPER);
        f.draw(fb, Face::Strong, 24.0, tr.x + 22, tr.y + 35, INK, tag);
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
/// Clara de propósito: escura parecia aparelho apagado.
pub fn draw_sleep(fb: &mut Framebuffer, f: &Fonts, st: &Status) {
    fb.fill(fb.screen(), PAPER);
    f.draw(fb, Face::Display, 40.0, MARGIN, 150, INK, "Verso Ink");
    icon_moon(fb, MARGIN, 420, 150);
    f.draw(fb, Face::Display, 150.0, MARGIN - 6, 760, INK, "Em repouso");
    f.draw(fb, Face::Body, 42.0, MARGIN, 850, INK, "Aperte o botão de ligar para acordar.");
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
