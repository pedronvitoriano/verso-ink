//! Reversi: cerque peças em qualquer direção para virá-las.

use crate::fb::{Framebuffer, Rect};
use crate::minigame::{self, MiniGame, Redraw, Rng, MARGIN};
use crate::text::{Face, Fonts};
use crate::ui::{self, INK, PAPER};

const OPTIONS: [&str; 4] = ["Contra a CPU · fácil", "Contra a CPU · médio", "Contra a CPU · difícil", "Duas pessoas"];
const WEIGHTS: [i32; 64] = [
    120,-20,20,5,5,20,-20,120, -20,-40,-5,-5,-5,-5,-40,-20,
    20,-5,15,3,3,15,-5,20, 5,-5,3,3,3,3,-5,5,
    5,-5,3,3,3,3,-5,5, 20,-5,15,3,3,15,-5,20,
    -20,-40,-5,-5,-5,-5,-40,-20, 120,-20,20,5,5,20,-20,120,
];

#[derive(Clone, Copy)]
struct Board {
    cells: [u8; 64],
    turn: u8,
}

impl Board {
    fn new() -> Self {
        let mut cells = [0; 64];
        cells[27] = 2; cells[28] = 1; cells[35] = 1; cells[36] = 2;
        Board { cells, turn: 1 }
    }

    fn flips(&self, m: usize, player: u8) -> Vec<usize> {
        if m >= 64 || self.cells[m] != 0 { return vec![]; }
        let (row, col) = ((m / 8) as i32, (m % 8) as i32);
        let mut all = Vec::new();
        for dr in -1..=1 {
            for dc in -1..=1 {
                if dr == 0 && dc == 0 { continue; }
                let (mut r, mut c) = (row + dr, col + dc);
                let mut line = Vec::new();
                while (0..8).contains(&r) && (0..8).contains(&c) {
                    let i = (r * 8 + c) as usize;
                    if self.cells[i] == 3 - player { line.push(i); }
                    else {
                        if self.cells[i] == player { all.extend(line); }
                        break;
                    }
                    r += dr; c += dc;
                }
            }
        }
        all
    }

    fn moves(&self, player: u8) -> Vec<usize> {
        (0..64).filter(|&i| !self.flips(i, player).is_empty()).collect()
    }

    fn play(&mut self, m: usize) -> bool {
        let flips = self.flips(m, self.turn);
        if flips.is_empty() { return false; }
        self.cells[m] = self.turn;
        for i in flips { self.cells[i] = self.turn; }
        self.turn = 3 - self.turn;
        true
    }

    fn score(&self) -> (usize, usize) {
        (self.cells.iter().filter(|&&x| x == 1).count(), self.cells.iter().filter(|&&x| x == 2).count())
    }

    fn over(&self) -> bool { self.moves(1).is_empty() && self.moves(2).is_empty() }
}

fn evaluate(b: &Board, me: u8) -> i32 {
    let mut value = 0;
    for (i, &p) in b.cells.iter().enumerate() {
        if p == me { value += WEIGHTS[i]; }
        if p == 3 - me { value -= WEIGHTS[i]; }
    }
    value + 5 * (b.moves(me).len() as i32 - b.moves(3 - me).len() as i32)
}

fn minimax(b: &Board, depth: u8, me: u8, mut alpha: i32, mut beta: i32) -> i32 {
    let moves = b.moves(b.turn);
    if moves.is_empty() {
        if b.moves(3 - b.turn).is_empty() {
            let (a, c) = b.score();
            let diff = if me == 1 { a as i32 - c as i32 } else { c as i32 - a as i32 };
            return diff * 1000;
        }
        let mut next = *b;
        next.turn = 3 - next.turn;
        return minimax(&next, depth, me, alpha, beta);
    }
    if depth == 0 { return evaluate(b, me); }
    let max = b.turn == me;
    let mut value = if max { i32::MIN } else { i32::MAX };
    for m in moves {
        let mut next = *b;
        next.play(m);
        let score = minimax(&next, depth - 1, me, alpha, beta);
        if max { value = value.max(score); alpha = alpha.max(value); }
        else { value = value.min(score); beta = beta.min(value); }
        if alpha >= beta { break; }
    }
    value
}

