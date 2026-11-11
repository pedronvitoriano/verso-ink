//! Coroas: uma coroa por linha, por coluna e por reino (região), e coroas não podem se tocar,
//! nem na diagonal. Grades geradas com solução única. Toque: vazio → X → coroa → vazio.

use crate::fb::{Framebuffer, Rect};
use crate::minigame::{self, MiniGame, Redraw, Rng, W};
use crate::text::Fonts;
use crate::ui::{self, INK, MUTED, PAPER};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    Empty,
    X,
    Crown,
}

pub struct Coroas {
    pub n: usize,
    pub region: Vec<u8>,
    pub solution: Vec<usize>,
    pub marks: Vec<Mark>,
    undo: Vec<(usize, Mark)>,
    menu: bool,
    started: std::time::Instant,
    pub won: bool,
}

const SIZES: [(usize, &str); 6] = [(5, "Mini · 5×5"), (6, "Fácil · 6×6"), (7, "Médio · 7×7"), (8, "Médio+ · 8×8"), (9, "Difícil · 9×9"), (10, "Mestre · 10×10")];

/// Uma coroa por linha, colunas sem repetir e sem encostar na linha vizinha.
fn place(n: usize, rng: &mut Rng) -> Vec<usize> {
    fn go(r: usize, n: usize, cols: &mut Vec<usize>, used: &mut [bool], rng: &mut Rng) -> bool {
        if r == n {
            return true;
        }
        let mut order: Vec<usize> = (0..n).collect();
        rng.shuffle(&mut order);
        for c in order {
            if used[c] || (r > 0 && cols[r - 1].abs_diff(c) <= 1) {
                continue;
            }
            used[c] = true;
            cols.push(c);
            if go(r + 1, n, cols, used, rng) {
                return true;
            }
            cols.pop();
            used[c] = false;
        }
        false
    }
    let mut cols = Vec::new();
    go(0, n, &mut cols, &mut vec![false; n], rng);
    cols
}

/// Reinos crescendo a partir de cada coroa até cobrir a grade.
fn grow(n: usize, sol: &[usize], rng: &mut Rng) -> Vec<u8> {
    let mut reg = vec![u8::MAX; n * n];
    for (r, &c) in sol.iter().enumerate() {
        reg[r * n + c] = r as u8;
    }
    let mut left = n * n - n;
    while left > 0 {
        let i = (rng.next() % (n * n) as u64) as usize;
        if reg[i] != u8::MAX {
            continue;
        }
        let (r, c) = (i / n, i % n);
        let mut nb = Vec::new();
        if r > 0 { nb.push(i - n); }
        if r + 1 < n { nb.push(i + n); }
        if c > 0 { nb.push(i - 1); }
        if c + 1 < n { nb.push(i + 1); }
        let owned: Vec<u8> = nb.iter().map(|&j| reg[j]).filter(|&g| g != u8::MAX).collect();
        if !owned.is_empty() {
            reg[i] = owned[(rng.next() % owned.len() as u64) as usize];
            left -= 1;
        }
    }
    reg
}

/// Conta soluções (até `limit`). Usada nos testes para conferir a unicidade.
#[cfg_attr(not(test), allow(dead_code))]
pub fn count(n: usize, reg: &[u8], limit: u32) -> u32 {
    fn go(r: usize, n: usize, reg: &[u8], cols: &mut [bool], regs: &mut [bool], prev: Option<usize>, limit: u32) -> u32 {
        if r == n {
            return 1;
        }
        let mut t = 0;
        for c in 0..n {
            let g = reg[r * n + c] as usize;
            if cols[c] || regs[g] || prev.is_some_and(|p| p.abs_diff(c) <= 1) {
                continue;
            }
            cols[c] = true;
            regs[g] = true;
            t += go(r + 1, n, reg, cols, regs, Some(c), limit - t);
            cols[c] = false;
            regs[g] = false;
            if t >= limit {
                break;
            }
        }
        t
    }
    go(0, n, reg, &mut vec![false; n], &mut vec![false; n], None, limit)
}

