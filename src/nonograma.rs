//! Nonograma: pinte a grade pelas dicas das linhas e colunas. Desenhos gerados ao acaso e aceitos
//! só se der para resolver pela lógica das linhas (sem chute).

use crate::fb::{Framebuffer, Rect};
use crate::fases;
use crate::minigame::{self, MiniGame, Redraw, Rng, MARGIN, W};
use crate::text::{Face, Fonts};
use crate::ui::{self, INK, MUTED, PAPER};

const LEVELS: [(usize, &str); 3] = [(5, "Fácil · 5×5"), (8, "Médio · 8×8"), (10, "Difícil · 10×10")];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Cell {
    Empty,
    Fill,
    X,
}

pub struct Nonograma {
    menu: bool,
    n: usize,
    sol: Vec<bool>,
    grid: Vec<Cell>,
    rows: Vec<Vec<usize>>,
    cols: Vec<Vec<usize>>,
    won: bool,
    /// Fase em jogo (0 = treino livre).
    fase: u32,
    practice: bool,
    prog: fases::Progress,
}

/// Tamanho e quanto da grade é pintado: menos pintura deixa as dicas mais curtas e o
/// raciocínio mais longo. Difícil e desafio são maiores.
fn fase_params(k: u32) -> (usize, u64) {
    let mut n = (5 + (k.saturating_sub(1) / 3) as usize).min(10);
    let mut fill = 62.0 - 8.0 * fases::ramp(k, 25);
    match fases::kind(k) {
        fases::Kind::Hard => {
            n = (n + 1).min(11);
            fill -= 4.0;
        }
        fases::Kind::Challenge => {
            n = (n + 2).min(12);
            fill -= 6.0;
        }
        fases::Kind::Normal => {}
    }
    (n, fill as u64)
}

fn clue(line: &[bool]) -> Vec<usize> {
    let mut v = Vec::new();
    let mut run = 0;
    for &b in line {
        if b {
            run += 1;
        } else if run > 0 {
            v.push(run);
            run = 0;
        }
    }
    if run > 0 {
        v.push(run);
    }
    v
}

/// Todas as linhas compatíveis com a dica e com o que já se sabe (Some(true/false)).
fn fits(n: usize, c: &[usize], known: &[Option<bool>]) -> Vec<Vec<bool>> {
    fn go(n: usize, c: &[usize], known: &[Option<bool>], pos: usize, cur: &mut Vec<bool>, out: &mut Vec<Vec<bool>>) {
        if out.len() > 2000 {
            return;
        }
        if c.is_empty() {
            let mut line = cur.clone();
            line.resize(n, false);
            if line.iter().zip(known).all(|(&b, k)| k.is_none_or(|k| k == b)) {
                out.push(line);
            }
            return;
        }
        let need: usize = c.iter().sum::<usize>() + c.len() - 1;
        for start in pos..=n.saturating_sub(need) {
            let mut line = cur.clone();
            line.resize(start, false);
            line.extend(std::iter::repeat_n(true, c[0]));
            if c.len() > 1 {
                line.push(false);
            }
            if line.iter().enumerate().all(|(i, &b)| known[i].is_none_or(|k| k == b)) {
                let next = line.len();
                go(n, &c[1..], known, next, &mut line, out);
            }
        }
    }
    let mut out = Vec::new();
    go(n, c, known, 0, &mut Vec::new(), &mut out);
    out
}

/// Resolve só pela lógica das linhas; devolve true se determinou todas as casas.
pub fn line_solvable(n: usize, rows: &[Vec<usize>], cols: &[Vec<usize>]) -> bool {
    let mut g: Vec<Option<bool>> = vec![None; n * n];
    loop {
        let mut changed = false;
        for horizontal in [true, false] {
            for k in 0..n {
                let idx: Vec<usize> = (0..n).map(|j| if horizontal { k * n + j } else { j * n + k }).collect();
                let known: Vec<Option<bool>> = idx.iter().map(|&i| g[i]).collect();
                let all = fits(n, if horizontal { &rows[k] } else { &cols[k] }, &known);
                if all.is_empty() {
                    return false;
                }
                for j in 0..n {
                    if g[idx[j]].is_none() && all.iter().all(|l| l[j] == all[0][j]) {
                        g[idx[j]] = Some(all[0][j]);
                        changed = true;
                    }
                }
            }
        }
        if g.iter().all(Option::is_some) {
            return true;
        }
        if !changed {
            return false;
        }
    }
}