fn choose(b: &Board, level: u8, rng: &mut Rng) -> usize {
    let moves = b.moves(b.turn);
    if level == 0 { return moves[(rng.next() % moves.len() as u64) as usize]; }
    let mut best = i32::MIN;
    let mut choices = Vec::new();
    for m in moves {
        let mut next = *b;
        next.play(m);
        let value = if level == 1 { evaluate(&next, b.turn) } else { minimax(&next, 3, b.turn, i32::MIN, i32::MAX) };
        if value > best { best = value; choices.clear(); }
        if value == best { choices.push(m); }
    }
    choices[(rng.next() % choices.len() as u64) as usize]
}

pub struct Reversi {
    menu: bool,
    level: Option<u8>,
    board: Board,
    passed: Option<u8>,
    rng: Rng,
}

impl Reversi {
    pub fn new() -> Self { Reversi { menu: true, level: Some(0), board: Board::new(), passed: None, rng: Rng::new() } }

    fn start(&mut self, level: Option<u8>) {
        self.level = level; self.board = Board::new(); self.passed = None; self.menu = false;
    }

    fn play(&mut self, m: usize) {
        if !self.board.play(m) { return; }
        self.passed = None;
        if self.board.moves(self.board.turn).is_empty() && !self.board.moves(3 - self.board.turn).is_empty() {
            self.passed = Some(self.board.turn);
            self.board.turn = 3 - self.board.turn;
        }
    }

    fn status(&self) -> String {
        if self.board.over() {
            let (a, b) = self.board.score();
            return if a == b { "Empate!".into() } else if self.level.is_some() { if a > b { "Você venceu!".into() } else { "A CPU venceu.".into() } } else if a > b { "Pretas venceram!".into() } else { "Brancas venceram!".into() };
        }
        if self.level.is_some() { if self.board.turn == 1 { "Sua vez".into() } else { "Vez da CPU".into() } }
        else if self.board.turn == 1 { "Vez das pretas".into() } else { "Vez das brancas".into() }
    }

    fn cell(i: usize) -> Rect { Rect::new(112 + (i % 8) as i32 * 106, 360 + (i / 8) as i32 * 106, 106, 106) }
    fn bottom(k: usize) -> Rect { Rect::new(MARGIN + k as i32 * 472, 1288, 456, 80) }

    #[cfg(test)]
    pub fn final_para_print() -> Self {
        let mut g = Self::new();
        g.start(None);
        g.board.cells = [1; 64];
        g.board.cells[0] = 2;
        g
    }
}

