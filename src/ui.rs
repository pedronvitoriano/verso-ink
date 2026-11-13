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
    Violao,
    Mais,
}

impl Module {
    pub const ALL: [Module; 6] = [Module::Ler, Module::Jogar, Module::Foco, Module::Claude, Module::Violao, Module::Mais];
    pub fn title(self) -> &'static str {
        match self {
            Module::Ler => "Ler",
            Module::Jogar => "Jogar",
            Module::Foco => "Foco",
            Module::Claude => "Verso",
            Module::Violao => "Aprender",
            Module::Mais => "Mais",
        }
    }
    pub fn subtitle(self) -> &'static str {
        match self {
            Module::Ler => "Biblioteca e Verso One",
            Module::Jogar => "Xadrez, sudoku e mais 15",
            Module::Foco => "Pomodoro e o seu dia",
            Module::Claude => "Conversa, arquivos e contexto",
            Module::Violao => "Violão, partitura e mais",
            Module::Mais => "Edição, artigos e mais",
        }
    }
    pub fn plan(self) -> [&'static str; 3] {
        match self {
            Module::Ler => [
                "Seus EPUB, PDF e quadrinhos,",
                "com o motor do Verso One",
                "e o que o Verso escrever para você.",
            ],
            Module::Jogar => [
                "Xadrez contra o Verso,",
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
            Module::Violao | Module::Mais => ["", "", ""],
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
pub fn chips_rect() -> Rect {
    Rect::new(MARGIN, 466, 1072 - 2 * MARGIN, 84)
}
/// Módulos numa grade igual de 2 colunas (o último sozinho ocupa a linha toda).
pub fn tile_rect(i: usize) -> Rect {
    let (top, gap, bottom) = (CONT_Y + CONT_H + 36, 22, 1300);
    let n = Module::ALL.len() as i32;
    let rows = (n + 1) / 2;
    let full_w = 1072 - 2 * MARGIN;
    let w = (full_w - gap) / 2;
    let h = (bottom - top - (rows - 1) * gap) / rows;
    let k = i as i32;
    let (col, row) = (k % 2, k / 2);
    if k == n - 1 && n % 2 == 1 {
        return Rect::new(MARGIN, top + row * (h + gap), full_w, h);
    }
    Rect::new(MARGIN + col * (w + gap), top + row * (h + gap), w, h)
}

/// Faixa "Continuar": o último livro e o último jogo, lado a lado e do mesmo tamanho.
const CONT_Y: i32 = 616;
const CONT_H: i32 = 170;
pub fn continue_book_rect(with_game: bool) -> Rect {
    let w = if with_game { (1072 - 2 * MARGIN - 22) / 2 } else { 1072 - 2 * MARGIN };
    Rect::new(MARGIN, CONT_Y, w, CONT_H)
}
pub fn continue_game_rect(with_book: bool) -> Rect {
    if !with_book {
        return Rect::new(MARGIN, CONT_Y, 1072 - 2 * MARGIN, CONT_H);
    }
    let w = (1072 - 2 * MARGIN - 22) / 2;
    Rect::new(MARGIN + w + 22, CONT_Y, w, CONT_H)
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
    Rect::new(MARGIN - 16, 866, 1072 - 2 * MARGIN + 32, 116)
}
/// Modo avião: desliga o rádio do Kindle (Wi-Fi), economiza bateria.
pub fn plane_rect() -> Rect {
    Rect::new(MARGIN - 16, 990, 1072 - 2 * MARGIN + 32, 116)
}
/// Atalho para a Início no painel Luz e tela (quando ele foi aberto de outra tela).
pub fn light_home_rect() -> Rect {
    Rect::new(MARGIN - 16 + 230 + 20, 72, 220, 88)
}

/// Botão do Claude no painel Luz e tela (pergunta sobre a tela de onde você veio).
pub fn light_claude_rect() -> Rect {
    Rect::new(1072 - MARGIN - 420, 72, 420, 88)
}
pub fn settings_row_rect() -> Rect {
    Rect::new(MARGIN, 1226, 1072 - 2 * MARGIN, 84)
}
/// Wi-Fi direto no painel (trocar de rede sem passar pelos ajustes).
pub fn wifi_row_rect() -> Rect {
    Rect::new(MARGIN, 1124, 1072 - 2 * MARGIN, 84)
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
    // Limpa com a cor do fundo que já está ali (o canto fica fora da forma arredondada), para o
    // cartão não deixar cantos brancos sobre um fundo cinza.
    let bg = fb.get(r.x, r.y + r.h + SH - 1).max(fb.get(r.x + r.w - 1, r.y));
    fb.fill(Rect::new(r.x, r.y, r.w, r.h + SH), bg);
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
        Module::Mais | Module::Violao => {
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
    // Hora em destaque na linha de cima e a data ao lado (a frase do dia é a protagonista).
    if now.plausible() {
        let end = f.draw(fb, Face::Display, 40.0, r.x, r.y + 46, INK, &now.time());
        f.draw(fb, Face::Strong, 28.0, end + 18, r.y + 44, MUTED, &now.date().to_uppercase());
    } else {
        f.draw(fb, Face::Strong, 30.0, r.x, r.y + 44, MUTED, "ACERTANDO O RELÓGIO");
    }
    if let Some(p) = st.battery {
        let label = format!("{p}%");
        let lw = f.measure(Face::Strong, 30.0, &label);
        let gx = 1072 - MARGIN - 74;
        f.draw(fb, Face::Strong, 30.0, gx - 16 - lw, r.y + 44, INK, &label);
        battery_glyph(fb, gx, r.y + 18, p, st.charging);
    }
}

pub fn draw_clock(fb: &mut Framebuffer, f: &Fonts, st: &Status) {
    let r = time_rect();
    fb.fill(r, PAPER);
    // A frase do momento, grande: o toque pessoal da tela inicial; embaixo, o botão dela.
    let p = home_phrase(st);
    let kicker = match p.rarity {
        crate::saudacao::Rarity::Rare => "ACHADO RARO",
        crate::saudacao::Rarity::Legendary => "ACHADO LENDÁRIO",
        _ => "",
    };
    let text = p.text.trim_start_matches("LENDÁRIO: ").to_string();
    let top = r.y + if kicker.is_empty() { 0 } else { 40 };
    let bottom = r.y + r.h - if p.act == crate::saudacao::Act::None { 0 } else { 74 };
    let mut size = 76.0;
    let fits = |size: f32, n: usize| n as i32 * (size * 1.1) as i32 <= bottom - top;
    let mut lines = crate::chat::wrap_words(f, Face::Display, size, &text, r.w - 10);
    while (lines.len() > 3 || !fits(size, lines.len())) && size > 40.0 {
        size -= 4.0;
        lines = crate::chat::wrap_words(f, Face::Display, size, &text, r.w - 10);
    }
    let lh = (size * 1.1) as i32;
    let n = lines.len().min(4) as i32;
    if !kicker.is_empty() {
        f.draw(fb, Face::Strong, 26.0, MARGIN, r.y + 34, MUTED, kicker);
    }
    let y0 = top + (bottom - top - n * lh) / 2 + (size * 0.8) as i32;
    for (k, l) in lines.iter().take(4).enumerate() {
        f.draw(fb, Face::Display, size, MARGIN - 4, y0 + k as i32 * lh, INK, l);
    }
    if p.act != crate::saudacao::Act::None {
        button(fb, f, phrase_rect(f, p.act), p.act.label(), matches!(p.act, crate::saudacao::Act::Achado(_)));
    }
    draw_chips(fb, f, st);
}

/// Coleção de achados raros: os guardados com o desenho, os que faltam como "?".
/// `new` = o que acabou de entrar (destaque e o bônus embaixo).
pub fn draw_achados(fb: &mut Framebuffer, f: &Fonts, new: Option<usize>) {
    use crate::saudacao::ITEMS;
    fb.fill(fb.screen(), PAPER);
    draw_back(fb, f, false);
    let got: Vec<usize> = crate::saudacao::collected().iter().map(|(i, _)| *i).collect();
    let owned = (0..ITEMS.len()).filter(|i| got.contains(i)).count();
    f.draw(fb, Face::Display, 110.0, MARGIN - 4, 300, INK, "Achados");
    f.draw(fb, Face::Strong, 30.0, MARGIN, 360, MUTED, &format!("{owned} de {} · aparecem nas frases da Início", ITEMS.len()));
    let cw = (1072 - 2 * MARGIN - 3 * 16) / 4;
    for (i, it) in ITEMS.iter().enumerate() {
        let r = Rect::new(MARGIN + (i % 4) as i32 * (cw + 16), 400 + (i / 4) as i32 * 196, cw, 180);
        let have = got.contains(&i);
        let n = got.iter().filter(|g| **g == i).count();
        if Some(i) == new {
            card(fb, r, 24, INK, false);
            fb.round_rect(Rect::new(r.x + 10, r.y + 10, r.w - 20, 130), 18, PAPER);
        } else if have {
            card(fb, r, 24, PAPER, false);
        } else {
            fb.round_rect(r, 24, SURFACE);
        }
        if have {
            crate::sprites::draw(fb, it.2, Rect::new(r.x + (r.w - 120) / 2, r.y + 14, 120, 120));
            let name = crate::apps::fit(f, Face::Strong, 20.0, it.0, r.w - 16);
            let w = f.measure(Face::Strong, 20.0, &name);
            f.draw(fb, Face::Strong, 20.0, r.x + (r.w - w) / 2, r.y + 166, if Some(i) == new { PAPER } else { INK }, &name);
            if n > 1 {
                f.draw(fb, Face::Strong, 22.0, r.x + r.w - 44, r.y + 34, MUTED, &format!("×{n}"));
            }
        } else {
            let q = if it.4 { "★" } else { "?" };
            let w = f.measure(Face::Display, 70.0, q);
            f.draw(fb, Face::Display, 70.0, r.x + (r.w - w) / 2, r.y + 110, 0xAA, q);
        }
    }
    if let Some(i) = new {
        let it = &ITEMS[i];
        let t = format!("Na coleção: {}. {}", it.0, crate::saudacao::bonus_text(it.3));
        let w = f.measure(Face::Strong, 30.0, &t);
        f.draw(fb, Face::Strong, 30.0, (1072 - w) / 2, 1230, INK, &t);
    }
    draw_footer(fb, f, &Status { battery: None, charging: false, wifi: false, offset_min: 0 });
}

/// Tela vazia com propósito: ilustração grande, os passos numerados para encher e, quando houver,
/// o endereço para mandar pelo celular.
pub fn draw_empty(fb: &mut Framebuffer, f: &Fonts, top: i32, sprite: &str, steps: &[&str], url: Option<String>) {
    let d = 300;
    crate::sprites::draw(fb, sprite, Rect::new((1072 - d) / 2, top, d, d));
    let mut y = top + d + 40;
    for (k, s) in steps.iter().enumerate() {
        let lines = crate::chat::wrap_words(f, Face::Body, 30.0, s, 1072 - 2 * MARGIN - 80);
        let h = (lines.len() as i32 * 40 + 36).max(84);
        fb.round_rect(Rect::new(MARGIN, y + (h - 52) / 2, 52, 52), 26, INK);
        let n = (k + 1).to_string();
        let w = f.measure(Face::Strong, 28.0, &n);
        f.draw(fb, Face::Strong, 28.0, MARGIN + 26 - w / 2, y + h / 2 + 10, PAPER, &n);
        let ty = y + (h - lines.len() as i32 * 40) / 2 + 30;
        for (i, l) in lines.iter().enumerate() {
            f.draw(fb, Face::Body, 30.0, MARGIN + 80, ty + i as i32 * 40, INK, l);
        }
        y += h + 10;
    }
    if let Some(u) = url {
        let r = Rect::new(MARGIN, y + 20, 1072 - 2 * MARGIN, 120);
        card(fb, r, 28, SURFACE, false);
        f.draw(fb, Face::Strong, 24.0, r.x + 28, r.y + 44, MUTED, "NO CELULAR, ABRA");
        f.draw(fb, Face::Strong, 30.0, r.x + 28, r.y + 90, INK, &crate::apps::fit(f, Face::Strong, 30.0, &u, r.w - 56));
    }
}

/// Botão da frase da Início (largura pelo texto).
pub fn phrase_rect(f: &Fonts, act: crate::saudacao::Act) -> Rect {
    let w = f.measure(Face::Strong, 28.0, act.label()) + 60;
    let r = time_rect();
    Rect::new(MARGIN - 4, r.y + r.h - 66, w.min(r.w), 60)
}

/// Frase do momento, guardada por 5 minutos: montar o contexto lê vários arquivos (destaques,
/// edição, ofensiva) e a frase é pedida a cada desenho da Início e a cada toque.
pub fn home_phrase(st: &Status) -> crate::saudacao::Phrase {
    use std::sync::Mutex;
    static CACHE: Mutex<Option<((i64, u32, u32, usize), crate::saudacao::Phrase)>> = Mutex::new(None);
    let now = clock::now(st.offset_min);
    let key = (crate::data::today(st.offset_min), now.hour, now.minute / 5, crate::saudacao::collected().len());
    let mut c = CACHE.lock().unwrap();
    if let Some((k, p)) = c.as_ref() {
        if *k == key {
            return p.clone();
        }
    }
    let p = home_phrase_fresh(st);
    *c = Some((key, p.clone()));
    p
}

fn home_phrase_fresh(st: &Status) -> crate::saudacao::Phrase {
    let now = clock::now(st.offset_min);
    let book = crate::data::last_book();
    let game = crate::data::last_game().and_then(|i| crate::apps::GAMES.get(i)).map(|g| g.0.to_string());
    let ctx = crate::saudacao::Context {
        hour: now.hour,
        weekday: now.weekday,
        day: crate::data::today(st.offset_min),
        raining: crate::weather::load().is_some_and(|w| w.rain_pct >= 50),
        book: book.map(|b| (b.title, b.progress)),
        game,
        read_today: crate::data::reading_minutes_today(st.offset_min),
        edition: crate::painel::edition().is_some_and(|e| e.day == crate::painel::today_iso(st.offset_min)),
        notice: !crate::painel::notices().is_empty(),
        battery: st.battery,
        guitar: Some((crate::violao::practiced_today(), crate::violao::streak())),
        learn: { let st = crate::aprender::status(); (st.streak > 0 || st.today_done).then_some((st.today_done, st.streak)) },
        review_due: crate::revisao::due_count(),
    };
    let mut p = crate::saudacao::pick(&ctx);
    if p.text.is_empty() {
        p.text = "O que vamos fazer hoje?".into();
    }
    p
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
    // Aviso crítico de um agente (ex.: não desconectar): toma a faixa inteira, em preto.
    if let Some(n) = crate::painel::notices().into_iter().find(|n| n.critical) {
        fb.fill(Rect::new(c.x, c.y, c.w, c.h + SH), PAPER);
        card(fb, c, 26, INK, false);
        let t = crate::apps::fit(f, Face::Strong, 30.0, &format!("! {}", n.text.to_uppercase()), c.w - 48);
        f.draw(fb, Face::Strong, 30.0, c.x + 24, c.y + 54, PAPER, &t);
        return;
    }
    card(fb, r, 26, PAPER, false);
    let today = crate::painel::edition().filter(|e| e.day == crate::painel::today_iso(st.offset_min));
    let line = match today {
        Some(e) => format!("Edição de hoje: {}", e.headline),
        None => crate::data::summary(st.offset_min).line.unwrap_or_else(|| "Hora de ler um pouco".into()),
    };
    let tx = if crate::sprites::draw(fb, "icone-livro", Rect::new(r.x + 14, r.y + 10, 62, 62)) { r.x + 88 } else { r.x + 24 };
    let line = crate::apps::fit(f, Face::Strong, 27.0, &line, r.x + r.w - 20 - tx);
    f.draw(fb, Face::Strong, 27.0, tx, r.y + 52, INK, &line);
}

/// Ilustração do dia (muda à meia-noite): um personagem diferente a cada dia, ao lado da hora.
fn draw_daily_art(fb: &mut Framebuffer, st: &Status) {
    // Achado raro na frase: o próprio achado no lugar da ilustração, num pedestal.
    if let crate::saudacao::Act::Achado(i) = home_phrase(st).act {
        let r = Rect::new(1072 - MARGIN - 300, 170, 300, 280);
        fb.round_rect(Rect::new(r.x + 30, r.y + 220, r.w - 60, 40), 20, SURFACE);
        crate::sprites::draw(fb, crate::saudacao::ITEMS[i].2, Rect::new(r.x + 40, r.y + 10, 220, 220));
        return;
    }
    let day = crate::data::today(st.offset_min);
    let n = (0..40).filter(|k| crate::sprites::exists(&format!("dia-{:02}", k + 1))).count() as i64;
    if n == 0 {
        return;
    }
    let k = day.rem_euclid(n) + 1;
    crate::sprites::draw(fb, &format!("dia-{k:02}"), Rect::new(1072 - MARGIN - 330, 160, 344, 302));
}

/// O que a tela inicial mostra além do fixo: livro em leitura e jogo em andamento.
#[derive(Default)]
pub struct HomeExtra {
    pub book: Option<crate::data::LastBook>,
    #[allow(dead_code)] // guardado no arquivo; ainda sem tela que o mostre
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
    // Letra proporcional à capa: cabe tanto na capa grande quanto na miniatura do Continuar.
    let size = (inner.w as f32 / 6.5).clamp(13.0, 34.0);
    let lh = (size * 1.2) as i32;
    let pad = (inner.w / 10).max(6);
    let cx = inner.x + 11 + inner.w / 2;
    let rule = (inner.w / 3).max(20);
    fb.fill(Rect::new(cx - rule / 2, inner.y + pad + 4, rule, 3), INK);
    fb.fill(Rect::new(cx - rule / 2, inner.y + inner.h - pad - 7, rule, 3), INK);
    let room_w = inner.w - 22 - 2 * pad;
    let max_lines = ((inner.h - 4 * pad - if author.is_empty() { 0 } else { lh }) / lh).max(1) as usize;
    let lines = crate::chat::wrap_words(f, Face::Display, size, title, room_w);
    let n = lines.len().min(max_lines) as i32;
    let block = n * lh + if author.is_empty() || inner.w < 120 { 0 } else { lh };
    let y0 = inner.y + (inner.h - block) / 2 + (size * 0.8) as i32;
    for (k, l) in lines.iter().take(max_lines).enumerate() {
        let l = if k + 1 == max_lines && lines.len() > max_lines { crate::apps::fit(f, Face::Display, size, &format!("{l}…"), room_w) } else { crate::apps::fit(f, Face::Display, size, l, room_w) };
        let w = f.measure(Face::Display, size, &l);
        f.draw(fb, Face::Display, size, inner.x + 22 + (inner.w - 22 - w) / 2, y0 + k as i32 * lh, INK, &l);
    }
    if !author.is_empty() && inner.w >= 120 {
        let a = crate::apps::fit(f, Face::Body, size * 0.66, author, room_w);
        let w = f.measure(Face::Body, size * 0.66, &a);
        f.draw(fb, Face::Body, size * 0.66, inner.x + 22 + (inner.w - 22 - w) / 2, y0 + n * lh + 2, 0x44, &a);
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

/// Ícone ilustrado de cada módulo.
fn module_icon(m: Module) -> &'static str {
    match m {
        Module::Ler => "ler",
        Module::Jogar => "jogar",
        Module::Foco => "foco",
        Module::Claude => "claude",
        Module::Violao => if crate::sprites::exists("icone-aprender") { "aprender" } else { "violao" },
        Module::Mais => "mais",
    }
}

pub fn draw_tile(fb: &mut Framebuffer, f: &Fonts, i: usize, pressed: bool, _extra: &HomeExtra) {
    let m = Module::ALL[i];
    let r0 = tile_rect(i);
    card(fb, r0, 30, PAPER, pressed);
    let r = if pressed { Rect::new(r0.x, r0.y + SH, r0.w, r0.h) } else { r0 };
    let d = (r.h - 44).min(104);
    let has = icon_disc(fb, r.x + 22, r.y + (r.h - d) / 2, d, module_icon(m));
    if !has {
        icon(fb, m, r.x + 36, r.y + (r.h - 66) / 2, 66, INK);
    }
    let x = r.x + 22 + d + 22;
    fb.fill(Rect::new(x - 12, r.y + 26, 3, r.h - 52), 0xBB);
    f.draw(fb, Face::Display, 50.0, x + 4, r.y + r.h / 2 + 4, INK, m.title());
    // Violão: com ofensiva viva, o cartão mostra a chama (apagada se ainda falta hoje).
    let mut x = x;
    let sub = match m {
        Module::Violao => {
            let st = crate::aprender::status();
            if st.streak > 0 {
                crate::aprender::flame(fb, Rect::new(x + 4, r.y + r.h / 2 + 12, 36, 36), st.today_done);
                x += 38;
                if st.today_done { format!("{} dia{} de ofensiva", st.streak, if st.streak == 1 { "" } else { "s" }) } else { format!("{} dia{} · falta hoje", st.streak, if st.streak == 1 { "" } else { "s" }) }
            } else {
                m.subtitle().to_string()
            }
        }
        Module::Mais => {
            let items = crate::day::load_items();
            let done = crate::day::checked(crate::data::today(crate::offset_min()));
            let habits: Vec<_> = items.iter().filter(|i| i.kind == crate::day::Kind::Habit).collect();
            let tasks: Vec<_> = items.iter().filter(|i| i.kind == crate::day::Kind::Task).collect();
            let (quick, quick_done) = crate::organizacao::resumo_rapidas();
            if habits.is_empty() && tasks.is_empty() && quick == 0 { m.subtitle().to_string() }
            else {
                let habits_done = habits.iter().filter(|i| done.contains(&i.key())).count();
                let tasks_done = tasks.iter().filter(|i| done.contains(&i.key())).count() + quick_done;
                format!("Hábitos: {habits_done}/{} · Tarefas: {tasks_done}/{}", habits.len(), tasks.len() + quick)
            }
        }
        _ => m.subtitle().to_string(),
    };
    let text = crate::apps::fit(f, Face::Body, 24.0, &sub, r.x + r.w - 16 - x);
    f.draw(fb, Face::Body, 24.0, x + 6, r.y + r.h / 2 + 42, MUTED, &text);
}

/// Faixa "Continuar": cartões do último livro e do último jogo.
fn draw_continue(fb: &mut Framebuffer, f: &Fonts, st: &Status, extra: &HomeExtra, game: Option<&(usize, String)>) {
    let book = extra.book.as_ref();
    let _ = st;
    if book.is_some() || game.is_some() {
        f.draw(fb, Face::Strong, 30.0, MARGIN, CONT_Y - 18, INK, "Continuar");
    }
    if book.is_none() && game.is_none() {
        return;
    }
    if let Some(b) = book {
        let r = continue_book_rect(game.is_some());
        card(fb, r, 28, PAPER, false);
        let cover = Rect::new(r.x + 18, r.y + 16, (r.h - 32) * 2 / 3, r.h - 32);
        draw_cover(fb, f, cover, Some(b));
        let x = cover.x + cover.w + 22;
        let room = r.x + r.w - 18 - x;
        let (title, author) = split_title(&b.title);
        let lines = crate::chat::wrap_words(f, Face::Display, 32.0, title, room);
        for (k, l) in lines.iter().take(2).enumerate() {
            let l = if k == 1 && lines.len() > 2 { crate::apps::fit(f, Face::Display, 32.0, &format!("{l}…"), room) } else { l.clone() };
            f.draw(fb, Face::Display, 32.0, x, r.y + 50 + k as i32 * 38, INK, &l);
        }
        let pct = b.progress.map(|p| format!("{}% lido", (p * 100.0).round() as i32)).unwrap_or_else(|| "Começar".into());
        let meta = if author.is_empty() { pct } else { format!("{author} · {pct}") };
        f.draw(fb, Face::Body, 22.0, x, r.y + r.h - 34, MUTED, &crate::apps::fit(f, Face::Body, 22.0, &meta, room));
        if let Some(p) = b.progress {
            let bar = Rect::new(x, r.y + r.h - 24, room, 8);
            fb.round_rect(bar, 4, SURFACE);
            fb.round_rect(Rect::new(bar.x, bar.y, ((bar.w as f32 * p.clamp(0.0, 1.0)) as i32).max(8), bar.h), 4, 0x55);
        }
    }
    if let Some((i, sum)) = game {
        let r = continue_game_rect(book.is_some());
        card(fb, r, 28, PAPER, false);
        let (name, _) = crate::apps::GAMES[*i];
        let d = 96;
        let slug = crate::apps::fold(name).replace(' ', "");
        let x = if icon_disc(fb, r.x + 18, r.y + (r.h - d) / 2, d, &slug) { r.x + 18 + d + 20 } else { r.x + 26 };
        let room = r.x + r.w - 18 - x;
        f.draw(fb, Face::Display, 40.0, x, r.y + 70, INK, &crate::apps::fit(f, Face::Display, 40.0, name, room));
        for (k, l) in crate::chat::wrap_words(f, Face::Body, 23.0, sum, room).iter().take(2).enumerate() {
            f.draw(fb, Face::Body, 23.0, x, r.y + 108 + k as i32 * 30, MUTED, l);
        }
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
    // Estado não é botão: no rodapé ele fica discreto, sem a sombra reservada para ações.
    let cw = w + if has { 46 } else { 0 };
    let x = 1072 - MARGIN - cw;
    let mut tx = x;
    if has {
        crate::sprites::draw(fb, "icone-wifi", Rect::new(x, 1359, 34, 34));
        tx += 46;
    }
    f.draw(fb, Face::Strong, 26.0, tx, 1387, if st.wifi { INK } else { MUTED }, wifi);
}

pub fn draw_home(fb: &mut Framebuffer, f: &Fonts, st: &Status, sum: &crate::data::Summary, game: Option<&(usize, String)>) {
    fb.fill(fb.screen(), PAPER);
    draw_header(fb, f, st);
    draw_daily_art(fb, st);
    draw_clock(fb, f, st);
    let _ = sum; // o resumo é desenhado nos cartões abaixo da hora (draw_chips)
    let extra = HomeExtra::load();
    draw_continue(fb, f, st, &extra, game);
    for i in 0..Module::ALL.len() {
        draw_tile(fb, f, i, false, &extra);
    }
    draw_footer(fb, f, st);
}

/// Para onde o botão de voltar leva (o app define a cada desenho): "Início", "Mais", "Voltar"…
static BACK_LABEL: std::sync::Mutex<&'static str> = std::sync::Mutex::new("Início");

pub fn set_back_label(l: &'static str) {
    *BACK_LABEL.lock().unwrap() = l;
}

pub fn draw_back(fb: &mut Framebuffer, f: &Fonts, pressed: bool) {
    let r0 = back_rect();
    card(fb, r0, r0.h / 2, if pressed { INK } else { PAPER }, pressed);
    let fg = if pressed { PAPER } else { INK };
    let r = if pressed { Rect::new(r0.x, r0.y + SH, r0.w, r0.h) } else { r0 };
    line(fb, (r.x as f32 + 54.0, r.y as f32 + 28.0), (r.x as f32 + 38.0, r.y as f32 + 44.0), 6.0, fg);
    line(fb, (r.x as f32 + 38.0, r.y as f32 + 44.0), (r.x as f32 + 54.0, r.y as f32 + 60.0), 6.0, fg);
    let label = *BACK_LABEL.lock().unwrap();
    let mut size = 36.0;
    while f.measure(Face::Strong, size, label) > r.w - 96 && size > 24.0 {
        size -= 2.0;
    }
    f.draw(fb, Face::Strong, size, r.x + 76, r.y + r.h / 2 + (size * 0.36) as i32, fg, label);
}

/// Ideias de módulos futuros (da conversa de concepção); entram aqui conforme forem feitos.
/// Módulos do Mais: (nome, descrição, pronto?). Os prontos abrem ao toque.
pub const COMING: [(&str, &str, bool); 11] = [
    ("Edição da manhã", "Os agentes da noite, avisos e notícias", true),
    ("Missões", "Mande os agentes do PC trabalharem por você", true),
    ("Artigos", "Links mandados do celular, para ler depois", true),
    ("Porta-retrato", "Fotos do celular em tons de cinza", true),
    ("Receitas", "Aberta na cozinha, sem apagar", true),
    ("Terminal", "Comandos de um toque no Linux do Kindle", true),
    ("Semana", "Fechamento com o Verso e o plano da próxima", true),
    ("Diário", "Uma pergunta para guardar o dia", true),
    ("Hábitos e tarefas", "Marque o dia e acompanhe sequências", true),
    ("Calculadora", "Contas, conversões e divisão", true),
    ("Lembretes", "Um aviso na hora escolhida", true),
];

pub fn more_row(k: usize) -> Rect {
    Rect::new(MARGIN + (k % 2) as i32 * 474, 470 + (k / 2) as i32 * 136, 454, 120)
}

fn draw_more(fb: &mut Framebuffer, f: &Fonts, st: &Status) {
    fb.fill(fb.screen(), PAPER);
    draw_back(fb, f, false);
    crate::sprites::draw(fb, "cab-mais", Rect::new(1072 - MARGIN - 330, 96, 340, 290));
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
        let title = crate::apps::fit(f, Face::Display, 34.0, name, r.w - 48);
        f.draw(fb, Face::Display, 34.0, r.x + 24, r.y + 50, ink, &title);
        for (line, text) in crate::chat::wrap_words(f, Face::Body, 23.0, desc, r.w - 48).iter().take(2).enumerate() {
            f.draw(fb, Face::Body, 23.0, r.x + 24, r.y + 82 + line as i32 * 25, MUTED, text);
        }
    }
    let wifi = if st.wifi { "Wi-Fi conectado" } else { "Sem Wi-Fi" };
    let width = f.measure(Face::Strong, 24.0, wifi);
    let end = f.draw(fb, Face::Strong, 24.0, MARGIN, 1355, MUTED, "Verso Ink");
    if let Some(v) = crate::backup::update_available() {
        let msg = crate::apps::fit(f, Face::Strong, 24.0, &format!("· versão {v} no Ajustes"), 1072 - MARGIN - width - end - 36);
        f.draw(fb, Face::Strong, 24.0, end + 16, 1355, INK, &msg);
    }
    f.draw(fb, Face::Strong, 24.0, 1072 - MARGIN - width, 1355, MUTED, wifi);
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
/// O que estava na tela quando o Kindle dormiu.
pub struct SleepScene {
    pub what: String,
    pub detail: String,
    /// Trecho da página (livro).
    pub quote: Option<String>,
    /// Miniatura da tela (jogo): largura, altura e pixels.
    pub shot: Option<(i32, i32, Vec<u8>)>,
    /// Foto do porta-retrato (dorme mostrando a foto inteira).
    pub photo: Option<std::path::PathBuf>,
}

/// Tela de repouso: continua o que estava aberto. Jogo: a partida em miniatura, como uma foto
/// na mesa; livro: o trecho da página onde parou; outras telas: o personagem do dia.
pub fn draw_sleep(fb: &mut Framebuffer, f: &Fonts, st: &Status, scene: Option<&SleepScene>) {
    fb.fill(fb.screen(), PAPER);
    if let Some(img) = scene.and_then(|s| s.photo.as_ref()).and_then(|p| image::open(p).ok()) {
        // Porta-retrato: a foto fica, a tela inteira.
        let g = img.to_luma8();
        fb.blit(0, 0, g.width() as i32, g.height() as i32, g.as_raw());
        return;
    }
    let now = clock::now(st.offset_min);
    // Topo: "Em repouso" e a hora em que dormiu; bateria à direita.
    icon_moon(fb, MARGIN, 70, 64);
    let since = if now.plausible() { format!("Em repouso desde {}", now.time()) } else { "Em repouso".into() };
    f.draw(fb, Face::Strong, 32.0, MARGIN + 84, 116, INK, &since);
    if let Some(p) = st.battery {
        let label = format!("{p}%");
        let lw = f.measure(Face::Strong, 30.0, &label);
        let gx = 1072 - MARGIN - 74;
        f.draw(fb, Face::Strong, 30.0, gx - 16 - lw, 116, INK, &label);
        battery_glyph(fb, gx, 90, p, st.charging);
    }
    match scene {
        Some(SleepScene { what, detail, shot: Some((w, h, px)), .. }) => {
            // A partida congelada, num cartão com sombra, e o que ela era.
            f.draw(fb, Face::Strong, 28.0, MARGIN, 214, MUTED, "PAUSADO EM");
            f.draw(fb, Face::Display, 70.0, MARGIN - 4, 290, INK, &crate::apps::fit(f, Face::Display, 70.0, what, 1072 - 2 * MARGIN));
            f.draw(fb, Face::Body, 30.0, MARGIN, 340, MUTED, &crate::apps::fit(f, Face::Body, 30.0, detail, 1072 - 2 * MARGIN));
            let r = Rect::new((1072 - w - 24) / 2, 376, w + 24, (*h).min(1300 - 376 - 24) + 24);
            card(fb, r, 30, PAPER, false);
            let vis_h = r.h - 24;
            fb.blit(r.x + 12, r.y + 12, *w, vis_h, &px[..(*w * vis_h) as usize]);
            fb.round_frame(r, 30, 4, INK, PAPER, SH, SHADOW);
        }
        Some(SleepScene { what, detail, quote: Some(q), .. }) => {
            // O livro: o trecho onde parou, grande e calmo, e de onde é.
            f.draw(fb, Face::Strong, 28.0, MARGIN, 214, MUTED, "VOCÊ PAROU AQUI");
            let text: String = q.split_whitespace().collect::<Vec<_>>().join(" ");
            let lines = crate::chat::wrap_words(f, Face::Body, 44.0, &text, 1072 - 2 * MARGIN - 40);
            fb.fill(Rect::new(MARGIN, 260, 8, 13 * 62 - 20), INK);
            for (k, l) in lines.iter().take(13).enumerate() {
                let l = if k == 12 && lines.len() > 13 { format!("{l}…") } else { l.clone() };
                f.draw(fb, Face::Body, 44.0, MARGIN + 40, 300 + k as i32 * 62, INK, &l);
            }
            f.draw(fb, Face::Display, 50.0, MARGIN, 1180, INK, &crate::apps::fit(f, Face::Display, 50.0, what, 1072 - 2 * MARGIN));
            f.draw(fb, Face::Body, 30.0, MARGIN, 1228, MUTED, &crate::apps::fit(f, Face::Body, 30.0, detail, 1072 - 2 * MARGIN));
        }
        _ => {
            // Sem nada aberto: o personagem do dia, grande, e a data.
            let day = crate::data::today(st.offset_min);
            let n = (0..40).filter(|k| crate::sprites::exists(&format!("dia-{:02}", k + 1))).count() as i64;
            if n > 0 {
                let k = day.rem_euclid(n) + 1;
                crate::sprites::draw(fb, &format!("dia-{k:02}"), Rect::new(96, 220, 880, 880));
            }
            if now.plausible() {
                let d = now.date();
                let w = f.measure(Face::Display, 56.0, &d);
                f.draw(fb, Face::Display, 56.0, (1072 - w) / 2, 1200, INK, &d);
            }
        }
    }
    let hint = "Aperte o botão de ligar para continuar";
    let w = f.measure(Face::Strong, 30.0, hint);
    fb.fill(Rect::new(MARGIN, 1300, 1072 - 2 * MARGIN, 3), INK);
    f.draw(fb, Face::Strong, 30.0, (1072 - w) / 2, 1370, INK, hint);
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
    let end = f.draw(fb, Face::Display, if l.level == 0 { 76.0 } else { 120.0 }, MARGIN - 4, 612, INK, &label);
    if l.level > 0 {
        f.draw(fb, Face::Body, 34.0, end + 14, 612, MUTED, &format!("de {}", l.max));
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
    toggle_row(fb, f, night_rect(), "Modo noturno", "Texto claro sobre fundo escuro", l.night, pressed);
}

pub fn draw_plane(fb: &mut Framebuffer, f: &Fonts, on: bool, pressed: bool) {
    toggle_row(fb, f, plane_rect(), "Modo avião", if on { "Rádio desligado: sem Wi-Fi, mais bateria" } else { "Desliga o Wi-Fi e economiza bateria" }, on, pressed);
}

/// Linha com título, explicação e a chave liga/desliga à direita.
fn toggle_row(fb: &mut Framebuffer, f: &Fonts, r: Rect, title: &str, sub: &str, on: bool, pressed: bool) {
    fb.fill(r, PAPER);
    if pressed {
        fb.round_rect(r, 32, 0xDD);
    }
    f.draw(fb, Face::Display, 46.0, MARGIN, r.y + 54, INK, title);
    f.draw(fb, Face::Body, 28.0, MARGIN, r.y + 96, MUTED, sub);
    let sw = Rect::new(1072 - MARGIN - 150, r.y + (r.h - 80) / 2, 150, 80);
    if on {
        fb.round_rect(sw, 40, INK);
        disc(fb, sw.x as f32 + sw.w as f32 - 40.0, sw.y as f32 + 40.0, 30.0, PAPER);
    } else {
        fb.round_outline(sw, 40, 5, INK, if pressed { 0xDD } else { PAPER });
        disc(fb, sw.x as f32 + 40.0, sw.y as f32 + 40.0, 25.0, INK);
    }
}

pub fn draw_light(fb: &mut Framebuffer, f: &Fonts, l: &Light, st: &Status, claude: bool, plane: bool, home: bool) {
    fb.fill(fb.screen(), PAPER);
    draw_back(fb, f, false);
    if home {
        button(fb, f, light_home_rect(), "Início", false);
    }
    if claude {
        let r = light_claude_rect();
        card(fb, r, r.h / 2, INK, false);
        let d = r.h - 20;
        let tx = if icon_disc(fb, r.x + 10, r.y + 10, d, "claude") { r.x + 10 + d + 14 } else { r.x + 30 };
        let t = "Perguntar ao Verso";
        let mut size = 30.0;
        while f.measure(Face::Strong, size, t) > r.x + r.w - 28 - tx && size > 20.0 {
            size -= 1.0;
        }
        f.draw(fb, Face::Strong, size, tx, r.y + r.h / 2 + (size * 0.36) as i32, PAPER, t);
    }
    f.draw(fb, Face::Display, 120.0, MARGIN - 4, 330, INK, "Luz e tela");
    // Bateria e rede, à vista sem voltar para a Início.
    // Sem leitura da bateria (só no PC), mostra só a rede.
    let bat = match (st.battery, st.charging) {
        (Some(b), true) => Some(format!("Bateria {b}% · carregando")),
        (Some(b), false) => Some(format!("Bateria {b}%")),
        (None, _) => None,
    };
    let net = if plane { "Modo avião" } else if st.wifi { "Wi-Fi conectado" } else { "Sem Wi-Fi" };
    let status_line = match bat {
        Some(b) => format!("{b} · {}", net.to_lowercase()),
        None => net.to_string(),
    };
    f.draw(fb, Face::Strong, 30.0, MARGIN, 392, MUTED, &status_line);
    draw_light_level(fb, f, l);
    draw_light_minus(fb, false);
    draw_light_plus(fb, false);
    fb.fill(Rect::new(MARGIN, 848, 1072 - 2 * MARGIN, 3), INK);
    draw_night(fb, f, l, false);
    draw_plane(fb, f, plane, false);
    let rows = [
        (wifi_row_rect(), "Wi-Fi", if plane { "Desligado pelo modo avião" } else if st.wifi { "Conectado · trocar de rede" } else { "Sem rede · conectar" }),
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
    let mut size = 64.0;
    let mut lines = crate::chat::wrap_words(f, Face::Display, size, title, 1072 - 2 * MARGIN);
    if lines.len() > 1 {
        size = 52.0;
        lines = crate::chat::wrap_words(f, Face::Display, size, title, 1072 - 2 * MARGIN);
    }
    while lines.len() > 2 && size > 32.0 {
        size -= 2.0;
        lines = crate::chat::wrap_words(f, Face::Display, size, title, 1072 - 2 * MARGIN);
    }
    if lines.len() <= 1 {
        f.draw(fb, Face::Display, size, MARGIN, 340, INK, title);
    } else {
        for (i, line) in lines.iter().take(2).enumerate() {
            f.draw(fb, Face::Display, size, MARGIN, 315 + i as i32 * 56, INK, line);
        }
    }
}