impl Nonograma {
    pub fn new() -> Self {
        Nonograma { menu: true, n: 0, sol: vec![], grid: vec![], rows: vec![], cols: vec![], won: false, fase: 0, practice: false, prog: fases::Progress::load("nonograma") }
    }

    fn start(&mut self, n: usize) {
        self.start_with(n, 58);
        self.fase = 0;
    }

    fn start_fase(&mut self) {
        let k = self.prog.level;
        let (n, fill) = fase_params(k);
        self.start_with(n, fill);
        self.fase = k;
    }

    fn start_with(&mut self, n: usize, fill: u64) {
        let mut rng = Rng::new();
        loop {
            let sol: Vec<bool> = (0..n * n).map(|_| rng.next() % 100 < fill).collect();
            let rows: Vec<Vec<usize>> = (0..n).map(|r| clue(&sol[r * n..r * n + n])).collect();
            let cols: Vec<Vec<usize>> = (0..n).map(|c| clue(&(0..n).map(|r| sol[r * n + c]).collect::<Vec<_>>())).collect();
            if line_solvable(n, &rows, &cols) {
                self.menu = false;
                self.practice = false;
                self.n = n;
                self.sol = sol;
                self.grid = vec![Cell::Empty; n * n];
                self.rows = rows;
                self.cols = cols;
                self.won = false;
                return;
            }
        }
    }

    /// Espaço das dicas à esquerda e em cima; o resto é a grade.
    fn geom(&self) -> (i32, i32, i32) {
        let n = self.n as i32;
        let max_clues = (self.n as i32 + 1) / 2;
        let s = ((W - 2 * 40) / (n + max_clues)).min(1000 / (n + max_clues));
        let x0 = (W - s * (n + max_clues)) / 2 + s * max_clues;
        let y0 = 270 + s * max_clues;
        (x0, y0, s)
    }

    fn cell(&self, i: usize) -> Rect {
        let (x0, y0, s) = self.geom();
        Rect::new(x0 + (i % self.n) as i32 * s, y0 + (i / self.n) as i32 * s, s, s)
    }

    fn check_win(&mut self) {
        self.won = (0..self.n * self.n).all(|i| (self.grid[i] == Cell::Fill) == self.sol[i]);
        if self.won && self.fase > 0 {
            self.prog.record_win(self.fase, fases::today());
        }
    }
}

impl MiniGame for Nonograma {
    fn draw(&self, fb: &mut Framebuffer, f: &Fonts) {
        fb.fill(fb.screen(), PAPER);
        ui::draw_back(fb, f, false);
        if self.menu && self.practice {
            let opts: Vec<&str> = LEVELS.iter().map(|l| l.1).collect();
            minigame::draw_menu(fb, f, "Treino", "Escolha o tamanho. Não conta para as fases.", &opts);
            return;
        }
        if self.menu {
            let main = format!("Fase {}", self.prog.level);
            fases::draw_menu(fb, f, "Nonograma", "Os números dizem os blocos pintados de cada linha.", &self.prog, &main, &["Treino livre"]);
            return;
        }
        let title = if self.fase > 0 { fases::title(self.fase) } else { "Nonograma".to_string() };
        minigame::header(fb, f, &title, if self.won { "Resolvido!" } else { "Toque: pinta, marca X, limpa." });
        let (x0, y0, s) = self.geom();
        let n = self.n as i32;
        let fs = (s as f32 * 0.5).min(34.0);
        for r in 0..self.n {
            let txt: Vec<String> = self.rows[r].iter().map(usize::to_string).collect();
            let t = if txt.is_empty() { "0".to_string() } else { txt.join(" ") };
            let w = f.measure(Face::Strong, fs, &t);
            f.draw(fb, Face::Strong, fs, x0 - 12 - w, y0 + r as i32 * s + s / 2 + (fs * 0.36) as i32, INK, &t);
        }
        for c in 0..self.n {
            let col = if self.cols[c].is_empty() { vec![0] } else { self.cols[c].clone() };
            for (k, v) in col.iter().rev().enumerate() {
                let t = v.to_string();
                let w = f.measure(Face::Strong, fs, &t);
                f.draw(fb, Face::Strong, fs, x0 + c as i32 * s + (s - w) / 2, y0 - 12 - k as i32 * (fs as i32 + 6), INK, &t);
            }
        }
        for i in 0..self.n * self.n {
            let r = self.cell(i);
            match self.grid[i] {
                Cell::Fill => fb.fill(Rect::new(r.x + 2, r.y + 2, r.w - 4, r.h - 4), INK),
                Cell::X => {
                    let (cx, cy, d) = (r.x as f32 + s as f32 / 2.0, r.y as f32 + s as f32 / 2.0, s as f32 * 0.18);
                    ui::stroke(fb, (cx - d, cy - d), (cx + d, cy + d), 3.0, MUTED);
                    ui::stroke(fb, (cx + d, cy - d), (cx - d, cy + d), 3.0, MUTED);
                }
                Cell::Empty => {}
            }
        }
        for k in 0..=n {
            let t = if k % 5 == 0 { 5 } else { 2 };
            fb.fill(Rect::new(x0 + k * s - t / 2, y0, t, s * n), INK);
            fb.fill(Rect::new(x0, y0 + k * s - t / 2, s * n, t), INK);
        }
        if self.fase > 0 && self.won {
            return fases::draw_win(fb, f, &self.prog);
        }
        let last = if self.fase > 0 { "Menu" } else { "Novo jogo" };
        for (k, l) in ["Limpar", last].iter().enumerate() {
            minigame::pill(fb, f, minigame::bottom_button(k, 2), l, false);
        }
        let _ = MARGIN;
    }

