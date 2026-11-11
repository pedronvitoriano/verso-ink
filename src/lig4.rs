//! Lig 4: quatro em linha numa grade 7×6. Contra a CPU (minimax com poda) ou duas pessoas.

use crate::fb::{Framebuffer, Rect};
use crate::fases;
use crate::minigame::{self, MiniGame, Redraw, Rng, W};
use crate::text::Fonts;
use crate::ui::{self, INK, MUTED, PAPER};

const COLS: usize = 7;
const ROWS: usize = 6;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Menu,
    Cpu(u8),
    Local,
}

pub struct Lig4 {
    mode: Mode,
    /// 0 vazio, 1 = você (ou jogador 1, preto), 2 = CPU (ou jogador 2, branco).
    b: [[u8; COLS]; ROWS],
    turn: u8,
    moves: Vec<usize>,
    winner: Option<(u8, Vec<(usize, usize)>)>,
    rng: Rng,
    /// Fase contra a CPU (0 = treino ou duas pessoas).
    fase: u32,
    practice: bool,
    prog: fases::Progress,
}

const MENU: [&str; 4] = ["Contra a CPU · Fácil", "Contra a CPU · Médio", "Contra a CPU · Difícil", "Duas pessoas"];

fn drop_row(b: &[[u8; COLS]; ROWS], c: usize) -> Option<usize> {
    (0..ROWS).rev().find(|&r| b[r][c] == 0)
}

/// Todas as janelas de 4 casas (calculadas uma vez).
fn lines() -> &'static [Vec<(usize, usize)>] {
    static L: std::sync::OnceLock<Vec<Vec<(usize, usize)>>> = std::sync::OnceLock::new();
    L.get_or_init(build_lines)
}

fn build_lines() -> Vec<Vec<(usize, usize)>> {
    let mut v = Vec::new();
    for r in 0..ROWS as i32 {
        for c in 0..COLS as i32 {
            for (dr, dc) in [(0, 1), (1, 0), (1, 1), (1, -1)] {
                let cells: Vec<(i32, i32)> = (0..4).map(|k| (r + dr * k, c + dc * k)).collect();
                if cells.iter().all(|&(rr, cc)| rr >= 0 && cc >= 0 && rr < ROWS as i32 && cc < COLS as i32) {
                    v.push(cells.iter().map(|&(a, b)| (a as usize, b as usize)).collect());
                }
            }
        }
    }
    v
}

fn winner(b: &[[u8; COLS]; ROWS]) -> Option<(u8, Vec<(usize, usize)>)> {
    lines().iter().find_map(|l| {
        let p = b[l[0].0][l[0].1];
        (p != 0 && l.iter().all(|&(r, c)| b[r][c] == p)).then(|| (p, l.clone()))
    })
}

/// Avaliação: janelas de 4 com peças de um lado só; centro vale mais.
fn eval(b: &[[u8; COLS]; ROWS], me: u8) -> i32 {
    let op = 3 - me;
    let mut s = 0;
    for r in 0..ROWS {
        if b[r][3] == me {
            s += 3;
        } else if b[r][3] == op {
            s -= 3;
        }
    }
    for l in lines().iter() {
        let m = l.iter().filter(|&&(r, c)| b[r][c] == me).count();
        let o = l.iter().filter(|&&(r, c)| b[r][c] == op).count();
        if o == 0 {
            s += [0, 1, 5, 50, 10_000][m];
        } else if m == 0 {
            s -= [0, 1, 6, 60, 10_000][o];
        }
    }
    s
}

fn negamax(b: &mut [[u8; COLS]; ROWS], depth: u8, mut a: i32, beta: i32, me: u8) -> i32 {
    if let Some((p, _)) = winner(b) {
        return if p == me { 100_000 + depth as i32 } else { -100_000 - depth as i32 };
    }
    if depth == 0 || (0..COLS).all(|c| b[0][c] != 0) {
        return eval(b, me);
    }
    for c in [3, 2, 4, 1, 5, 0, 6] {
        if let Some(r) = drop_row(b, c) {
            b[r][c] = me;
            let v = -negamax(b, depth - 1, -beta, -a, 3 - me);
            b[r][c] = 0;
            if v > a {
                a = v;
            }
            if a >= beta {
                break;
            }
        }
    }
    a
}

