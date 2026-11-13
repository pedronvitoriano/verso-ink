//! Go 9×9: capturas, ko simples e contagem por área com komi de 6,5 pontos.

use crate::fb::{Framebuffer, Rect};
use crate::minigame::{self, MiniGame, Redraw, Rng, MARGIN, W};
use crate::text::{Face, Fonts};
use crate::ui::{self, INK, MUTED, PAPER};

const SIDE: usize = 9;
const N: usize = SIDE * SIDE;
type Board = [u8; N];

fn neighbors(i: usize) -> impl Iterator<Item = usize> {
    let (r, c) = (i / SIDE, i % SIDE);
    [if r > 0 { Some(i - SIDE) } else { None }, if r + 1 < SIDE { Some(i + SIDE) } else { None },
     if c > 0 { Some(i - 1) } else { None }, if c + 1 < SIDE { Some(i + 1) } else { None }].into_iter().flatten()
}

/// Encontra um grupo e suas liberdades distintas.
fn group(b: &Board, start: usize) -> (Vec<usize>, usize) {
    let mut seen = [false; N];
    let mut liberties = [false; N];
    let mut stack = vec![start];
    let mut stones = Vec::new();
    seen[start] = true;
    while let Some(i) = stack.pop() {
        stones.push(i);
        for n in neighbors(i) {
            if b[n] == 0 { liberties[n] = true; }
            else if b[n] == b[start] && !seen[n] { seen[n] = true; stack.push(n); }
        }
    }
    (stones, liberties.iter().filter(|&&v| v).count())
}

#[derive(Clone)]
struct Position {
    board: Board,
    turn: u8,
    /// Tabuleiro anterior ao último lance; uma passagem libera o ko.
    previous: Option<Board>,
    passes: u8,
    last: Option<usize>,
}

impl Position {
    fn new() -> Self { Self { board: [0; N], turn: 1, previous: None, passes: 0, last: None } }

    fn placed(&self, i: usize) -> Option<Board> {
        if i >= N || self.board[i] != 0 || self.passes >= 2 { return None; }
        let mut b = self.board;
        b[i] = self.turn;
        for n in neighbors(i) {
            if b[n] == 3 - self.turn {
                let (stones, libs) = group(&b, n);
                if libs == 0 { for s in stones { b[s] = 0; } }
            }
        }
        if group(&b, i).1 == 0 || self.previous.as_ref() == Some(&b) { return None; }
        Some(b)
    }

    fn play(&mut self, i: usize) -> bool {
        let Some(next) = self.placed(i) else { return false };
        self.previous = Some(self.board);
        self.board = next;
        self.turn = 3 - self.turn;
        self.passes = 0;
        self.last = Some(i);
        true
    }

    fn pass(&mut self) {
        if self.passes < 2 {
            self.previous = None;
            self.turn = 3 - self.turn;
            self.passes += 1;
            self.last = None;
        }
    }

    fn moves(&self) -> Vec<usize> { (0..N).filter(|&i| self.placed(i).is_some()).collect() }

    /// Pontos em meios pontos: pedras mais regiões vazias cercadas só por uma cor.
    fn score(&self) -> (i32, i32) {
        let mut points = [0i32; 3];
        let mut seen = [false; N];
        for i in 0..N {
            if self.board[i] != 0 { points[self.board[i] as usize] += 2; continue; }
            if seen[i] { continue; }
            let mut stack = vec![i];
            let mut size = 0;
            let mut borders = 0u8;
            seen[i] = true;
            while let Some(p) = stack.pop() {
                size += 1;
                for n in neighbors(p) {
                    if self.board[n] == 0 && !seen[n] { seen[n] = true; stack.push(n); }
                    else if self.board[n] != 0 { borders |= 1 << self.board[n]; }
                }
            }
            if borders == 2 { points[1] += size * 2; }
            if borders == 4 { points[2] += size * 2; }
        }
        (points[1], points[2] + 13)
    }
}

/// Simulações curtas: sorteia jogadas legais e compara a área ao fim de cada amostra.
fn cpu_choice(pos: &Position, rng: &mut Rng) -> Option<usize> {
    let mut moves = pos.moves();
    if moves.is_empty() { return None; }
    // Não insistir em preencher o próprio território quando a outra pessoa já passou.
    if pos.passes == 1 {
        let (black, white) = pos.score();
        if white > black { return None; }
    }
    rng.shuffle(&mut moves);
    moves.truncate(16);
    let mut best = (f32::NEG_INFINITY, moves[0]);
    let start = std::time::Instant::now();
    for &m in &moves {
        let mut total = 0f32;
        let mut count = 0;
        for _ in 0..3 {
            let mut s = pos.clone();
            s.play(m);
            for _ in 0..32 {
                // Sortear até achar uma jogada legal evita examinar todas as 81 casas em cada passo.
                let offset = (rng.next() % N as u64) as usize;
                let next = (0..N).map(|k| (offset + k) % N).find(|&i| s.placed(i).is_some());
                match next { Some(i) => { s.play(i); }, None => break }
            }
            let (a, b) = s.score();
            total += (b - a) as f32;
            count += 1;
            if start.elapsed() >= std::time::Duration::from_millis(650) { break; }
        }
        let value = total / count as f32;
        if value > best.0 { best = (value, m); }
        if start.elapsed() >= std::time::Duration::from_millis(650) { break; }
    }
    Some(best.1)
}