    fn targets(&self) -> Vec<(u32, Rect)> {
        if self.menu && self.practice {
            return (0..LEVELS.len()).map(|k| (910 + k as u32, minigame::menu_row(k))).collect();
        }
        if self.menu {
            return vec![(900, minigame::menu_row(0)), (901, minigame::menu_row(1))];
        }
        if self.fase > 0 && self.won {
            return fases::win_targets();
        }
        let mut v = vec![(800u32, minigame::bottom_button(0, 2)), (801, minigame::bottom_button(1, 2))];
        if !self.won {
            v.extend((0..self.n * self.n).map(|i| (i as u32, self.cell(i))));
        }
        v
    }

    fn back(&mut self) -> bool {
        if self.menu && !self.practice {
            return false;
        }
        self.menu = true;
        self.practice = false;
        true
    }

    fn tap(&mut self, id: u32, _x: i32, _y: i32) -> Redraw {
        let all = Rect::new(0, 170, W, 1448 - 170);
        match id {
            900 | fases::WIN_NEXT => {
                self.start_fase();
                Redraw::Full
            }
            901 => {
                self.practice = true;
                Redraw::Full
            }
            fases::WIN_MENU => {
                self.menu = true;
                Redraw::Full
            }
            910..=912 => {
                self.start(LEVELS[(id - 910) as usize].0);
                Redraw::Full
            }
            800 => {
                self.grid.iter_mut().for_each(|c| *c = Cell::Empty);
                Redraw::Part(all)
            }
            801 => {
                self.menu = true;
                Redraw::Full
            }
            i if (i as usize) < self.n * self.n => {
                let i = i as usize;
                self.grid[i] = match self.grid[i] {
                    Cell::Empty => Cell::Fill,
                    Cell::Fill => Cell::X,
                    Cell::X => Cell::Empty,
                };
                self.check_win();
                if self.won { Redraw::Full } else { Redraw::Part(all) }
            }
            _ => Redraw::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fases_geram_grades_resolviveis_rapido() {
        for k in [1, 9, 10, 15, 20, 30, 40] {
            let t = std::time::Instant::now();
            let mut g = Nonograma { prog: fases::Progress::load_from(std::env::temp_dir().join("nono-teste.txt")), ..Nonograma::new() };
            g.prog.level = k;
            g.start_fase();
            let (n, _) = fase_params(k);
            assert_eq!(g.n, n);
            assert!(line_solvable(n, &g.rows, &g.cols));
            eprintln!("fase {k}: {n}x{n} em {:?}", t.elapsed());
        }
    }
    #[test]
    fn dicas_e_resolucao() {
        assert_eq!(clue(&[true, true, false, true]), vec![2, 1]);
        let mut g = Nonograma::new();
        g.start(8);
        assert!(line_solvable(8, &g.rows, &g.cols));
    }
}