/// Uma solução diferente de `sol`, se houver.
fn other_solution(n: usize, reg: &[u8], sol: &[usize]) -> Option<Vec<usize>> {
    fn go(r: usize, n: usize, reg: &[u8], sol: &[usize], cols: &mut [bool], regs: &mut [bool], cur: &mut Vec<usize>) -> bool {
        if r == n {
            return cur.as_slice() != sol;
        }
        for c in 0..n {
            let g = reg[r * n + c] as usize;
            if cols[c] || regs[g] || cur.last().is_some_and(|&p| p.abs_diff(c) <= 1) {
                continue;
            }
            cols[c] = true;
            regs[g] = true;
            cur.push(c);
            if go(r + 1, n, reg, sol, cols, regs, cur) {
                return true;
            }
            cur.pop();
            cols[c] = false;
            regs[g] = false;
        }
        false
    }
    let mut cur = Vec::new();
    go(0, n, reg, sol, &mut vec![false; n], &mut vec![false; n], &mut cur).then_some(cur)
}

fn neighbors(n: usize, i: usize) -> impl Iterator<Item = usize> {
    let (r, c) = (i / n, i % n);
    [(r > 0).then(|| i - n), (r + 1 < n).then(|| i + n), (c > 0).then(|| i - 1), (c + 1 < n).then(|| i + 1)].into_iter().flatten()
}

/// O reino `g` continua inteiro (ligado) sem a casa `skip`?
fn connected_without(n: usize, reg: &[u8], g: u8, skip: usize) -> bool {
    let cells: Vec<usize> = (0..n * n).filter(|&i| reg[i] == g && i != skip).collect();
    let Some(&first) = cells.first() else { return false };
    let mut seen = vec![false; n * n];
    let mut stack = vec![first];
    let mut k = 0;
    while let Some(i) = stack.pop() {
        if seen[i] {
            continue;
        }
        seen[i] = true;
        k += 1;
        stack.extend(neighbors(n, i).filter(|&j| reg[j] == g && j != skip && !seen[j]));
    }
    k == cells.len()
}

/// Gera reinos com solução única: cresce os reinos a partir das coroas e, enquanto houver
/// outra solução, passa uma casa dela para um reino vizinho (o que mata aquela solução).
pub fn generate(n: usize) -> (Vec<u8>, Vec<usize>) {
    let mut rng = Rng::new();
    loop {
        let sol = place(n, &mut rng);
        let mut reg = grow(n, &sol, &mut rng);
        for _ in 0..n * n * 2 {
            let Some(alt) = other_solution(n, &reg, &sol) else { return (reg, sol) };
            let rows: Vec<usize> = (0..n).filter(|&r| alt[r] != sol[r]).collect();
            let mut moved = false;
            let mut order = rows.clone();
            rng.shuffle(&mut order);
            for r in order {
                let a = r * n + alt[r];
                let g = reg[a];
                let mut opts: Vec<u8> = neighbors(n, a).map(|j| reg[j]).filter(|&h| h != g).collect();
                opts.dedup();
                if opts.is_empty() || !connected_without(n, &reg, g, a) {
                    continue;
                }
                reg[a] = opts[(rng.next() % opts.len() as u64) as usize];
                moved = true;
                break;
            }
            if !moved {
                break;
            }
        }
    }
}

impl Coroas {
    pub fn new() -> Self {
        Coroas { n: 0, region: Vec::new(), solution: Vec::new(), marks: Vec::new(), undo: Vec::new(), menu: true, started: std::time::Instant::now(), won: false }
    }

    fn start(&mut self, n: usize) {
        let (region, solution) = generate(n);
        *self = Coroas { n, region, solution, marks: vec![Mark::Empty; n * n], undo: Vec::new(), menu: false, started: std::time::Instant::now(), won: false };
    }