impl Lig4 {
    pub fn new() -> Self {
        Lig4 { mode: Mode::Menu, b: [[0; COLS]; ROWS], turn: 1, moves: Vec::new(), winner: None, rng: Rng::new(), fase: 0, practice: false, prog: fases::Progress::load("lig4") }
    }

    fn reset(&mut self, mode: Mode, fase: u32) {
        self.mode = mode;
        self.b = [[0; COLS]; ROWS];
        self.moves.clear();
        self.winner = None;
        self.practice = false;
        self.fase = fase;
        // No desafio, a CPU começa.
        self.turn = if fase > 0 && fases::kind(fase) == fases::Kind::Challenge { 2 } else { 1 };
    }

    /// Profundidade da busca e chance (%) de jogar ao acaso.
    fn strength(&self) -> (u8, u64) {
        if self.fase == 0 {
            let Mode::Cpu(level) = self.mode else { return (2, 0) };
            return [(2, 33), (4, 0), (7, 0)][level as usize];
        }
        let k = self.fase;
        let depth = (1 + k / 2).min(7) as u8;
        let random = 40u64.saturating_sub(4 * k as u64);
        match fases::kind(k) {
            fases::Kind::Normal => (depth, random),
            fases::Kind::Hard => ((depth + 1).min(8), 0),
            fases::Kind::Challenge => (9, 0),
        }
    }

    fn geom() -> (i32, i32, i32) {
        minigame::grid_geom(COLS as i32, ROWS as i32, 330, 900)
    }

    fn col_rect(c: usize) -> Rect {
        let (x0, y0, s) = Self::geom();
        Rect::new(x0 + c as i32 * s, y0 - 70, s, s * ROWS as i32 + 70)
    }

    fn play(&mut self, c: usize) -> bool {
        let Some(r) = drop_row(&self.b, c) else { return false };
        self.b[r][c] = self.turn;
        self.moves.push(c);
        self.winner = winner(&self.b);
        if self.fase > 0 && self.winner.as_ref().is_some_and(|w| w.0 == 1) {
            self.prog.record_win(self.fase, fases::today());
        }
        self.turn = 3 - self.turn;
        true
    }

    fn full(&self) -> bool {
        (0..COLS).all(|c| self.b[0][c] != 0)
    }

    fn status(&self) -> String {
        let name = |p: u8| match (self.mode, p) {
            (Mode::Cpu(_), 1) => "Você".to_string(),
            (Mode::Cpu(_), _) => "A CPU".to_string(),
            (_, 1) => "Pretas".to_string(),
            _ => "Brancas".to_string(),
        };
        if let Some((p, _)) = &self.winner {
            let n = name(*p);
            return if n == "Você" { "Você venceu!".into() } else { format!("{n} venceu.") };
        }
        if self.full() {
            return "Empate: grade cheia.".into();
        }
        match self.mode {
            Mode::Cpu(_) if self.turn == 1 => "Sua vez: toque numa coluna.".into(),
            Mode::Cpu(_) => "CPU pensando…".into(),
            _ => format!("Vez das {}.", if self.turn == 1 { "pretas" } else { "brancas" }),
        }
    }
}

impl MiniGame for Lig4 {
    fn context(&self) -> String {
        if self.mode == Mode::Menu {
            return "No menu do Lig 4.".into();
        }
        let rows: Vec<String> = self.b.iter().map(|r| r.iter().map(|&v| match v { 1 => 'X', 2 => 'O', _ => '.' }).collect()).collect();
        format!("Lig 4 (grade 7×6, colunas 1 a 7; X = Pedro, O = adversário; a linha de baixo é a última). Vez de: {}.\n{}", if self.turn == 1 { "Pedro" } else { "adversário" }, rows.join("\n"))
    }

    fn at_menu(&self) -> bool {
        self.mode == Mode::Menu
    }