const OPTIONS: [&str; 2] = ["Contra a CPU", "Duas pessoas"];

pub struct Go {
    menu: bool,
    cpu: bool,
    pos: Position,
    rng: Rng,
}

impl Go {
    pub fn new() -> Self { Self { menu: true, cpu: true, pos: Position::new(), rng: Rng::new() } }
    fn start(&mut self, cpu: bool) { self.cpu = cpu; self.pos = Position::new(); self.menu = false; }
    fn board() -> Rect { Rect::new(MARGIN, 300, W - 2 * MARGIN, W - 2 * MARGIN) }
    fn point(i: usize) -> (i32, i32) { (136 + (i % SIDE) as i32 * 100, 364 + (i / SIDE) as i32 * 100) }
    fn target(i: usize) -> Rect { let (x, y) = Self::point(i); Rect::new(x - 49, y - 49, 98, 98) }
    fn bottom(k: usize) -> Rect { Rect::new(MARGIN + k as i32 * 472, 1288, 456, 80) }
    fn score_text(v: i32) -> String { if v % 2 == 0 { format!("{}", v / 2) } else { format!("{},5", v / 2) } }
    #[cfg(test)]
    pub fn final_para_print() -> Self {
        let mut g = Self::new();
        g.start(false);
        for (i, p) in [(10, 1), (11, 1), (19, 2), (20, 2), (40, 1), (50, 2)] { g.pos.board[i] = p; }
        g.pos.passes = 2;
        g
    }
    fn status(&self) -> String {
        if self.pos.passes >= 2 {
            let (a, b) = self.pos.score();
            let winner = if a > b { "Pretas vencem" } else { "Brancas vencem" };
            format!("{winner} · {} a {}", Self::score_text(a), Self::score_text(b))
        } else if self.cpu && self.pos.turn == 1 { "Sua vez · pretas".into() }
        else if self.cpu { "Vez da CPU · brancas".into() }
        else if self.pos.turn == 1 { "Vez das pretas".into() }
        else { "Vez das brancas".into() }
    }

    fn draw_board(&self, fb: &mut Framebuffer) {
        ui::card(fb, Self::board(), 32, PAPER, false);
        let (x, y) = Self::point(0);
        for k in 0..9 {
            fb.fill(Rect::new(x + k * 100 - 2, y, 4, 800), INK);
            fb.fill(Rect::new(x, y + k * 100 - 2, 800, 4), INK);
        }
        for &i in &[20, 24, 40, 56, 60] {
            let (x, y) = Self::point(i);
            fb.round_rect(Rect::new(x - 7, y - 7, 14, 14), 7, INK);
        }
        for i in 0..N {
            let p = self.pos.board[i];
            if p == 0 { continue; }
            let (x, y) = Self::point(i);
            let r = Rect::new(x - 41, y - 41, 82, 82);
            if p == 1 { fb.round_rect(r, 41, INK); }
            else { fb.round_outline(r, 41, 4, INK, PAPER); }
            if self.pos.last == Some(i) {
                fb.round_rect(Rect::new(x - 7, y - 7, 14, 14), 7, if p == 1 { PAPER } else { INK });
            }
        }
    }
}