    /// Muda uma casa (com desfazer) e confere a vitória.
    fn set(&mut self, i: usize, m: Mark) -> Redraw {
        self.undo.push((i, self.marks[i]));
        self.marks[i] = m;
        self.won = self.crowns().len() == self.n && self.conflicts().is_empty();
        if self.won {
            Redraw::Full
        } else {
            Redraw::Part(Rect::new(0, 170, W, self.board_rect().y + self.board_rect().h - 170))
        }
    }

    fn crowns(&self) -> Vec<usize> {
        (0..self.n * self.n).filter(|&i| self.marks[i] == Mark::Crown).collect()
    }

    /// Coroas que quebram alguma regra (mesma linha, coluna, reino ou vizinhas).
    pub fn conflicts(&self) -> Vec<usize> {
        let n = self.n;
        let cr = self.crowns();
        cr.iter()
            .copied()
            .filter(|&a| {
                cr.iter().any(|&b| {
                    a != b && (a / n == b / n || a % n == b % n || self.region[a] == self.region[b] || ((a / n).abs_diff(b / n) <= 1 && (a % n).abs_diff(b % n) <= 1))
                })
            })
            .collect()
    }

    fn geom(&self) -> (i32, i32, i32) {
        minigame::grid_geom(self.n as i32, self.n as i32, 290, 940)
    }

    fn cell_rect(&self, i: usize) -> Rect {
        let (x0, y0, c) = self.geom();
        Rect::new(x0 + (i % self.n) as i32 * c, y0 + (i / self.n) as i32 * c, c, c)
    }

    fn board_rect(&self) -> Rect {
        let (x0, y0, c) = self.geom();
        Rect::new(x0 - 6, y0 - 6, c * self.n as i32 + 12, c * self.n as i32 + 12)
    }

    /// Tons claros para os reinos, vizinhos sempre diferentes.
    fn tones(&self) -> Vec<u8> {
        const PALETTE: [u8; 5] = [0xFF, 0xE4, 0xCC, 0xF2, 0xD8];
        let n = self.n;
        let mut tone = vec![255usize; n];
        for g in 0..n {
            let mut used = [false; 5];
            for i in 0..n * n {
                if self.region[i] as usize != g {
                    continue;
                }
                let (r, c) = (i / n, i % n);
                for (dr, dc) in [(0i32, 1i32), (1, 0), (0, -1), (-1, 0)] {
                    let (rr, cc) = (r as i32 + dr, c as i32 + dc);
                    if rr >= 0 && cc >= 0 && (rr as usize) < n && (cc as usize) < n {
                        let o = self.region[rr as usize * n + cc as usize] as usize;
                        if o != g && tone[o] < 5 {
                            used[tone[o]] = true;
                        }
                    }
                }
            }
            tone[g] = (0..5).find(|&k| !used[k]).unwrap_or(g % 5);
        }
        tone.iter().map(|&t| PALETTE[t]).collect()
    }

    fn draw_crown(fb: &mut Framebuffer, r: Rect, v: u8) {
        // Desenho em 0..1 ampliado a partir do centro, para a coroa encher a casa.
        let k = 1.4;
        let (s, x, y) = (r.w as f32 * k, r.x as f32 - r.w as f32 * (k - 1.0) / 2.0, r.y as f32 - r.w as f32 * (k - 1.0) / 2.0);
        let pts = [(0.2, 0.72), (0.2, 0.38), (0.36, 0.55), (0.5, 0.28), (0.64, 0.55), (0.8, 0.38), (0.8, 0.72)];
        for w in pts.windows(2) {
            ui::stroke(fb, (x + w[0].0 * s, y + w[0].1 * s), (x + w[1].0 * s, y + w[1].1 * s), s * 0.08, v);
        }
        fb.fill(Rect::new((x + 0.2 * s) as i32, (y + 0.66 * s) as i32, (0.6 * s) as i32, (0.1 * s) as i32), v);
        // Preenche o corpo da coroa.
        for k in 0..8 {
            let yy = y + (0.5 + k as f32 * 0.025) * s;
            ui::stroke(fb, (x + 0.24 * s, yy), (x + 0.76 * s, yy), s * 0.05, v);
        }
    }
}

