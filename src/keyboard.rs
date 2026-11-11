//! Teclado na tela em português: letras (com ç), números e símbolos, e acentos.

use crate::fb::{Framebuffer, Rect};
use crate::text::{Face, Fonts};
use crate::ui::{self, INK, PAPER};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Letters,
    Numbers,
    Accents,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Action {
    Char(char),
    Shift,
    Backspace,
    Mode(Mode),
    Space,
    Done,
}

pub struct Key {
    pub label: String,
    pub action: Action,
    pub rect: Rect,
}

pub const TOP: i32 = 900;
const ROW_H: i32 = 120;
const GAP: i32 = 8;
const SIDE: i32 = 20;

fn row_chars(s: &str, shift: bool) -> Vec<(String, Action, f32)> {
    s.chars()
        .map(|c| {
            let c = if shift { c.to_uppercase().next().unwrap_or(c) } else { c };
            (c.to_string(), Action::Char(c), 1.0)
        })
        .collect()
}

/// Monta as teclas do modo atual com as posições na tela.
pub fn layout(mode: Mode, shift: bool) -> Vec<Key> {
    let mut rows: Vec<Vec<(String, Action, f32)>> = match mode {
        Mode::Letters => vec![row_chars("qwertyuiop", shift), row_chars("asdfghjklç", shift), {
            let mut r = vec![("⇧".to_string(), Action::Shift, 1.5)];
            r.extend(row_chars("zxcvbnm", shift));
            r.push(("⌫".to_string(), Action::Backspace, 1.5));
            r
        }],
        Mode::Numbers => vec![row_chars("1234567890", false), row_chars("-/:;()@&\"'", false), {
            let mut r = row_chars(".,?!#%+=*", false);
            r.push(("⌫".to_string(), Action::Backspace, 1.0));
            r
        }],
        Mode::Accents => vec![row_chars("áàâãéêíóôõ", shift), row_chars("úüçñ", shift), {
            let mut r = vec![("⇧".to_string(), Action::Shift, 1.5)];
            r.extend(row_chars("ºª°", false));
            r.push(("⌫".to_string(), Action::Backspace, 1.5));
            r
        }],
    };
    let toggle = if mode == Mode::Letters { ("123", Mode::Numbers) } else { ("abc", Mode::Letters) };
    rows.push(vec![
        (toggle.0.to_string(), Action::Mode(toggle.1), 1.5),
        ("áé".to_string(), Action::Mode(Mode::Accents), 1.3),
        ("espaço".to_string(), Action::Space, 5.7),
        ("OK".to_string(), Action::Done, 2.0),
    ]);
    let mut keys = Vec::new();
    for (ri, row) in rows.into_iter().enumerate() {
        // Linhas completas usam a largura de 10 teclas; as menores ficam centralizadas.
        let unit = (1072 - 2 * SIDE - 9 * GAP) as f32 / 10.0;
        let total: f32 = row.iter().map(|k| k.2 * unit).sum::<f32>() + GAP as f32 * (row.len() as f32 - 1.0);
        let mut x = (1072.0 - total) / 2.0;
        let y = TOP + ri as i32 * (ROW_H + GAP);
        for (label, action, w) in row {
            let wpx = (w * unit) as i32;
            keys.push(Key { label, action, rect: Rect::new(x as i32, y, wpx, ROW_H) });
            x += wpx as f32 + GAP as f32;
        }
    }
    keys
}

pub fn area() -> Rect {
    Rect::new(0, TOP - 10, 1072, 1448 - TOP + 10)
}

pub fn draw(fb: &mut Framebuffer, f: &Fonts, keys: &[Key]) {
    fb.fill(area(), PAPER);
    for k in keys {
        let special = !matches!(k.action, Action::Char(_));
        if special && k.action == Action::Done {
            fb.round_rect(k.rect, 18, INK);
        } else {
            fb.round_outline(k.rect, 18, 3, INK, if special { 0xE8 } else { PAPER });
        }
        let fg = if k.action == Action::Done { PAPER } else { INK };
        match k.action {
            Action::Backspace => {
                let (cx, cy) = (k.rect.x as f32 + k.rect.w as f32 / 2.0, k.rect.y as f32 + k.rect.h as f32 / 2.0);
                ui::stroke(fb, (cx - 22.0, cy), (cx - 6.0, cy - 16.0), 5.0, fg);
                ui::stroke(fb, (cx - 22.0, cy), (cx - 6.0, cy + 16.0), 5.0, fg);
                ui::stroke(fb, (cx - 20.0, cy), (cx + 24.0, cy), 5.0, fg);
            }
            Action::Shift => {
                let (cx, cy) = (k.rect.x as f32 + k.rect.w as f32 / 2.0, k.rect.y as f32 + k.rect.h as f32 / 2.0);
                ui::stroke(fb, (cx, cy - 20.0), (cx - 18.0, cy), 5.0, fg);
                ui::stroke(fb, (cx, cy - 20.0), (cx + 18.0, cy), 5.0, fg);
                ui::stroke(fb, (cx, cy - 18.0), (cx, cy + 20.0), 5.0, fg);
            }
            _ => {
                let size = if special { 32.0 } else { 50.0 };
                let face = if special { Face::Strong } else { Face::Body };
                let w = f.measure(face, size, &k.label);
                f.draw(fb, face, size, k.rect.x + (k.rect.w - w) / 2, k.rect.y + k.rect.h / 2 + (size * 0.36) as i32, fg, &k.label);
            }
        }
    }
}