    fn draw(&self, fb: &mut Framebuffer, f: &Fonts) {
        fb.fill(fb.screen(), PAPER);
        ui::draw_back(fb, f, false);
        if self.mode == Mode::Menu && self.practice {
            minigame::draw_menu(fb, f, "Treino", "Escolha a força da CPU. Não conta para as fases.", &MENU[..3]);
            return;
        }
        if self.mode == Mode::Menu {
            let main = format!("Fase {} · contra a CPU", self.prog.level);
            fases::draw_menu(fb, f, "Lig 4", "Quatro em linha: deitado, em pé ou na diagonal.", &self.prog, &main, &["Treino contra a CPU", "Duas pessoas"]);
            return;
        }
        let title = if self.fase > 0 { fases::title(self.fase) } else { "Lig 4".to_string() };
        minigame::header(fb, f, &title, &self.status());
        let (x0, y0, s) = Self::geom();
        let board = Rect::new(x0 - 10, y0 - 10, s * COLS as i32 + 20, s * ROWS as i32 + 20);
        fb.round_rect(board, 24, 0x99);
        let win: Vec<(usize, usize)> = self.winner.as_ref().map(|w| w.1.clone()).unwrap_or_default();
        for r in 0..ROWS {
            for c in 0..COLS {
                let cx = x0 + c as i32 * s + s / 2;
                let cy = y0 + r as i32 * s + s / 2;
                let rad = s * 40 / 100;
                let disc = Rect::new(cx - rad, cy - rad, rad * 2, rad * 2);
                match self.b[r][c] {
                    1 => fb.round_rect(disc, rad, INK),
                    2 => fb.round_outline(disc, rad, 5, INK, PAPER),
                    _ => fb.round_rect(disc, rad, 0xEE),
                }
                if win.contains(&(r, c)) {
                    // Marca as quatro da vitória com um ponto no centro.
                    let d = rad / 3;
                    fb.round_rect(Rect::new(cx - d, cy - d, d * 2, d * 2), d, if self.b[r][c] == 1 { PAPER } else { INK });
                }
            }
        }
        // Última jogada: setinha sobre a coluna.
        if let Some(&c) = self.moves.last() {
            let cx = (x0 + c as i32 * s + s / 2) as f32;
            let y = (y0 - 46) as f32;
            ui::stroke(fb, (cx - 16.0, y), (cx, y + 18.0), 5.0, MUTED);
            ui::stroke(fb, (cx + 16.0, y), (cx, y + 18.0), 5.0, MUTED);
        }
        if self.fase > 0 && self.winner.as_ref().is_some_and(|w| w.0 == 1) {
            return fases::draw_win(fb, f, &self.prog);
        }
        if self.fase > 0 && (self.winner.is_some() || self.full()) {
            return fases::draw_retry(fb, f, self.fase);
        }
        // Na fase não há desfazer: o botão recomeça a partida.
        let first = if self.fase > 0 { "Recomeçar" } else { "Desfazer" };
        let second = if self.fase > 0 { "Menu" } else { "Novo jogo" };
        for (k, l) in [first, second].iter().enumerate() {
            minigame::pill(fb, f, minigame::bottom_button(k, 2), l, false);
        }
    }

    fn targets(&self) -> Vec<(u32, Rect)> {
        if self.mode == Mode::Menu && self.practice {
            return (0..3).map(|k| (910 + k as u32, minigame::menu_row(k))).collect();
        }
        if self.mode == Mode::Menu {
            return (0..3).map(|k| (900 + k as u32, minigame::menu_row(k))).collect();
        }
        if self.fase > 0 && (self.winner.is_some() || self.full()) {
            return fases::win_targets();
        }
        let mut v: Vec<(u32, Rect)> = (0..2).map(|k| (800 + k as u32, minigame::bottom_button(k, 2))).collect();
        let my_turn = !matches!(self.mode, Mode::Cpu(_)) || self.turn == 1;
        if self.winner.is_none() && my_turn {
            v.extend((0..COLS).map(|c| (c as u32, Self::col_rect(c))));
        }
        v
    }