impl MiniGame for Coroas {
    fn draw(&self, fb: &mut Framebuffer, f: &Fonts) {
        fb.fill(fb.screen(), PAPER);
        ui::draw_back(fb, f, false);
        if self.menu {
            let opts: Vec<&str> = SIZES.iter().map(|s| s.1).collect();
            minigame::draw_menu(fb, f, "Coroas", "Uma por linha, coluna e reino. Sem encostar.", &opts);
            return;
        }
        let status = if self.won {
            format!("Resolvido em {} min!", self.started.elapsed().as_secs() / 60 + 1)
        } else {
            let c = self.conflicts().len();
            let placed = self.crowns().len();
            if c > 0 { "Há coroas em conflito (sublinhadas).".to_string() } else { format!("{placed} de {} coroas", self.n) }
        };
        minigame::header(fb, f, "Coroas", &status);
        let tones = self.tones();
        for i in 0..self.n * self.n {
            let r = self.cell_rect(i);
            fb.fill(r, tones[self.region[i] as usize]);
        }
        // Linhas finas entre casas e grossas entre reinos.
        let n = self.n;
        let (x0, y0, c) = self.geom();
        for i in 0..n * n {
            let r = self.cell_rect(i);
            let (row, col) = (i / n, i % n);
            let right_border = col + 1 == n || self.region[i] != self.region[i + 1];
            let down_border = row + 1 == n || self.region[i] != self.region[i + n];
            fb.fill(Rect::new(r.x + r.w - if right_border { 3 } else { 1 }, r.y, if right_border { 6 } else { 2 }, r.h), INK);
            fb.fill(Rect::new(r.x, r.y + r.h - if down_border { 3 } else { 1 }, r.w, if down_border { 6 } else { 2 }), INK);
        }
        fb.fill(Rect::new(x0 - 3, y0 - 3, c * n as i32 + 6, 6), INK);
        fb.fill(Rect::new(x0 - 3, y0 - 3, 6, c * n as i32 + 6), INK);
        let conflicts = self.conflicts();
        for i in 0..n * n {
            let r = self.cell_rect(i);
            let inner = Rect::new(r.x + c / 8, r.y + c / 8, c - c / 4, c - c / 4);
            match self.marks[i] {
                Mark::X => {
                    let (cx, cy, d) = (r.x as f32 + c as f32 / 2.0, r.y as f32 + c as f32 / 2.0, c as f32 * 0.12);
                    ui::stroke(fb, (cx - d, cy - d), (cx + d, cy + d), 3.0, MUTED);
                    ui::stroke(fb, (cx + d, cy - d), (cx - d, cy + d), 3.0, MUTED);
                }
                Mark::Crown => {
                    Self::draw_crown(fb, inner, INK);
                    if conflicts.contains(&i) {
                        fb.fill(Rect::new(r.x + c / 5, r.y + c - c / 8, c - 2 * c / 5, 6), INK);
                    }
                }
                Mark::Empty => {}
            }
        }
        for (k, l) in ["Desfazer", "Limpar", "Dica", "Novo"].iter().enumerate() {
            minigame::pill(fb, f, minigame::bottom_button(k, 4), l, false);
        }
        minigame::hint(fb, f, "Toque ou arraste: marca X. Segure: coroa.");
    }

    fn hold(&mut self, id: u32) -> Option<Redraw> {
        let i = id as usize;
        if self.menu || self.won || i >= self.n * self.n {
            return None;
        }
        let next = if self.marks[i] == Mark::Crown { Mark::Empty } else { Mark::Crown };
        Some(self.set(i, next))
    }

