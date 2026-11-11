//! Sudoku: gerador com solução única, três níveis, jogo salvo e conferência.

use crate::fb::{Framebuffer, Rect};
use crate::text::{Face, Fonts};
use crate::ui::{self, INK, MUTED, PAPER};

const MARGIN: i32 = 72;
const W: i32 = 1072;
pub const CELL: i32 = 96;
pub const GRID_X: i32 = (W - 9 * CELL) / 2;
pub const GRID_Y: i32 = 300;

/// Gerador pseudoaleatório simples (xorshift), semeado pelo relógio.
pub struct Rng(u64);
impl Rng {
    pub fn new() -> Self {
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1);
        Rng(t | 1)
    }
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    pub fn shuffle<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            let j = (self.next() % (i as u64 + 1)) as usize;
            v.swap(i, j);
        }
    }
}

fn ok(g: &[u8; 81], i: usize, n: u8) -> bool {
    let (r, c) = (i / 9, i % 9);
    for k in 0..9 {
        if g[r * 9 + k] == n || g[k * 9 + c] == n {
            return false;
        }
    }
    let (br, bc) = (r / 3 * 3, c / 3 * 3);
    for dr in 0..3 {
        for dc in 0..3 {
            if g[(br + dr) * 9 + bc + dc] == n {
                return false;
            }
        }
    }
    true
}

fn fill(g: &mut [u8; 81], rng: &mut Rng) -> bool {
    let Some(i) = g.iter().position(|&v| v == 0) else { return true };
    let mut nums = [1u8, 2, 3, 4, 5, 6, 7, 8, 9];
    rng.shuffle(&mut nums);
    for n in nums {
        if ok(g, i, n) {
            g[i] = n;
            if fill(g, rng) {
                return true;
            }
            g[i] = 0;
        }
    }
    false
}

/// Conta soluções até `limit` (para garantir solução única).
fn count(g: &mut [u8; 81], limit: u32) -> u32 {
    let Some(i) = g.iter().position(|&v| v == 0) else { return 1 };
    let mut total = 0;
    for n in 1..=9 {
        if ok(g, i, n) {
            g[i] = n;
            total += count(g, limit - total);
            g[i] = 0;
            if total >= limit {
                break;
            }
        }
    }
    total
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Level {
    Easy,
    Medium,
    Hard,
}

impl Level {
    pub fn name(self) -> &'static str {
        match self {
            Level::Easy => "Fácil",
            Level::Medium => "Médio",
            Level::Hard => "Difícil",
        }
    }
    fn holes(self) -> usize {
        match self {
            Level::Easy => 38,
            Level::Medium => 48,
            Level::Hard => 54,
        }
    }
}

pub struct Game {
    pub solution: [u8; 81],
    pub given: [bool; 81],
    pub cells: [u8; 81],
    pub selected: Option<usize>,
    pub level: Level,
    /// Depois de "Conferir": casas erradas ficam marcadas até serem mudadas.
    pub wrong: [bool; 81],
}

impl Game {
    pub fn new(level: Level) -> Self {
        let mut rng = Rng::new();
        let mut sol = [0u8; 81];
        fill(&mut sol, &mut rng);
        let mut puzzle = sol;
        let mut order: Vec<usize> = (0..81).collect();
        rng.shuffle(&mut order);
        let mut removed = 0;
        for i in order {
            if removed >= level.holes() {
                break;
            }
            let keep = puzzle[i];
            puzzle[i] = 0;
            let mut probe = puzzle;
            if count(&mut probe, 2) == 1 {
                removed += 1;
            } else {
                puzzle[i] = keep;
            }
        }
        let given = std::array::from_fn(|i| puzzle[i] != 0);
        Game { solution: sol, given, cells: puzzle, selected: None, level, wrong: [false; 81] }
    }

    pub fn solved(&self) -> bool {
        self.cells == self.solution
    }

    pub fn filled(&self) -> usize {
        self.cells.iter().filter(|&&v| v != 0).count()
    }

    pub fn set(&mut self, n: u8) {
        if let Some(i) = self.selected {
            if !self.given[i] {
                self.cells[i] = n;
                self.wrong[i] = false;
            }
        }
    }