    fn tap(&mut self, id: u32, _x: i32, _y: i32) -> Redraw {
        match id {
            900 | fases::WIN_NEXT => {
                let k = self.prog.level;
                self.reset(Mode::Cpu(1), k);
                Redraw::Full
            }
            901 => {
                self.practice = true;
                Redraw::Full
            }
            902 => {
                self.reset(Mode::Local, 0);
                Redraw::Full
            }
            fases::WIN_MENU => {
                self.mode = Mode::Menu;
                Redraw::Full
            }
            910..=912 => {
                self.reset(Mode::Cpu((id - 910) as u8), 0);
                Redraw::Full
            }
            800 if self.fase > 0 => {
                let k = self.fase;
                self.reset(Mode::Cpu(1), k);
                Redraw::Full
            }
            800 => {
                // Desfaz a sua jogada e a resposta da CPU.
                let n = if matches!(self.mode, Mode::Cpu(_)) { 2 } else { 1 };
                for _ in 0..n {
                    if let Some(c) = self.moves.pop() {
                        if let Some(r) = (0..ROWS).find(|&r| self.b[r][c] != 0) {
                            self.b[r][c] = 0;
                            self.turn = 3 - self.turn;
                        }
                    }
                }
                self.winner = None;
                Redraw::Part(Rect::new(0, 170, W, 1448 - 170))
            }
            801 => {
                self.mode = Mode::Menu;
                Redraw::Full
            }
            c if (c as usize) < COLS => {
                if self.play(c as usize) {
                    Redraw::Part(Rect::new(0, 170, W, 1448 - 170))
                } else {
                    Redraw::None
                }
            }
            _ => Redraw::None,
        }
    }

    fn back(&mut self) -> bool {
        if self.mode == Mode::Menu && !self.practice {
            return false;
        }
        self.mode = Mode::Menu;
        self.practice = false;
        true
    }

    fn cpu_turn(&self) -> bool {
        matches!(self.mode, Mode::Cpu(_)) && self.turn == 2 && self.winner.is_none() && !self.full()
    }

    fn cpu_move(&mut self) {
        if !matches!(self.mode, Mode::Cpu(_)) {
            return;
        }
        let (depth, random) = self.strength();
        // Nas fases iniciais (e no treino fácil), às vezes joga ao acaso.
        if random > 0 && self.rng.next() % 100 < random {
            let cols: Vec<usize> = (0..COLS).filter(|&c| drop_row(&self.b, c).is_some()).collect();
            let c = cols[(self.rng.next() % cols.len() as u64) as usize];
            self.play(c);
            return;
        }
        let mut best = (i32::MIN, 3);
        let mut b = self.b;
        for c in [3, 2, 4, 1, 5, 0, 6] {
            if let Some(r) = drop_row(&b, c) {
                b[r][c] = 2;
                let v = -negamax(&mut b, depth - 1, -1_000_000, 1_000_000, 1);
                b[r][c] = 0;
                if v > best.0 {
                    best = (v, c);
                }
            }
        }
        self.play(best.1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cpu_bloqueia_e_vence() {
        let mut g = Lig4 { mode: Mode::Cpu(1), ..Lig4::new() };
        for c in [0, 6, 0, 6, 0] {
            g.turn = if g.moves.len() % 2 == 0 { 1 } else { 2 };
            g.play(c);
        }
        // Você ameaça quatro na coluna 0: a CPU tem que tampar.
        g.turn = 2;
        g.cpu_move();
        assert_eq!(g.b[2][0], 2);
    }
}

#[cfg(test)]
mod tempo {
    use super::*;
    #[test]
    #[ignore]
    fn tempo_da_cpu_por_profundidade() {
        for k in [1u32, 10, 15, 20] {
            let mut g = Lig4::new();
            g.reset(Mode::Cpu(1), k);
            g.turn = 2;
            let t = std::time::Instant::now();
            g.cpu_move();
            eprintln!("fase {k} {:?}: {:?}", g.strength(), t.elapsed());
        }
    }
}