    fn drag(&mut self, id: u32) -> Option<Redraw> {
        let i = id as usize;
        if self.menu || self.won || i >= self.n * self.n {
            return None;
        }
        // Passar o dedo só marca X em casas vazias; coroas ficam.
        if self.marks[i] == Mark::Empty {
            self.undo.push((i, Mark::Empty));
            self.marks[i] = Mark::X;
        }
        Some(Redraw::Part(self.cell_rect(i)))
    }

    fn targets(&self) -> Vec<(u32, Rect)> {
        if self.menu {
            return (0..SIZES.len()).map(|k| (900 + k as u32, minigame::menu_row(k))).collect();
        }
        let mut v: Vec<(u32, Rect)> = (0..4).map(|k| (800 + k as u32, minigame::bottom_button(k, 4))).collect();
        if !self.won {
            v.extend((0..self.n * self.n).map(|i| (i as u32, self.cell_rect(i))));
        }
        v
    }

    fn tap(&mut self, id: u32, _x: i32, _y: i32) -> Redraw {
        match id {
            900..=905 => {
                self.start(SIZES[(id - 900) as usize].0);
                Redraw::Full
            }
            800 => {
                if let Some((i, m)) = self.undo.pop() {
                    self.marks[i] = m;
                }
                Redraw::Part(Rect::new(0, 170, W, 1448 - 170))
            }
            801 => {
                self.marks.iter_mut().for_each(|m| *m = Mark::Empty);
                self.undo.clear();
                Redraw::Part(Rect::new(0, 170, W, 1448 - 170))
            }
            802 => {
                // Dica: coloca uma coroa certa que ainda falta (e tira as erradas daquela linha).
                let n = self.n;
                if let Some(r) = (0..n).find(|&r| self.marks[r * n + self.solution[r]] != Mark::Crown) {
                    for c in 0..n {
                        if self.marks[r * n + c] == Mark::Crown {
                            self.marks[r * n + c] = Mark::Empty;
                        }
                    }
                    self.undo.push((r * n + self.solution[r], self.marks[r * n + self.solution[r]]));
                    self.marks[r * n + self.solution[r]] = Mark::Crown;
                }
                self.won = self.crowns().len() == n && self.conflicts().is_empty();
                Redraw::Part(Rect::new(0, 170, W, 1448 - 170))
            }
            803 => {
                self.menu = true;
                Redraw::Full
            }
            i if (i as usize) < self.n * self.n => {
                // Toque: marca X (ou limpa o que houver).
                let i = i as usize;
                let next = if self.marks[i] == Mark::Empty { Mark::X } else { Mark::Empty };
                self.set(i, next)
            }
            _ => Redraw::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn grade_com_solucao_unica_e_regras() {
        for n in (5..=10).chain(5..=10).chain(9..=10) {
            let (reg, sol) = generate(n);
            assert_eq!(count(n, &reg, 2), 1);
            // n reinos, cada um ligado, cada um com uma coroa da solução.
            for g in 0..n as u8 {
                assert!(reg.contains(&g));
                assert!(connected_without(n, &reg, g, usize::MAX));
            }
            assert!(reg.iter().all(|&g| (g as usize) < n));
            let mut g = Coroas::new();
            g.n = n;
            g.region = reg;
            g.solution = sol.clone();
            g.marks = vec![Mark::Empty; n * n];
            for (r, &c) in sol.iter().enumerate() {
                g.marks[r * n + c] = Mark::Crown;
            }
            assert!(g.conflicts().is_empty());
        }
    }
}

#[cfg(test)]
mod tempo {
    #[test]
    #[ignore]
    fn tempo_por_tamanho() {
        for n in 5..=10 {
            let t = std::time::Instant::now();
            for _ in 0..5 {
                super::generate(n);
            }
            eprintln!("{n}x{n}: {:?} por grade", t.elapsed() / 5);
        }
    }
}
