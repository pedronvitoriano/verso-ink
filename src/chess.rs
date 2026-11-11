//! Xadrez para duas pessoas no mesmo Kindle: regras completas (xeque, roque,
//! en passant, promoção a dama), mate e afogamento, desfazer e jogo salvo.

use crate::fb::{Framebuffer, Rect};
use crate::text::{Face, Fonts};
use crate::ui::{self, INK, MUTED, PAPER};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Piece {
    pub kind: Kind,
    pub white: bool,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Pos {
    /// Índice = linha * 8 + coluna; linha 0 é o lado das pretas (topo da tela).
    pub board: [Option<Piece>; 64],
    pub white_to_move: bool,
    /// Roques possíveis: brancas curto, brancas longo, pretas curto, pretas longo.
    pub castle: [bool; 4],
    pub ep: Option<usize>,
    /// Meios-lances desde a última captura ou lance de peão (regra dos 50 lances).
    pub halfmove: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Move {
    pub from: usize,
    pub to: usize,
}

const BACK: [Kind; 8] = [Kind::Rook, Kind::Knight, Kind::Bishop, Kind::Queen, Kind::King, Kind::Bishop, Kind::Knight, Kind::Rook];

impl Pos {
    pub fn start() -> Self {
        let mut b = [None; 64];
        for c in 0..8 {
            b[c] = Some(Piece { kind: BACK[c], white: false });
            b[8 + c] = Some(Piece { kind: Kind::Pawn, white: false });
            b[48 + c] = Some(Piece { kind: Kind::Pawn, white: true });
            b[56 + c] = Some(Piece { kind: BACK[c], white: true });
        }
        Pos { board: b, white_to_move: true, castle: [true; 4], ep: None, halfmove: 0 }
    }

    fn king(&self, white: bool) -> Option<usize> {
        (0..64).find(|&i| self.board[i] == Some(Piece { kind: Kind::King, white }))
    }

    /// A casa `sq` é atacada por alguma peça da cor `by_white`?
    pub fn attacked(&self, sq: usize, by_white: bool) -> bool {
        let (r, c) = ((sq / 8) as i32, (sq % 8) as i32);
        let at = |r: i32, c: i32| -> Option<Piece> { ((0..8).contains(&r) && (0..8).contains(&c)).then(|| self.board[(r * 8 + c) as usize]).flatten() };
        let is = |p: Option<Piece>, k: &[Kind]| p.is_some_and(|p| p.white == by_white && k.contains(&p.kind));
        // Peão: ataca na diagonal para a frente dele.
        let pr = if by_white { r + 1 } else { r - 1 };
        if is(at(pr, c - 1), &[Kind::Pawn]) || is(at(pr, c + 1), &[Kind::Pawn]) {
            return true;
        }
        for (dr, dc) in [(1, 2), (2, 1), (-1, 2), (-2, 1), (1, -2), (2, -1), (-1, -2), (-2, -1)] {
            if is(at(r + dr, c + dc), &[Kind::Knight]) {
                return true;
            }
        }
        for dr in -1..=1 {
            for dc in -1..=1 {
                if (dr, dc) != (0, 0) && is(at(r + dr, c + dc), &[Kind::King]) {
                    return true;
                }
            }
        }
        let rays = |dirs: &[(i32, i32)], kinds: &[Kind]| {
            dirs.iter().any(|&(dr, dc)| {
                let (mut rr, mut cc) = (r + dr, c + dc);
                while (0..8).contains(&rr) && (0..8).contains(&cc) {
                    if let Some(p) = self.board[(rr * 8 + cc) as usize] {
                        return p.white == by_white && kinds.contains(&p.kind);
                    }
                    rr += dr;
                    cc += dc;
                }
                false
            })
        };
        rays(&[(1, 0), (-1, 0), (0, 1), (0, -1)], &[Kind::Rook, Kind::Queen]) || rays(&[(1, 1), (1, -1), (-1, 1), (-1, -1)], &[Kind::Bishop, Kind::Queen])
    }

    pub fn in_check(&self, white: bool) -> bool {
        self.king(white).is_some_and(|k| self.attacked(k, !white))
    }

    fn pseudo(&self, from: usize, out: &mut Vec<Move>) {
        let Some(p) = self.board[from] else { return };
        let (r, c) = ((from / 8) as i32, (from % 8) as i32);
        let free = |r: i32, c: i32| (0..8).contains(&r) && (0..8).contains(&c) && self.board[(r * 8 + c) as usize].is_none();
        let enemy = |r: i32, c: i32| (0..8).contains(&r) && (0..8).contains(&c) && self.board[(r * 8 + c) as usize].is_some_and(|q| q.white != p.white);
        let mut push = |r: i32, c: i32| out.push(Move { from, to: (r * 8 + c) as usize });
        match p.kind {
            Kind::Pawn => {
                let d = if p.white { -1 } else { 1 };
                let start = if p.white { 6 } else { 1 };
                if free(r + d, c) {
                    push(r + d, c);
                    if r == start && free(r + 2 * d, c) {
                        push(r + 2 * d, c);
                    }
                }
                for dc in [-1, 1] {
                    let to = (r + d) * 8 + c + dc;
                    if enemy(r + d, c + dc) || ((0..8).contains(&(c + dc)) && self.ep == Some(to as usize)) {
                        push(r + d, c + dc);
                    }
                }
            }
            Kind::Knight | Kind::King => {
                let steps: &[(i32, i32)] = if p.kind == Kind::Knight {
                    &[(1, 2), (2, 1), (-1, 2), (-2, 1), (1, -2), (2, -1), (-1, -2), (-2, -1)]
                } else {
                    &[(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)]
                };
                for &(dr, dc) in steps {
                    if free(r + dr, c + dc) || enemy(r + dr, c + dc) {
                        push(r + dr, c + dc);
                    }
                }
                if p.kind == Kind::King {
                    let (row, ks, qs) = if p.white { (7, 0, 1) } else { (0, 2, 3) };
                    if r == row && c == 4 && !self.attacked(from, !p.white) {
                        if self.castle[ks] && free(row, 5) && free(row, 6) && !self.attacked((row * 8 + 5) as usize, !p.white) {
                            push(row, 6);
                        }
                        if self.castle[qs] && free(row, 3) && free(row, 2) && free(row, 1) && !self.attacked((row * 8 + 3) as usize, !p.white) {
                            push(row, 2);
                        }
                    }
                }
            }
            _ => {
                let dirs: &[(i32, i32)] = match p.kind {
                    Kind::Rook => &[(1, 0), (-1, 0), (0, 1), (0, -1)],
                    Kind::Bishop => &[(1, 1), (1, -1), (-1, 1), (-1, -1)],
                    _ => &[(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)],
                };
                for &(dr, dc) in dirs {
                    let (mut rr, mut cc) = (r + dr, c + dc);
                    while free(rr, cc) {
                        push(rr, cc);
                        rr += dr;
                        cc += dc;
                    }
                    if enemy(rr, cc) {
                        push(rr, cc);
                    }
                }
            }
        }
    }

    pub fn is_promotion(&self, m: Move) -> bool {
        self.board[m.from].is_some_and(|p| p.kind == Kind::Pawn) && (m.to / 8 == 0 || m.to / 8 == 7)
    }

    pub fn apply(&self, m: Move) -> Pos {
        self.apply_promo(m, Kind::Queen)
    }

    pub fn apply_promo(&self, m: Move, promo_kind: Kind) -> Pos {
        let mut n = self.clone();
        let capture = self.board[m.to].is_some();
        let p = n.board[m.from].take().expect("casa de origem vazia");
        n.halfmove = if p.kind == Kind::Pawn || capture { 0 } else { self.halfmove + 1 };
        let (fr, fc, tr, tc) = (m.from / 8, m.from % 8, m.to / 8, m.to % 8);
        // En passant: captura o peão que passou.
        if p.kind == Kind::Pawn && Some(m.to) == self.ep && self.board[m.to].is_none() {
            n.board[fr * 8 + tc] = None;
        }
        n.ep = (p.kind == Kind::Pawn && fr.abs_diff(tr) == 2).then_some((fr + tr) / 2 * 8 + fc);
        // Roque: move a torre também.
        if p.kind == Kind::King && fc.abs_diff(tc) == 2 {
            let (rook_from, rook_to) = if tc == 6 { (fr * 8 + 7, fr * 8 + 5) } else { (fr * 8, fr * 8 + 3) };
            n.board[rook_to] = n.board[rook_from].take();
        }
        let promo = p.kind == Kind::Pawn && (tr == 0 || tr == 7);
        n.board[m.to] = Some(if promo { Piece { kind: promo_kind, white: p.white } } else { p });
        for (sq, idx) in [(60usize, [0usize, 1]), (4, [2, 3])] {
            if m.from == sq || m.to == sq {
                for i in idx {
                    n.castle[i] = false;
                }
            }
        }
        for (sq, i) in [(63usize, 0usize), (56, 1), (7, 2), (0, 3)] {
            if m.from == sq || m.to == sq {
                n.castle[i] = false;
            }
        }
        n.white_to_move = !self.white_to_move;
        n
    }

    pub fn legal_from(&self, from: usize) -> Vec<Move> {
        let mut v = Vec::new();
        if self.board[from].is_some_and(|p| p.white == self.white_to_move) {
            self.pseudo(from, &mut v);
        }
        v.retain(|&m| !self.apply(m).in_check(self.white_to_move));
        v
    }

    pub fn has_moves(&self) -> bool {
        (0..64).any(|i| !self.legal_from(i).is_empty())
    }

    /// Mesma posição para a regra da repetição (tabuleiro, vez, roques e en passant).
    fn same(&self, o: &Pos) -> bool {
        self.board == o.board && self.white_to_move == o.white_to_move && self.castle == o.castle && self.ep == o.ep
    }

    /// Só reis, ou rei com um bispo ou um cavalo: ninguém consegue dar mate.
    pub fn insufficient(&self) -> bool {
        let others: Vec<Kind> = self.board.iter().flatten().filter(|p| p.kind != Kind::King).map(|p| p.kind).collect();
        others.is_empty() || (others.len() == 1 && matches!(others[0], Kind::Bishop | Kind::Knight))
    }

    /// Vantagem de material das brancas (peão 1, cavalo e bispo 3, torre 5, dama 9).
    pub fn material(&self) -> i32 {
        self.board
            .iter()
            .flatten()
            .map(|p| {
                let v = match p.kind {
                    Kind::Pawn => 1,
                    Kind::Knight | Kind::Bishop => 3,
                    Kind::Rook => 5,
                    Kind::Queen => 9,
                    Kind::King => 0,
                };
                if p.white { v } else { -v }
            })
            .sum()
    }

    fn encode(&self) -> String {
        let mut s: String = self
            .board
            .iter()
            .map(|p| match p {
                None => '.',
                Some(p) => {
                    let ch = match p.kind {
                        Kind::Pawn => 'p',
                        Kind::Knight => 'n',
                        Kind::Bishop => 'b',
                        Kind::Rook => 'r',
                        Kind::Queen => 'q',
                        Kind::King => 'k',
                    };
                    if p.white { ch.to_ascii_uppercase() } else { ch }
                }
            })
            .collect();
        s.push(if self.white_to_move { 'w' } else { 'b' });
        s.extend(self.castle.iter().map(|&c| if c { '1' } else { '0' }));
        s.push_str(&format!(" {} {}", self.ep.map(|e| e as i32).unwrap_or(-1), self.halfmove));
        s
    }

    fn decode(t: &str) -> Option<Pos> {
        let (body, ep) = t.split_once(' ')?;
        let ch: Vec<char> = body.chars().collect();
        if ch.len() != 69 {
            return None;
        }
        let board = std::array::from_fn(|i| {
            let k = match ch[i].to_ascii_lowercase() {
                'p' => Kind::Pawn,
                'n' => Kind::Knight,
                'b' => Kind::Bishop,
                'r' => Kind::Rook,
                'q' => Kind::Queen,
                'k' => Kind::King,
                _ => return None,
            };
            Some(Piece { kind: k, white: ch[i].is_ascii_uppercase() })
        });
        let mut nums = ep.split_whitespace();
        let ep: i32 = nums.next()?.parse().ok()?;
        let halfmove: u32 = nums.next().and_then(|h| h.parse().ok()).unwrap_or(0);
        Some(Pos { board, white_to_move: ch[64] == 'w', castle: std::array::from_fn(|i| ch[65 + i] == '1'), ep: (ep >= 0).then_some(ep as usize), halfmove })
    }
}

pub struct Game {
    pub pos: Pos,
    pub history: Vec<Pos>,
    pub selected: Option<usize>,
    pub targets: Vec<usize>,
    pub last: Option<Move>,
    /// Peão chegou à última fileira: esperando a escolha da peça.
    pub pending_promo: Option<Move>,
}

pub const PROMO: [Kind; 4] = [Kind::Queen, Kind::Rook, Kind::Bishop, Kind::Knight];

impl Game {
    pub fn new() -> Self {
        Game { pos: Pos::start(), history: Vec::new(), selected: None, targets: Vec::new(), last: None, pending_promo: None }
    }

    fn path() -> std::path::PathBuf {
        crate::home_dir().join("data/xadrez.txt")
    }

    pub fn save(&self) {
        let mut s: Vec<String> = self.history.iter().map(Pos::encode).collect();
        s.push(self.pos.encode());
        let _ = std::fs::write(Self::path(), s.join("\n"));
    }

    pub fn load() -> Option<Self> {
        let s = std::fs::read_to_string(Self::path()).ok()?;
        let mut all: Vec<Pos> = s.lines().filter_map(Pos::decode).collect();
        let pos = all.pop()?;
        Some(Game { pos, history: all, selected: None, targets: Vec::new(), last: None, pending_promo: None })
    }

    /// Partida terminada? Devolve o texto do resultado.
    pub fn result(&self) -> Option<String> {
        let other = if self.pos.white_to_move { "pretas" } else { "brancas" };
        if !self.pos.has_moves() {
            return Some(if self.pos.in_check(self.pos.white_to_move) { format!("Xeque-mate. Vitória das {other}.") } else { "Empate por afogamento.".into() });
        }
        if self.pos.insufficient() {
            return Some("Empate: ninguém consegue dar mate.".into());
        }
        if self.pos.halfmove >= 100 {
            return Some("Empate pela regra dos 50 lances.".into());
        }
        let reps = self.history.iter().filter(|p| p.same(&self.pos)).count() + 1;
        if reps >= 3 {
            return Some("Empate por repetição.".into());
        }
        None
    }

    pub fn status(&self) -> String {
        if let Some(r) = self.result() {
            return r;
        }
        let side = if self.pos.white_to_move { "brancas" } else { "pretas" };
        if self.pos.in_check(self.pos.white_to_move) { format!("Xeque! Vez das {side}.") } else { format!("Vez das {side}.") }
    }

    pub fn in_progress(&self) -> bool {
        !self.history.is_empty() && self.result().is_none()
    }

    fn commit(&mut self, next: Pos, m: Move) {
        self.history.push(self.pos.clone());
        self.pos = next;
        self.last = Some(m);
        self.selected = None;
        self.targets.clear();
        self.pending_promo = None;
        self.save();
    }

    /// Escolha da peça na promoção.
    pub fn promote(&mut self, k: Kind) {
        if let Some(m) = self.pending_promo {
            let next = self.pos.apply_promo(m, k);
            self.commit(next, m);
        }
    }

    /// Toque numa casa: escolhe a peça, move ou troca de peça. Devolve true se moveu.
    pub fn tap(&mut self, sq: usize) -> bool {
        if self.result().is_some() || self.pending_promo.is_some() {
            return false;
        }
        if let Some(m) = self.selected.and_then(|from| self.targets.contains(&sq).then_some(Move { from, to: sq })) {
            if self.pos.is_promotion(m) {
                self.pending_promo = Some(m);
                return false;
            }
            let next = self.pos.apply(m);
            self.commit(next, m);
            return true;
        }
        let moves = self.pos.legal_from(sq);
        if moves.is_empty() || self.selected == Some(sq) {
            self.selected = None;
            self.targets.clear();
        } else {
            self.selected = Some(sq);
            self.targets = moves.iter().map(|m| m.to).collect();
        }
        false
    }

    pub fn undo(&mut self) {
        if self.pending_promo.take().is_some() {
            return;
        }
        if let Some(p) = self.history.pop() {
            self.pos = p;
            self.selected = None;
            self.targets.clear();
            self.last = None;
            self.save();
        }
    }
}

// ---------- Desenho ----------

pub const SQ: i32 = 112;
pub const BX: i32 = (1072 - 8 * SQ) / 2;
pub const BY: i32 = 250;

pub fn square_rect(i: usize) -> Rect {
    Rect::new(BX + (i % 8) as i32 * SQ, BY + (i / 8) as i32 * SQ, SQ, SQ)
}
pub fn board_rect() -> Rect {
    Rect::new(BX, BY, 8 * SQ, 8 * SQ)
}
pub fn undo_rect() -> Rect {
    Rect::new(72, 1300, (1072 - 144 - 24) / 2, 100)
}
pub fn new_rect() -> Rect {
    let r = undo_rect();
    Rect::new(r.x + r.w + 24, r.y, r.w, r.h)
}

/// Formas das peças em coordenadas 0..1: polígonos e círculos (cx, cy, r).
fn shape(k: Kind) -> (Vec<Vec<(f32, f32)>>, Vec<(f32, f32, f32)>) {
    let base = vec![(0.22, 0.80), (0.78, 0.80), (0.80, 0.90), (0.20, 0.90)];
    match k {
        Kind::Pawn => (vec![vec![(0.38, 0.46), (0.62, 0.46), (0.70, 0.80), (0.30, 0.80)], base], vec![(0.5, 0.33, 0.14)]),
        Kind::Rook => (
            vec![
                vec![(0.32, 0.36), (0.68, 0.36), (0.70, 0.80), (0.30, 0.80)],
                vec![(0.25, 0.16), (0.36, 0.16), (0.36, 0.25), (0.45, 0.25), (0.45, 0.16), (0.55, 0.16), (0.55, 0.25), (0.64, 0.25), (0.64, 0.16), (0.75, 0.16), (0.75, 0.38), (0.25, 0.38)],
                base,
            ],
            vec![],
        ),
        Kind::Bishop => (
            vec![vec![(0.5, 0.14), (0.66, 0.32), (0.62, 0.54), (0.38, 0.54), (0.34, 0.32)], vec![(0.40, 0.54), (0.60, 0.54), (0.68, 0.80), (0.32, 0.80)], base],
            vec![(0.5, 0.11, 0.06)],
        ),
        Kind::Knight => (
            // Cabeça de cavalo de perfil, virada para a esquerda: focinho, orelhas e crina.
            vec![vec![(0.30, 0.80), (0.34, 0.63), (0.45, 0.50), (0.36, 0.49), (0.27, 0.55), (0.19, 0.50), (0.21, 0.40), (0.37, 0.27), (0.45, 0.14), (0.50, 0.21), (0.56, 0.11), (0.61, 0.21), (0.71, 0.31), (0.77, 0.50), (0.75, 0.80)], base],
            vec![],
        ),
        Kind::Queen => (
            vec![vec![(0.22, 0.22), (0.36, 0.50), (0.41, 0.16), (0.50, 0.48), (0.59, 0.16), (0.64, 0.50), (0.78, 0.22), (0.70, 0.72), (0.30, 0.72)], vec![(0.27, 0.72), (0.73, 0.72), (0.76, 0.80), (0.24, 0.80)], base],
            vec![(0.22, 0.20, 0.05), (0.41, 0.14, 0.05), (0.59, 0.14, 0.05), (0.78, 0.20, 0.05)],
        ),
        Kind::King => (
            vec![
                vec![(0.30, 0.42), (0.70, 0.42), (0.67, 0.80), (0.33, 0.80)],
                vec![(0.46, 0.05), (0.54, 0.05), (0.54, 0.32), (0.46, 0.32)],
                vec![(0.38, 0.12), (0.62, 0.12), (0.62, 0.20), (0.38, 0.20)],
                base,
            ],
            vec![(0.5, 0.44, 0.19)],
        ),
    }
}

fn inside_poly(px: f32, py: f32, poly: &[(f32, f32)]) -> bool {
    let mut inside = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let ((xi, yi), (xj, yj)) = (poly[i], poly[j]);
        if (yi > py) != (yj > py) && px < (xj - xi) * (py - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Máscara de cobertura (0..255) da peça, com 4x4 amostras por pixel. `shrink` encolhe a forma.
fn mask(k: Kind, size: i32, shrink: f32) -> Vec<u8> {
    let (polys, circles) = shape(k);
    let center = (0.5, 0.55);
    let scale = |(x, y): (f32, f32)| (center.0 + (x - center.0) * (1.0 - shrink), center.1 + (y - center.1) * (1.0 - shrink));
    let polys: Vec<Vec<(f32, f32)>> = polys.iter().map(|p| p.iter().map(|&q| scale(q)).collect()).collect();
    let circles: Vec<(f32, f32, f32)> = circles.iter().map(|&(x, y, r)| {
        let (x, y) = scale((x, y));
        (x, y, r * (1.0 - shrink))
    }).collect();
    let mut out = vec![0u8; (size * size) as usize];
    let n = 4;
    for py in 0..size {
        for px in 0..size {
            let mut hits = 0;
            for sy in 0..n {
                for sx in 0..n {
                    let x = (px as f32 + (sx as f32 + 0.5) / n as f32) / size as f32;
                    let y = (py as f32 + (sy as f32 + 0.5) / n as f32) / size as f32;
                    if polys.iter().any(|p| inside_poly(x, y, p)) || circles.iter().any(|&(cx, cy, r)| (x - cx).powi(2) + (y - cy).powi(2) <= r * r) {
                        hits += 1;
                    }
                }
            }
            out[(py * size + px) as usize] = (hits * 255 / (n * n)) as u8;
        }
    }
    out
}

/// Peças desenhadas uma vez e guardadas: (contorno, miolo) por tipo.
pub struct Sprites {
    full: Vec<Vec<u8>>,
    inner: Vec<Vec<u8>>,
}

const KINDS: [Kind; 6] = [Kind::Pawn, Kind::Knight, Kind::Bishop, Kind::Rook, Kind::Queen, Kind::King];

impl Sprites {
    /// Desenha as peças (alguns segundos no Kindle) ou lê do cache em disco.
    pub fn new() -> Self {
        let size = SQ - 8;
        let n = (size * size) as usize;
        let path = crate::home_dir().join("cache/pecas-v2.bin");
        if let Ok(data) = std::fs::read(&path) {
            if data.len() == n * 12 {
                let chunks: Vec<Vec<u8>> = data.chunks(n).map(<[u8]>::to_vec).collect();
                return Sprites { full: chunks[..6].to_vec(), inner: chunks[6..].to_vec() };
            }
        }
        let sp = Sprites { full: KINDS.iter().map(|&k| mask(k, size, 0.0)).collect(), inner: KINDS.iter().map(|&k| mask(k, size, 0.13)).collect() };
        let _ = std::fs::create_dir_all(crate::home_dir().join("cache"));
        let _ = std::fs::write(path, [sp.full.concat(), sp.inner.concat()].concat());
        sp
    }
    pub fn draw(&self, fb: &mut Framebuffer, p: Piece, x: i32, y: i32) {
        let k = KINDS.iter().position(|&q| q == p.kind).unwrap_or(0);
        let size = SQ - 8;
        for (layer, v) in [(&self.full[k], INK), (&self.inner[k], if p.white { PAPER } else { 0x55 })] {
            for py in 0..size {
                for px in 0..size {
                    let a = layer[(py * size + px) as usize];
                    if a > 0 {
                        fb.blend(x + px, y + py, v, a);
                    }
                }
            }
        }
    }
}

pub fn draw_square(fb: &mut Framebuffer, g: &Game, sp: &Sprites, i: usize) {
    let r = square_rect(i);
    let dark = (i / 8 + i % 8) % 2 == 1;
    let last = g.last.is_some_and(|m| m.from == i || m.to == i);
    fb.fill(r, if dark { 0xB0 } else { 0xF2 });
    if last {
        fb.fill(Rect::new(r.x + 4, r.y + 4, r.w - 8, r.h - 8), if dark { 0x90 } else { 0xD8 });
    }
    if g.selected == Some(i) {
        fb.round_outline(Rect::new(r.x + 3, r.y + 3, r.w - 6, r.h - 6), 10, 7, INK, if dark { 0xB0 } else { 0xF2 });
    }
    if let Some(p) = g.pos.board[i] {
        sp.draw(fb, p, r.x + 4, r.y + 4);
    }
    if g.targets.contains(&i) {
        let (cx, cy) = (r.x + r.w / 2, r.y + r.h / 2);
        if g.pos.board[i].is_some() {
            fb.round_outline(Rect::new(r.x + 6, r.y + 6, r.w - 12, r.h - 12), 50, 6, INK, PAPER);
            if let Some(p) = g.pos.board[i] {
                sp.draw(fb, p, r.x + 4, r.y + 4);
            }
        } else {
            fb.round_rect(Rect::new(cx - 16, cy - 16, 32, 32), 16, INK);
        }
    }
}

pub fn draw(fb: &mut Framebuffer, f: &Fonts, g: &Game, sp: &Sprites) {
    fb.fill(fb.screen(), PAPER);
    ui::draw_back(fb, f, false);
    let tw = f.measure(Face::Strong, 34.0, "Xadrez");
    f.draw(fb, Face::Strong, 34.0, 1072 - 72 - tw, 128, INK, "Xadrez");
    draw_status(fb, f, g, sp);
    let b = board_rect();
    fb.round_outline(Rect::new(b.x - 6, b.y - 6, b.w + 12, b.h + 12), 8, 4, INK, PAPER);
    for i in 0..64 {
        draw_square(fb, g, sp, i);
    }
    for (r, label) in [(undo_rect(), "Desfazer"), (new_rect(), "Novo jogo")] {
        fb.round_outline(r, r.h / 2, 4, INK, PAPER);
        let w = f.measure(Face::Strong, 38.0, label);
        f.draw(fb, Face::Strong, 38.0, r.x + (r.w - w) / 2, r.y + 63, INK, label);
    }
}

pub fn status_rect() -> Rect {
    Rect::new(0, 1180, 1072, 90)
}

pub fn promo_rect(k: usize) -> Rect {
    let w = (1072 - 144 - 3 * 16) / 4;
    Rect::new(72 + k as i32 * (w + 16), 1176, w, 96)
}

pub fn draw_status(fb: &mut Framebuffer, f: &Fonts, g: &Game, sp: &Sprites) {
    let r = status_rect();
    fb.fill(r, PAPER);
    if g.pending_promo.is_some() {
        // Escolha da peça: quatro botões com a peça desenhada.
        let white = g.pos.white_to_move;
        for (k, kind) in PROMO.iter().enumerate() {
            let b = promo_rect(k);
            fb.round_outline(b, 20, 4, INK, PAPER);
            sp.draw(fb, Piece { kind: *kind, white }, b.x + (b.w - (SQ - 8)) / 2, b.y - 4);
        }
        return;
    }
    let s = g.status();
    let s = crate::apps::fit(f, Face::Display, 46.0, &s, 1072 - 144 - 200);
    f.draw(fb, Face::Display, 46.0, 72, r.y + 62, INK, &s);
    let n = g.history.len();
    let mat = g.pos.material();
    let side = match mat {
        0 => String::new(),
        m if m > 0 => format!("brancas +{m} · "),
        m => format!("pretas +{} · ", -m),
    };
    let moves = if n == 0 { "início".to_string() } else { format!("{side}{n} {}", if n == 1 { "lance" } else { "lances" }) };
    let w = f.measure(Face::Body, 28.0, &moves);
    f.draw(fb, Face::Body, 28.0, 1072 - 72 - w, r.y + 60, MUTED, &moves);
}

pub fn draw_resume(fb: &mut Framebuffer, f: &Fonts, g: &Game) {
    fb.fill(fb.screen(), PAPER);
    ui::draw_back(fb, f, false);
    f.draw(fb, Face::Display, 150.0, 66, 330, INK, "Xadrez");
    f.draw(fb, Face::Strong, 34.0, 72, 420, MUTED, "Você tem uma partida pela metade");
    let r = resume_continue_rect();
    fb.round_rect(r, 50, INK);
    f.draw(fb, Face::Display, 56.0, r.x + 44, r.y + 70, PAPER, "Continuar");
    let side = if g.pos.white_to_move { "vez das brancas" } else { "vez das pretas" };
    let n = g.history.len();
    f.draw(fb, Face::Body, 30.0, r.x + 46, r.y + 116, 0xCC, &format!("{n} {} · {side}", if n == 1 { "lance" } else { "lances" }));
    let n = resume_new_rect();
    fb.round_outline(n, n.h / 2, 4, INK, PAPER);
    let w = f.measure(Face::Strong, 40.0, "Partida nova");
    f.draw(fb, Face::Strong, 40.0, n.x + (n.w - w) / 2, n.y + 72, INK, "Partida nova");
}
pub fn resume_continue_rect() -> Rect {
    Rect::new(72, 520, 1072 - 144, 140)
}
pub fn resume_new_rect() -> Rect {
    Rect::new(72, 700, 1072 - 144, 116)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sq(s: &str) -> usize {
        let b = s.as_bytes();
        (7 - (b[1] - b'1') as usize) * 8 + (b[0] - b'a') as usize
    }
    fn play(g: &mut Game, moves: &[&str]) {
        for m in moves {
            let (a, b) = m.split_at(2);
            g.tap(sq(a));
            assert!(g.tap(sq(b)), "lance ilegal: {m}");
        }
    }
    #[test]
    fn abertura_tem_20_lances() {
        let p = Pos::start();
        assert_eq!((0..64).map(|i| p.legal_from(i).len()).sum::<usize>(), 20);
    }
    #[test]
    fn mate_do_pastor() {
        let mut g = Game::new();
        play(&mut g, &["e2e4", "e7e5", "f1c4", "b8c6", "d1h5", "g8f6", "h5f7"]);
        assert!(g.status().starts_with("Xeque-mate"));
    }
    #[test]
    fn roque_en_passant_e_promocao() {
        let mut g = Game::new();
        play(&mut g, &["e2e4", "a7a6", "e4e5", "d7d5", "e5d6"]); // en passant
        assert!(g.pos.board[sq("d5")].is_none());
        play(&mut g, &["a6a5", "g1f3", "a5a4", "f1e2", "a4a3", "e1g1"]); // roque curto
        assert_eq!(g.pos.board[sq("f1")].map(|p| p.kind), Some(Kind::Rook));
        play(&mut g, &["a3b2", "d6c7"]);
        g.tap(sq("b2"));
        assert!(!g.tap(sq("a1")) && g.pending_promo.is_some()); // promoção espera a escolha
        g.promote(Kind::Knight);
        assert_eq!(g.pos.board[sq("a1")].map(|p| p.kind), Some(Kind::Knight));
        g.undo();
        assert_eq!(g.pos.board[sq("b2")].map(|p| p.kind), Some(Kind::Pawn));
    }
    #[test]
    fn empate_por_repeticao() {
        let mut g = Game::new();
        play(&mut g, &["g1f3", "g8f6", "f3g1", "f6g8", "g1f3", "g8f6", "f3g1", "f6g8"]);
        assert_eq!(g.result().as_deref(), Some("Empate por repetição."));
    }
}