impl MiniGame for Reversi {
    fn at_menu(&self) -> bool { self.menu }
    fn summary(&self) -> String { if self.menu { String::new() } else { self.status() } }
    fn draw(&self, fb: &mut Framebuffer, f: &Fonts) {
        fb.fill(fb.screen(), PAPER);
        ui::draw_back(fb, f, false);
        if self.menu {
            minigame::draw_menu(fb, f, "Reversi", "Cerque as peças e vire o jogo.", &OPTIONS);
            if !crate::sprites::exists("icone-reversi") {
                ui::card(fb, Rect::new(MARGIN, 1100, 928, 260), 32, ui::SURFACE, false);
                for row in 0..4 {
                    for col in 0..4 {
                        let r = Rect::new(422 + col * 57, 1116 + row * 57, 55, 55);
                        fb.fill(r, 0xC6);
                        if (row == 1 || row == 2) && (col == 1 || col == 2) {
                            let d = Rect::new(r.x + 8, r.y + 8, 39, 39);
                            if row == col { fb.round_outline(d, 19, 3, INK, PAPER); }
                            else { fb.round_rect(d, 19, INK); }
                        }
                    }
                }
            } else { minigame::menu_art(fb, "Reversi", 1100); }
            return;
        }
        minigame::header(fb, f, "Reversi", &self.status());
        let (black, white) = self.board.score();
        let labels = if self.level.is_some() { ("Você", "CPU") } else { ("Pretas", "Brancas") };
        f.draw(fb, Face::Strong, 34.0, MARGIN, 310, INK, &format!("{}: {}", labels.0, black));
        let right = format!("{}: {}", labels.1, white);
        let width = f.measure(Face::Strong, 34.0, &right);
        f.draw(fb, Face::Strong, 34.0, 1000 - width, 310, INK, &right);
        ui::card(fb, Rect::new(92, 340, 888, 888), 24, 0xD8, false);
        let legal = if self.board.over() { vec![] } else { self.board.moves(self.board.turn) };
        for i in 0..64 {
            let r = Self::cell(i);
            fb.fill(Rect::new(r.x, r.y, r.w - 2, r.h - 2), 0xC6);
            let disc = Rect::new(r.x + 13, r.y + 13, 78, 78);
            match self.board.cells[i] {
                1 => fb.round_rect(disc, 39, INK),
                2 => fb.round_outline(disc, 39, 5, INK, PAPER),
                _ if legal.contains(&i) => fb.round_rect(Rect::new(r.x + 44, r.y + 44, 16, 16), 8, 0x77),
                _ => {}
            }
        }
        let hint = if let Some(p) = self.passed {
            if p == 1 { "Pretas sem jogada: a vez passou." } else { "Brancas sem jogada: a vez passou." }
        } else if self.board.over() { "Fim de jogo: vence quem tem mais peças." } else { "Toque em um ponto para jogar." };
        minigame::hint(fb, f, hint);
        minigame::pill(fb, f, Self::bottom(0), "Menu", false);
        minigame::pill(fb, f, Self::bottom(1), if self.board.over() { "Jogar de novo" } else { "Recomeçar" }, false);
    }
    fn targets(&self) -> Vec<(u32, Rect)> {
        if self.menu { return (0..OPTIONS.len()).map(|i| (900 + i as u32, minigame::menu_row(i))).collect(); }
        let mut v = vec![(801, Self::bottom(0)), (802, Self::bottom(1))];
        if !self.board.over() && !self.cpu_turn() { v.extend(self.board.moves(self.board.turn).into_iter().map(|i| (i as u32, Self::cell(i)))); }
        v
    }
    fn tap(&mut self, id: u32, _x: i32, _y: i32) -> Redraw {
        match id {
            900..=903 => { self.start(if id == 903 { None } else { Some((id - 900) as u8) }); Redraw::Full }
            801 => { self.menu = true; Redraw::Full }
            802 => { self.start(self.level); Redraw::Full }
            0..=63 if !self.board.over() && !self.cpu_turn() && !self.board.flips(id as usize, self.board.turn).is_empty() => { self.play(id as usize); Redraw::Full }
            _ => Redraw::None,
        }
    }
    fn cpu_turn(&self) -> bool { !self.menu && self.level.is_some() && self.board.turn == 2 && !self.board.over() }
    fn cpu_move(&mut self) { if self.cpu_turn() { let m = choose(&self.board, self.level.unwrap(), &mut self.rng); self.play(m); } }
    fn risky(&self, id: u32) -> bool { id == 802 && !self.board.over() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inicio_e_captura_em_varias_direcoes() {
        let mut b = Board::new();
        assert_eq!(b.moves(1), vec![19, 26, 37, 44]);
        assert!(!b.play(0));
        assert!(b.play(19));
        assert_eq!(b.score(), (4, 1));
        let mut b = Board { cells: [0; 64], turn: 1 };
        for i in [19, 26, 28, 35] { b.cells[i] = 2; }
        for i in [11, 25, 29, 43] { b.cells[i] = 1; }
        assert_eq!(b.flips(27, 1).len(), 4);
        assert!(b.play(27));
        assert_eq!(b.score(), (9, 0));
    }
    #[test]
    fn passagem_e_fim() {
        let mut g = Reversi::new();
        g.start(None);
        g.board.cells = [1; 64];
        g.board.cells[0] = 0; g.board.cells[1] = 2; g.board.cells[63] = 0; g.board.cells[62] = 2;
        g.play(0);
        assert_eq!(g.passed, Some(2));
        assert_eq!(g.board.turn, 1);
        assert!(!g.board.over());
        g.play(63);
        assert!(g.board.over());
        assert_eq!(g.board.score(), (64, 0));
    }
    #[test]
    fn cpu_escolhe_jogada_legal_e_valoriza_canto() {
        let mut rng = Rng::new();
        let b = Board::new();
        for level in 0..3 { assert!(b.moves(1).contains(&choose(&b, level, &mut rng))); }
        let mut b = Board { cells: [0; 64], turn: 2 };
        b.cells[1] = 1; b.cells[2] = 2; b.cells[8] = 1; b.cells[16] = 2;
        assert_eq!(choose(&b, 2, &mut rng), 0);
    }
}
