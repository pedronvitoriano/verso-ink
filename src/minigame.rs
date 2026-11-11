//! Base comum dos minijogos: cada jogo desenha a tela, declara alvos de toque e reage a eles.
//! O app cuida de navegação, destaque de toque, atualização do e-ink e lances da CPU.

use crate::fb::{Framebuffer, Rect};
use crate::text::{Face, Fonts};
use crate::ui::{INK, MUTED, PAPER};

pub const MARGIN: i32 = 72;
pub const W: i32 = 1072;

/// O que redesenhar depois de um toque.
pub enum Redraw {
    None,
    /// Só uma região (atualização parcial, sem piscar).
    Part(Rect),
    /// A tela inteira.
    Full,
}

pub trait MiniGame {
    fn draw(&self, fb: &mut Framebuffer, f: &Fonts);
    /// Alvos de toque (id do jogo, retângulo). O botão Início é do app.
    fn targets(&self) -> Vec<(u32, Rect)>;
    fn tap(&mut self, id: u32, x: i32, y: i32) -> Redraw;
    /// Guarda o estado para continuar depois (quando fizer sentido).
    fn save(&self) {}
    /// É a vez da CPU? O app mostra "pensando" e chama `cpu_move`.
    fn cpu_turn(&self) -> bool {
        false
    }
    fn cpu_move(&mut self) {}
}

// ---------- Desenho comum ----------

pub fn pill(fb: &mut Framebuffer, f: &Fonts, r: Rect, label: &str, filled: bool) {
    fb.fill(r, PAPER);
    let fg = if filled {
        fb.round_rect(r, r.h / 2, INK);
        PAPER
    } else {
        fb.round_outline(r, r.h / 2, 4, INK, PAPER);
        INK
    };
    let mut size = 36.0;
    while f.measure(Face::Strong, size, label) > r.w - 30 && size > 22.0 {
        size -= 2.0;
    }
    let tw = f.measure(Face::Strong, size, label);
    f.draw(fb, Face::Strong, size, r.x + (r.w - tw) / 2, r.y + r.h / 2 + (size * 0.36) as i32, fg, label);
}

/// Linha de botões na base da tela (até 4).
pub fn bottom_button(k: usize, n: usize) -> Rect {
    let n = n.max(1) as i32;
    let w = (W - 2 * MARGIN - (n - 1) * 16) / n;
    Rect::new(MARGIN + k as i32 * (w + 16), 1322, w, 96)
}

pub fn header(fb: &mut Framebuffer, f: &Fonts, title: &str, status: &str) {
    let tw = f.measure(Face::Strong, 34.0, title);
    f.draw(fb, Face::Strong, 34.0, W - MARGIN - tw, 128, INK, title);
    fb.fill(Rect::new(0, 170, W, 80), PAPER);
    f.draw(fb, Face::Display, 44.0, MARGIN, 226, INK, &crate::apps::fit(f, Face::Display, 44.0, status, W - 2 * MARGIN));
}

/// Escolha de modo (contra a CPU / duas pessoas) e de nível, comum aos jogos de dois lados.
pub fn menu_row(k: usize) -> Rect {
    Rect::new(MARGIN, 470 + k as i32 * 150, W - 2 * MARGIN, 124)
}

pub fn draw_menu(fb: &mut Framebuffer, f: &Fonts, title: &str, subtitle: &str, options: &[&str]) {
    fb.fill(Rect::new(0, 170, W, 1448 - 170), PAPER);
    f.draw(fb, Face::Display, 130.0, MARGIN - 6, 330, INK, title);
    f.draw(fb, Face::Strong, 32.0, MARGIN, 410, MUTED, subtitle);
    for (k, o) in options.iter().enumerate() {
        pill(fb, f, menu_row(k), o, k == 0);
    }
}

/// Grade quadrada centrada: (x0, y0, lado da casa).
pub fn grid_geom(cols: i32, rows: i32, top: i32, max_h: i32) -> (i32, i32, i32) {
    let cell = ((W - 2 * 40) / cols).min(max_h / rows);
    let x0 = (W - cell * cols) / 2;
    (x0, top, cell)
}

/// Gerador simples para os jogos (xorshift do Sudoku).
pub use crate::sudoku::Rng;