impl MiniGame for Go {
    fn at_menu(&self) -> bool { self.menu }
    fn summary(&self) -> String { if self.menu { String::new() } else { self.status() } }
    fn draw(&self, fb: &mut Framebuffer, f: &Fonts) {
        fb.fill(fb.screen(), PAPER);
        ui::draw_back(fb, f, false);
        if self.menu {
            minigame::draw_menu(fb, f, "Go", "Cercar espaço, capturar pedras.", &OPTIONS);
            f.draw(fb, Face::Body, 30.0, MARGIN, 850, MUTED, "Duas passagens encerram a partida.");
            f.draw(fb, Face::Body, 30.0, MARGIN, 900, MUTED, "Brancas recebem 6,5 pontos de komi.");
            f.draw(fb, Face::Body, 30.0, MARGIN, 950, MUTED, "Capture pedras presas antes de passar.");
            minigame::menu_art(fb, "Go", 1000);
            if !crate::sprites::exists("icone-go") {
                ui::card(fb, Rect::new(MARGIN, 1000, W - 2 * MARGIN, 360), 32, ui::SURFACE, false);
                for k in 0..5 {
                    fb.fill(Rect::new(400 + k * 68, 1044, 3, 272), INK);
                    fb.fill(Rect::new(400, 1044 + k * 68, 272, 3), INK);
                }
                fb.round_rect(Rect::new(505, 1084, 56, 56), 28, INK);
                fb.round_outline(Rect::new(573, 1152, 56, 56), 28, 3, INK, PAPER);
                fb.round_rect(Rect::new(437, 1220, 56, 56), 28, INK);
            }
            return;
        }
        minigame::header(fb, f, "Go", &self.status());
        self.draw_board(fb);
        if self.pos.passes >= 2 {
            minigame::hint(fb, f, "Área cercada e pedras no tabuleiro · komi 6,5");
            minigame::pill(fb, f, Self::bottom(0), "Menu", false);
            minigame::pill(fb, f, Self::bottom(1), "Jogar de novo", true);
        } else {
            minigame::hint(fb, f, if self.pos.passes == 1 { "Capture pedras presas ou passe para encerrar." } else { "Toque numa interseção para jogar." });
            minigame::pill(fb, f, Self::bottom(0), "Passar", false);
            minigame::pill(fb, f, Self::bottom(1), "Recomeçar", false);
        }
    }
    fn targets(&self) -> Vec<(u32, Rect)> {
        if self.menu { return (0..2).map(|k| (900 + k as u32, minigame::menu_row(k))).collect(); }
        let mut v = vec![(800, Self::bottom(0)), (801, Self::bottom(1))];
        if self.pos.passes < 2 && (!self.cpu || self.pos.turn == 1) {
            v.extend(self.pos.moves().into_iter().map(|i| (i as u32, Self::target(i))));
        }
        v
    }
    fn tap(&mut self, id: u32, _x: i32, _y: i32) -> Redraw {
        match id {
            900 | 901 => { self.start(id == 900); Redraw::Full }
            800 if self.pos.passes >= 2 => { self.menu = true; Redraw::Full }
            800 if !self.cpu || self.pos.turn == 1 => { self.pos.pass(); Redraw::Full }
            801 => { self.start(self.cpu); Redraw::Full }
            i if i < N as u32 && (!self.cpu || self.pos.turn == 1) && self.pos.play(i as usize) => Redraw::Full,
            _ => Redraw::None,
        }
    }
    fn cpu_turn(&self) -> bool { !self.menu && self.cpu && self.pos.turn == 2 && self.pos.passes < 2 }
    fn cpu_move(&mut self) {
        if !self.cpu_turn() { return; }
        if let Some(i) = cpu_choice(&self.pos, &mut self.rng) { self.pos.play(i); } else { self.pos.pass(); }
    }
    fn risky(&self, id: u32) -> bool { !self.menu && self.pos.passes < 2 && id == 801 }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn p(r: usize, c: usize) -> usize { r * SIDE + c }

    #[test]
    fn captura_grupo_e_proibe_suicidio() {
        let mut s = Position::new();
        s.board[p(1, 1)] = 2;
        for i in [p(0, 1), p(1, 0), p(2, 1)] { s.board[i] = 1; }
        assert!(s.play(p(1, 2)));
        assert_eq!(s.board[p(1, 1)], 0);
        let mut s = Position::new();
        for i in [p(0, 1), p(1, 0), p(2, 1), p(1, 2)] { s.board[i] = 2; }
        assert!(!s.play(p(1, 1)));
    }

    #[test]
    fn ko_impede_repeticao_imediata_mas_passagem_libera() {
        let mut s = Position::new();
        s.board[p(4, 4)] = 2;
        for i in [p(3, 4), p(4, 3), p(5, 4)] { s.board[i] = 1; }
        for i in [p(3, 5), p(4, 6), p(5, 5)] { s.board[i] = 2; }
        assert!(s.play(p(4, 5)));
        assert!(!s.play(p(4, 4)));
        s.pass();
        assert!(s.play(p(0, 0)));
        assert!(s.play(p(4, 4)));
    }

    #[test]
    fn area_neutra_territorio_komi_e_passagens() {
        let mut s = Position::new();
        for r in 0..SIDE { s.board[p(r, 4)] = if r % 2 == 0 { 1 } else { 2 }; }
        let (a, b) = s.score();
        assert_eq!((a, b), (10, 21));
        let mut s = Position::new();
        s.board[p(0, 1)] = 1;
        s.board[p(1, 0)] = 1;
        s.board[p(8, 8)] = 2;
        assert_eq!(s.score(), (6, 15));
        s.pass(); s.pass();
        assert!(s.placed(p(0, 0)).is_none());
        assert_eq!(s.passes, 2);
    }

    #[test]
    fn cpu_joga_lance_legal_ou_passa() {
        let mut s = Position::new();
        assert!(s.play(p(4, 4)));
        let mut rng = Rng::seeded(42);
        if let Some(m) = cpu_choice(&s, &mut rng) { assert!(s.play(m)); } else { s.pass(); }
        assert_eq!(s.turn, 1);
    }
}