    pub fn check(&mut self) -> usize {
        let mut bad = 0;
        for i in 0..81 {
            self.wrong[i] = self.cells[i] != 0 && self.cells[i] != self.solution[i];
            bad += self.wrong[i] as usize;
        }
        bad
    }

    fn path() -> std::path::PathBuf {
        crate::home_dir().join("data/sudoku.txt")
    }

    pub fn save(&self) {
        let enc = |a: &[u8; 81]| a.iter().map(|d| (b'0' + d) as char).collect::<String>();
        let given: String = self.given.iter().map(|&g| if g { '1' } else { '0' }).collect();
        let _ = std::fs::write(Self::path(), format!("{}\n{}\n{}\n{}\n", self.level.name(), enc(&self.solution), given, enc(&self.cells)));
    }

    pub fn load() -> Option<Self> {
        let s = std::fs::read_to_string(Self::path()).ok()?;
        let l: Vec<&str> = s.lines().collect();
        let first = *l.first()?;
        let level = [Level::Easy, Level::Medium, Level::Hard].into_iter().find(|lv| lv.name() == first)?;
        let dec = |t: &str| -> Option<[u8; 81]> {
            let v: Vec<u8> = t.bytes().map(|b| b.wrapping_sub(b'0')).collect();
            (v.len() == 81 && v.iter().all(|&d| d <= 9)).then(|| std::array::from_fn(|i| v[i]))
        };
        let solution = dec(l.get(1)?)?;
        let given: Vec<bool> = l.get(2)?.chars().map(|c| c == '1').collect();
        let cells = dec(l.get(3)?)?;
        (given.len() == 81).then(|| Game { solution, given: std::array::from_fn(|i| given[i]), cells, selected: None, level, wrong: [false; 81] })
    }

    pub fn clear_saved() {
        let _ = std::fs::remove_file(Self::path());
    }
}

// ---------- Desenho ----------

pub fn cell_rect(i: usize) -> Rect {
    Rect::new(GRID_X + (i % 9) as i32 * CELL, GRID_Y + (i / 9) as i32 * CELL, CELL, CELL)
}

/// Teclado numérico: 1–9 e apagar (0).
pub fn key_rect(n: u8) -> Rect {
    let (w, gap) = (84, 8);
    let total = 10 * w + 9 * gap;
    let x0 = (W - total) / 2;
    let k = if n == 0 { 9 } else { n as i32 - 1 };
    Rect::new(x0 + k * (w + gap), GRID_Y + 9 * CELL + 28, w, 100)
}

pub fn check_rect() -> Rect {
    Rect::new(MARGIN, 1322, (W - 2 * MARGIN - 24) / 2, 96)
}
pub fn new_game_rect() -> Rect {
    let r = check_rect();
    Rect::new(r.x + r.w + 24, r.y, r.w, r.h)
}
pub fn level_rect(k: usize) -> Rect {
    Rect::new(MARGIN, 520 + k as i32 * 150, W - 2 * MARGIN, 120)
}
pub fn continue_rect() -> Rect {
    Rect::new(MARGIN, 1000, W - 2 * MARGIN, 120)
}

fn pill(fb: &mut Framebuffer, f: &Fonts, r: Rect, label: &str, filled: bool) {
    fb.fill(r, PAPER);
    let fg = if filled {
        fb.round_rect(r, r.h / 2, INK);
        PAPER
    } else {
        fb.round_outline(r, r.h / 2, 4, INK, PAPER);
        INK
    };
    let tw = f.measure(Face::Strong, 38.0, label);
    f.draw(fb, Face::Strong, 38.0, r.x + (r.w - tw) / 2, r.y + r.h / 2 + 14, fg, label);
}

pub fn draw_menu(fb: &mut Framebuffer, f: &Fonts, saved: Option<&Game>) {
    fb.fill(fb.screen(), PAPER);
    ui::draw_back(fb, f, false);
    f.draw(fb, Face::Display, 150.0, MARGIN - 6, 330, INK, "Sudoku");
    f.draw(fb, Face::Strong, 34.0, MARGIN, 420, MUTED, "Escolha o nível de um jogo novo");
    for (k, lv) in [Level::Easy, Level::Medium, Level::Hard].iter().enumerate() {
        pill(fb, f, level_rect(k), lv.name(), false);
    }
    if let Some(g) = saved {
        pill(fb, f, continue_rect(), &format!("Continuar · {} · {}/81", g.level.name(), g.filled()), true);
    }
}

pub fn draw_board(fb: &mut Framebuffer, f: &Fonts, g: &Game, note: &str) {
    fb.fill(fb.screen(), PAPER);
    ui::draw_back(fb, f, false);
    let title = format!("Sudoku · {}", g.level.name());
    let tw = f.measure(Face::Strong, 34.0, &title);
    f.draw(fb, Face::Strong, 34.0, W - MARGIN - tw, 128, INK, &title);
    f.draw(fb, Face::Body, 32.0, GRID_X, GRID_Y - 40, MUTED, note);
    for i in 0..81 {
        draw_cell(fb, f, g, i);
    }
    draw_grid_lines(fb);
    for n in 0..=9u8 {
        let r = key_rect(n);
        fb.round_outline(r, 18, 4, INK, PAPER);
        if n == 0 {
            // Apagar: um "x".
            let (cx, cy) = (r.x as f32 + r.w as f32 / 2.0, r.y as f32 + r.h as f32 / 2.0);
            ui::stroke(fb, (cx - 16.0, cy - 16.0), (cx + 16.0, cy + 16.0), 6.0, INK);
            ui::stroke(fb, (cx + 16.0, cy - 16.0), (cx - 16.0, cy + 16.0), 6.0, INK);
        } else {
            let s = n.to_string();
            let w = f.measure(Face::Display, 56.0, &s);
            f.draw(fb, Face::Display, 56.0, r.x + (r.w - w) / 2, r.y + 70, INK, &s);
        }
    }
    pill(fb, f, check_rect(), "Conferir", false);
    pill(fb, f, new_game_rect(), "Novo jogo", false);
}

pub fn draw_cell(fb: &mut Framebuffer, f: &Fonts, g: &Game, i: usize) {
    let r = cell_rect(i);
    let inner = Rect::new(r.x + 3, r.y + 3, r.w - 6, r.h - 6);
    let sel = g.selected == Some(i);
    // Destaque leve para a linha, a coluna e o bloco da casa escolhida.
    let related = g.selected.is_some_and(|s| s / 9 == i / 9 || s % 9 == i % 9 || (s / 27 == i / 27 && s % 9 / 3 == i % 9 / 3));
    let bg = if sel { INK } else if related { 0xE8 } else { PAPER };
    fb.fill(inner, bg);
    let v = g.cells[i];
    if v != 0 {
        let s = v.to_string();
        let (face, ink) = if g.given[i] { (Face::Display, if sel { PAPER } else { INK }) } else { (Face::Body, if sel { PAPER } else { 0x33 }) };
        let w = f.measure(face, 62.0, &s);
        f.draw(fb, face, 62.0, r.x + (r.w - w) / 2, r.y + 71, ink, &s);
    }
    if g.wrong[i] {
        // Erro marcado com um traço embaixo do número.
        fb.fill(Rect::new(r.x + 26, r.y + r.h - 18, r.w - 52, 6), if sel { PAPER } else { INK });
    }
}

pub fn draw_grid_lines(fb: &mut Framebuffer) {
    for k in 0..=9 {
        let t = if k % 3 == 0 { 6 } else { 2 };
        fb.fill(Rect::new(GRID_X + k * CELL - t / 2, GRID_Y - 3, t, 9 * CELL + 6), INK);
        fb.fill(Rect::new(GRID_X - 3, GRID_Y + k * CELL - t / 2, 9 * CELL + 6, t), INK);
    }
}

pub fn grid_rect() -> Rect {
    Rect::new(GRID_X - 4, GRID_Y - 4, 9 * CELL + 8, 9 * CELL + 8)
}

pub fn note_rect() -> Rect {
    Rect::new(0, GRID_Y - 90, W, 70)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gera_jogo_valido_com_solucao_unica() {
        let g = Game::new(Level::Medium);
        let mut p = g.cells;
        assert_eq!(count(&mut p, 2), 1);
        for i in 0..81 {
            if g.given[i] {
                assert_eq!(g.cells[i], g.solution[i]);
            }
        }
        assert!(81 - g.filled() >= 40);
    }
}
