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

    /// Posição em FEN (para o Claude e para o contexto).
    pub fn fen(&self, fullmove: usize) -> String {
        let mut s = String::new();
        for r in 0..8 {
            let mut empty = 0;
            for c in 0..8 {
                match self.board[r * 8 + c] {
                    None => empty += 1,
                    Some(p) => {
                        if empty > 0 {
                            s.push_str(&empty.to_string());
                            empty = 0;
                        }
                        let ch = match p.kind {
                            Kind::Pawn => 'p',
                            Kind::Knight => 'n',
                            Kind::Bishop => 'b',
                            Kind::Rook => 'r',
                            Kind::Queen => 'q',
                            Kind::King => 'k',
                        };
                        s.push(if p.white { ch.to_ascii_uppercase() } else { ch });
                    }
                }
            }
            if empty > 0 {
                s.push_str(&empty.to_string());
            }
            if r < 7 {
                s.push('/');
            }
        }
        let castle: String = ['K', 'Q', 'k', 'q'].iter().zip(self.castle).filter(|(_, ok)| *ok).map(|(c, _)| *c).collect();
        let ep = self.ep.map(|i| format!("{}{}", (b'a' + (i % 8) as u8) as char, 8 - i / 8)).unwrap_or_else(|| "-".into());
        format!("{s} {} {} {ep} {} {fullmove}", if self.white_to_move { 'w' } else { 'b' }, if castle.is_empty() { "-".into() } else { castle }, self.halfmove)
    }

    /// Todos os lances legais em UCI (promoções com as quatro peças).
    pub fn legal_uci(&self) -> Vec<String> {
        let mut v = Vec::new();
        for from in 0..64 {
            for m in self.legal_from(from) {
                let pawn = self.board[from].is_some_and(|p| p.kind == Kind::Pawn);
                if pawn && (m.to / 8 == 0 || m.to / 8 == 7) {
                    for k in [Kind::Queen, Kind::Rook, Kind::Bishop, Kind::Knight] {
                        v.push(uci(m, Some(k)));
                    }
                } else {
                    v.push(uci(m, None));
                }
            }
        }
        v
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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// Duas pessoas no mesmo Kindle.
    Local,
    /// Contra um treinador da escada (índice), com a cor de quem joga.
    Coach { level: usize, user_white: bool },
}

pub struct Game {
    pub mode: Mode,
    /// Lances no formato do Stockfish (e2e4, e7e8q), para o motor e a revisão.
    pub moves: Vec<String>,
    pub pos: Pos,
    pub history: Vec<Pos>,
    pub selected: Option<usize>,
    pub targets: Vec<usize>,
    pub last: Option<Move>,
    /// Peão chegou à última fileira: esperando a escolha da peça.
    pub pending_promo: Option<Move>,
    /// Relógio da partida (None = sem tempo).
    pub clock: Option<Clock>,
}

// ---------- Relógio ----------

/// Ritmos (minutos + segundos de acréscimo por lance, o incremento de Fischer), na mesma
/// divisão do Lichess/FIDE: bala, relâmpago, rápida e clássica.
pub const CONTROLS: [(u32, u32); 11] = [(60, 0), (120, 1), (180, 0), (180, 2), (300, 0), (300, 3), (600, 0), (600, 5), (900, 10), (1800, 0), (1800, 20)];

pub fn control_label(k: usize) -> String {
    let (b, i) = CONTROLS[k];
    format!("{}+{}", b / 60, i)
}

/// Categoria pelo tempo estimado de partida (base + 40 × acréscimo), como no Lichess.
pub fn control_category(k: usize) -> &'static str {
    let (b, i) = CONTROLS[k];
    match b + 40 * i {
        0..=179 => "Bala",
        180..=479 => "Relâmpago",
        480..=1499 => "Rápida",
        _ => "Clássica",
    }
}

fn control_path() -> std::path::PathBuf {
    crate::home_dir().join("data/xadrez_ritmo.txt")
}

/// Ritmo escolhido para as próximas partidas (None = sem tempo).
pub fn load_control() -> Option<usize> {
    std::fs::read_to_string(control_path()).ok()?.trim().parse().ok().filter(|&k: &usize| k < CONTROLS.len())
}

pub fn save_control(k: Option<usize>) {
    let _ = std::fs::write(control_path(), k.map(|k| k.to_string()).unwrap_or_default());
}

pub fn control_name(k: Option<usize>) -> String {
    match k {
        None => "Sem tempo".into(),
        Some(k) => format!("{} · {}", control_label(k), control_category(k).to_lowercase()),
    }
}

#[derive(Clone, Debug)]
pub struct Clock {
    pub ctrl: usize,
    /// Milissegundos restantes: [brancas, pretas].
    pub left: [i64; 2],
    /// Desde quando o relógio de quem joga está correndo.
    pub since: Option<std::time::Instant>,
    /// Caiu a bandeira de quem (true = brancas).
    pub flag: Option<bool>,
}

impl Clock {
    pub fn new(ctrl: usize) -> Clock {
        let ms = CONTROLS[ctrl].0 as i64 * 1000;
        Clock { ctrl, left: [ms, ms], since: None, flag: None }
    }
}

fn side(white: bool) -> usize {
    if white { 0 } else { 1 }
}

/// "4:59", "0:07"; abaixo de 10 s mostra décimos ("9.4").
pub fn clock_text(ms: i64) -> String {
    let ms = ms.max(0);
    if ms < 10_000 {
        return format!("{}.{}", ms / 1000, ms % 1000 / 100);
    }
    let s = (ms + 999) / 1000;
    if s >= 3600 { format!("{}:{:02}:{:02}", s / 3600, s % 3600 / 60, s % 60) } else { format!("{}:{:02}", s / 60, s % 60) }
}

pub const PROMO: [Kind; 4] = [Kind::Queen, Kind::Rook, Kind::Bishop, Kind::Knight];

impl Game {
    pub fn new() -> Self {
        Game { mode: Mode::Local, moves: Vec::new(), pos: Pos::start(), history: Vec::new(), selected: None, targets: Vec::new(), last: None, pending_promo: None, clock: load_control().map(Clock::new) }
    }

    fn path() -> std::path::PathBuf {
        crate::home_dir().join("data/xadrez.txt")
    }

    pub fn new_coach(level: usize, user_white: bool) -> Self {
        Game { mode: Mode::Coach { level, user_white }, ..Game::new() }
    }

    /// O tabuleiro aparece com as pretas embaixo quando você joga de pretas. No jogo local com
    /// "Girar", na vez das pretas a tela vira 180° e o tabuleiro também se inverte: assim quem está
    /// do outro lado da mesa vê as próprias peças (pretas) perto de si e de pé.
    pub fn flipped(&self) -> bool {
        match self.mode {
            Mode::Coach { user_white, .. } => !user_white,
            Mode::Local => rotate_enabled() && !self.pos.white_to_move,
        }
    }

    /// Apaga a partida salva (fim de jogo local por desistência ou empate).
    pub fn clear_saved() {
        let _ = std::fs::remove_file(Self::path());
    }

    /// É a vez do computador?
    pub fn engine_turn(&self) -> bool {
        matches!(self.mode, Mode::Coach { user_white, .. } if user_white != self.pos.white_to_move) && self.result().is_none()
    }

    pub fn save(&self) {
        let mode = match self.mode {
            Mode::Local => "M local".to_string(),
            Mode::Coach { level, user_white } => format!("M coach {level} {}", if user_white { "w" } else { "b" }),
        };
        let mut s: Vec<String> = vec![mode, format!("U {}", self.moves.join(" "))];
        if let Some(c) = &self.clock {
            let flag = match c.flag {
                Some(true) => "w",
                Some(false) => "b",
                None => "-",
            };
            s.push(format!("C {} {} {} {flag}", c.ctrl, self.clock_left(true).unwrap_or(0), self.clock_left(false).unwrap_or(0)));
        }
        s.extend(self.history.iter().map(Pos::encode));
        s.push(self.pos.encode());
        let _ = std::fs::write(Self::path(), s.join("\n"));
    }

    pub fn load() -> Option<Self> {
        let s = std::fs::read_to_string(Self::path()).ok()?;
        let mut mode = Mode::Local;
        let mut moves = Vec::new();
        let mut clock = None;
        for l in s.lines() {
            if let Some(c) = l.strip_prefix("C ") {
                let w: Vec<&str> = c.split_whitespace().collect();
                if let (Some(k), Some(a), Some(b)) = (w.first().and_then(|x| x.parse::<usize>().ok()), w.get(1).and_then(|x| x.parse().ok()), w.get(2).and_then(|x| x.parse().ok())) {
                    if k < CONTROLS.len() {
                        let flag = match w.get(3) {
                            Some(&"w") => Some(true),
                            Some(&"b") => Some(false),
                            _ => None,
                        };
                        clock = Some(Clock { ctrl: k, left: [a, b], since: None, flag });
                    }
                }
                continue;
            }
            if let Some(m) = l.strip_prefix("M ") {
                let w: Vec<&str> = m.split_whitespace().collect();
                if w.first() == Some(&"coach") {
                    mode = Mode::Coach { level: w.get(1).and_then(|x| x.parse().ok()).unwrap_or(0), user_white: w.get(2) != Some(&"b") };
                }
            } else if let Some(u) = l.strip_prefix("U") {
                moves = u.split_whitespace().map(str::to_string).collect();
            }
        }
        let mut all: Vec<Pos> = s.lines().filter_map(Pos::decode).collect();
        let pos = all.pop()?;
        Some(Game { mode, moves, pos, history: all, selected: None, targets: Vec::new(), last: None, pending_promo: None, clock })
    }

    /// Aplica um lance que veio do Stockfish (e2e4 / e7e8q). Devolve o lance em notação.
    pub fn play_uci(&mut self, uci: &str) -> Option<String> {
        let (m, promo) = parse_uci(uci)?;
        if !self.pos.legal_from(m.from).iter().any(|x| *x == m) {
            return None;
        }
        let san = san(&self.pos, m, promo);
        let next = self.pos.apply_promo(m, promo.unwrap_or(Kind::Queen));
        self.commit(next, m, promo);
        Some(san)
    }

    /// Partida terminada? Devolve o texto do resultado.
    pub fn result(&self) -> Option<String> {
        if let Some(loser) = self.clock.as_ref().and_then(|c| c.flag) {
            // Sem material para dar mate, quem venceria só empata.
            let winner_has = self.pos.board.iter().flatten().any(|p| p.white != loser && p.kind != Kind::King);
            let (who, other) = if loser { ("brancas", "pretas") } else { ("pretas", "brancas") };
            return Some(if winner_has { format!("Tempo das {who} acabou. Vitória das {other}.") } else { format!("Tempo das {who} acabou, mas as {other} não têm como dar mate: empate.") });
        }
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

    /// Tempo restante de um lado (None = partida sem relógio).
    pub fn clock_left(&self, white: bool) -> Option<i64> {
        let c = self.clock.as_ref()?;
        let mut ms = c.left[side(white)];
        if let Some(t) = c.since.filter(|_| self.pos.white_to_move == white) {
            ms -= t.elapsed().as_millis() as i64;
        }
        Some(ms.max(0))
    }

    pub fn clock_running(&self) -> bool {
        self.clock.as_ref().is_some_and(|c| c.since.is_some())
    }

    /// Confere a bandeira; devolve true se acabou o tempo agora.
    pub fn clock_tick(&mut self) -> bool {
        let white = self.pos.white_to_move;
        if self.clock_running() && self.clock_left(white) == Some(0) {
            if let Some(c) = self.clock.as_mut() {
                c.left[side(white)] = 0;
                c.since = None;
                c.flag = Some(white);
            }
            self.save();
            return true;
        }
        false
    }

    /// Para o relógio (saiu da partida); o tempo gasto fica guardado.
    pub fn clock_pause(&mut self) {
        let white = self.pos.white_to_move;
        let left = self.clock_left(white);
        if let (Some(c), Some(ms)) = (self.clock.as_mut(), left) {
            c.left[side(white)] = ms;
            c.since = None;
        }
    }

    /// Volta a correr (entrou na partida de novo).
    pub fn clock_resume(&mut self) {
        let go = self.moves.len() >= 2 && self.result().is_none() && self.pending_promo.is_none();
        if let Some(c) = self.clock.as_mut() {
            if go && c.since.is_none() {
                c.since = Some(std::time::Instant::now());
            }
        }
    }

    pub fn in_progress(&self) -> bool {
        !self.history.is_empty() && self.result().is_none()
    }

    fn commit(&mut self, next: Pos, m: Move, promo: Option<Kind>) {
        let mover = self.pos.white_to_move;
        let n = self.moves.len() + 1;
        if let Some(c) = self.clock.as_mut() {
            if let Some(t) = c.since.take() {
                c.left[side(mover)] -= t.elapsed().as_millis() as i64;
            }
            c.left[side(mover)] += CONTROLS[c.ctrl].1 as i64 * 1000;
            // Como no Lichess: o relógio só corre depois do primeiro lance de cada lado.
            if n >= 2 {
                c.since = Some(std::time::Instant::now());
            }
        }
        self.moves.push(uci(m, promo));
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
            self.commit(next, m, Some(k));
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
            self.commit(next, m, None);
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

    #[allow(dead_code)] // usado nos testes; na partida não há desfazer
    pub fn undo(&mut self) {
        if self.pending_promo.take().is_some() {
            return;
        }
        if let Some(p) = self.history.pop() {
            self.moves.pop();
            self.pos = p;
            self.selected = None;
            self.targets.clear();
            self.last = None;
            self.save();
        }
    }
}

// ---------- Notação ----------

pub fn square_name(i: usize) -> String {
    format!("{}{}", (b'a' + (i % 8) as u8) as char, 8 - i / 8)
}

fn square_from(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    let (f, r) = (b.first()?.checked_sub(b'a')? as usize, b.get(1)?.checked_sub(b'1')? as usize);
    (f < 8 && r < 8).then_some((7 - r) * 8 + f)
}

pub fn uci(m: Move, promo: Option<Kind>) -> String {
    let p = match promo {
        Some(Kind::Queen) => "q",
        Some(Kind::Rook) => "r",
        Some(Kind::Bishop) => "b",
        Some(Kind::Knight) => "n",
        _ => "",
    };
    format!("{}{}{p}", square_name(m.from), square_name(m.to))
}

pub fn parse_uci(s: &str) -> Option<(Move, Option<Kind>)> {
    let from = square_from(s.get(0..2)?)?;
    let to = square_from(s.get(2..4)?)?;
    let promo = match s.get(4..5) {
        Some("q") => Some(Kind::Queen),
        Some("r") => Some(Kind::Rook),
        Some("b") => Some(Kind::Bishop),
        Some("n") => Some(Kind::Knight),
        _ => None,
    };
    Some((Move { from, to }, promo))
}

pub fn letter(k: Kind) -> &'static str {
    match k {
        Kind::King => "R",
        Kind::Queen => "D",
        Kind::Rook => "T",
        Kind::Bishop => "B",
        Kind::Knight => "C",
        Kind::Pawn => "",
    }
}

pub fn piece_name(k: Kind) -> &'static str {
    match k {
        Kind::King => "rei",
        Kind::Queen => "dama",
        Kind::Rook => "torre",
        Kind::Bishop => "bispo",
        Kind::Knight => "cavalo",
        Kind::Pawn => "peão",
    }
}

/// Notação algébrica em português (R, D, T, B, C), com x, =D, + e #.
pub fn san(pos: &Pos, m: Move, promo: Option<Kind>) -> String {
    let Some(p) = pos.board[m.from] else { return String::new() };
    if p.kind == Kind::King && (m.from % 8).abs_diff(m.to % 8) == 2 {
        return if m.to % 8 == 6 { "O-O".into() } else { "O-O-O".into() };
    }
    let capture = pos.board[m.to].is_some() || (p.kind == Kind::Pawn && m.from % 8 != m.to % 8);
    let mut s = String::new();
    if p.kind == Kind::Pawn {
        if capture {
            s.push((b'a' + (m.from % 8) as u8) as char);
        }
    } else {
        s.push_str(letter(p.kind));
        // Desambiguação: outra peça igual que também chega na casa.
        let others: Vec<usize> = (0..64)
            .filter(|&i| i != m.from && pos.board[i] == Some(p) && pos.legal_from(i).iter().any(|x| x.to == m.to))
            .collect();
        if !others.is_empty() {
            if others.iter().all(|&i| i % 8 != m.from % 8) {
                s.push((b'a' + (m.from % 8) as u8) as char);
            } else {
                s.push((b'0' + (8 - m.from / 8) as u8) as char);
            }
        }
    }
    if capture {
        s.push('x');
    }
    s.push_str(&square_name(m.to));
    if p.kind == Kind::Pawn && (m.to / 8 == 0 || m.to / 8 == 7) {
        s.push('=');
        s.push_str(letter(promo.unwrap_or(Kind::Queen)));
    }
    let next = pos.apply_promo(m, promo.unwrap_or(Kind::Queen));
    if next.in_check(next.white_to_move) {
        s.push(if next.has_moves() { '+' } else { '#' });
    }
    s
}

// ---------- Desenho ----------

pub const SQ: i32 = 104;
pub const BX: i32 = (1072 - 8 * SQ) / 2;
pub const BY: i32 = 446;

/// Casa na tela; com `flip`, as pretas ficam embaixo.
pub fn square_rect(i: usize, flip: bool) -> Rect {
    let (r, c) = if flip { (7 - i / 8, 7 - i % 8) } else { (i / 8, i % 8) };
    Rect::new(BX + c as i32 * SQ, BY + r as i32 * SQ, SQ, SQ)
}
pub fn board_rect() -> Rect {
    Rect::new(BX, BY, 8 * SQ, 8 * SQ)
}
pub fn undo_rect() -> Rect {
    Rect::new(72, 1368, (1072 - 144 - 24) / 2, 72)
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
        self.draw_sized(fb, p, x, y, SQ - 8);
    }

    /// Peça desenhada (assets/sprites/xadrez-*.png) ou, sem o arquivo, o desenho geométrico.
    pub fn draw_sized(&self, fb: &mut Framebuffer, p: Piece, x: i32, y: i32, size: i32) {
        let name = match p.kind {
            Kind::King => "rei",
            Kind::Queen => "dama",
            Kind::Rook => "torre",
            Kind::Bishop => "bispo",
            Kind::Knight => "cavalo",
            Kind::Pawn => "peao",
        };
        if crate::sprites::draw(fb, &format!("xadrez-{name}-{}", if p.white { "b" } else { "p" }), Rect::new(x, y, size, size)) {
            return;
        }
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

/// Moldura do tabuleiro: borda escura arredondada, como um tabuleiro de verdade.
pub fn draw_board_frame(fb: &mut Framebuffer, b: Rect) {
    fb.round_rect(Rect::new(b.x - 10, b.y - 10, b.w + 20, b.h + 20), 18, 0x30);
}

/// Letras e números das casas, pequenos, no canto das casas da borda.
pub fn draw_coords(fb: &mut Framebuffer, f: &Fonts, flip: bool) {
    for k in 0..8 {
        // Coluna (embaixo) e fileira (à esquerda), na ordem de quem está embaixo.
        let file = if flip { 7 - k } else { k };
        let col_sq = 56 + file; // fileira 1 vista de baixo
        let r = square_rect(if flip { file } else { col_sq }, flip);
        let dark = ((if flip { file } else { col_sq }) / 8 + file) % 2 == 1;
        let t = ((b'a' + file as u8) as char).to_string();
        f.draw(fb, Face::Strong, 19.0, r.x + r.w - 17, r.y + r.h - 6, if dark { 0xEE } else { 0x88 }, &t);
        let rank_row = k; // linha da tela, de cima para baixo
        let sq = if flip { (7 - rank_row) * 8 + 7 } else { rank_row * 8 };
        let r = square_rect(sq, flip);
        let dark = (sq / 8 + sq % 8) % 2 == 1;
        let n = (8 - sq / 8).to_string();
        f.draw(fb, Face::Strong, 19.0, r.x + 5, r.y + 21, if dark { 0xEE } else { 0x88 }, &n);
    }
}

pub fn draw_square(fb: &mut Framebuffer, g: &Game, sp: &Sprites, i: usize) {
    let r = square_rect(i, g.flipped());
    let dark = (i / 8 + i % 8) % 2 == 1;
    let last = g.last.is_some_and(|m| m.from == i || m.to == i);
    fb.fill(r, if dark { 0xB0 } else { 0xF2 });
    if last {
        fb.fill(Rect::new(r.x + 4, r.y + 4, r.w - 8, r.h - 8), if dark { 0x90 } else { 0xD8 });
    }
    if g.selected == Some(i) {
        fb.round_outline(Rect::new(r.x + 3, r.y + 3, r.w - 6, r.h - 6), 10, 7, INK, if dark { 0xB0 } else { 0xF2 });
    }
    // Rei em xeque: listras e moldura grossa na casa dele.
    let checked = g.pos.board[i].is_some_and(|p| p.kind == Kind::King && p.white == g.pos.white_to_move) && g.pos.in_check(g.pos.white_to_move);
    if checked {
        let bg = if dark { 0xB0 } else { 0xF2 };
        fb.fill(r, bg);
        for yy in r.y..r.y + r.h {
            let mut xx = r.x + (12 - (r.x + yy).rem_euclid(12)) % 12;
            while xx < r.x + r.w {
                fb.fill(Rect::new(xx, yy, 4.min(r.x + r.w - xx), 1), 0x70);
                xx += 12;
            }
        }
        fb.round_outline(Rect::new(r.x + 2, r.y + 2, r.w - 4, r.h - 4), 6, 8, INK, bg);
        for yy in r.y + 10..r.y + r.h - 10 {
            let mut xx = r.x + 10 + (12 - (r.x + 10 + yy).rem_euclid(12)) % 12;
            while xx < r.x + r.w - 10 {
                fb.fill(Rect::new(xx, yy, 4.min(r.x + r.w - 10 - xx), 1), 0x70);
                xx += 12;
            }
        }
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

/// Botões de baixo da partida: rótulo e se está em destaque (confirmação); vazio = não mostra.
pub struct Buttons<'a> {
    pub left: (&'a str, bool),
    pub right: (&'a str, bool),
    /// Recado no lugar da linha de estado (empate proposto, fim por desistência…).
    pub note: Option<&'a str>,
}

pub fn draw(fb: &mut Framebuffer, f: &Fonts, g: &Game, sp: &Sprites, btn: &Buttons) {
    fb.fill(fb.screen(), PAPER);
    ui::draw_back(fb, f, false);
    if g.mode == Mode::Local {
        // Partida local: botão para girar a tela para quem joga.
        let r = rotate_rect();
        let label = if rotate_enabled() { "Girar: sim" } else { "Girar: não" };
        fb.round_outline(r, r.h / 2, 4, INK, PAPER);
        let w = f.measure(Face::Strong, 32.0, label);
        f.draw(fb, Face::Strong, 32.0, r.x + (r.w - w) / 2, r.y + 54, INK, label);
    } else {
        let tw = f.measure(Face::Strong, 34.0, "Xadrez");
        f.draw(fb, Face::Strong, 34.0, 1072 - 72 - tw, 128, INK, "Xadrez");
    }
    draw_status(fb, f, g, sp);
    if g.mode == Mode::Local {
        // Sem treinador: o nome de quem joga do lado de cima, grande.
        let top = if g.flipped() { "Brancas" } else { "Pretas" };
        f.draw(fb, Face::Display, 64.0, 72, 420, INK, top);
    }
    let b = board_rect();
    draw_board_frame(fb, b);
    for i in 0..64 {
        draw_square(fb, g, sp, i);
    }
    draw_coords(fb, f, g.flipped());
    // Xeque e xeque-mate escritos sobre o tabuleiro, além da casa marcada.
    let stm = g.pos.white_to_move;
    if g.pos.in_check(stm) {
        let mate = !g.pos.has_moves();
        let label = if mate { "Xeque-mate" } else { "Xeque" };
        let size = if mate { 64.0 } else { 40.0 };
        let w = f.measure(Face::Display, size, label);
        let (bw, bh) = (w + 80, if mate { 120 } else { 76 });
        let pill = if mate { Rect::new(b.x + (b.w - bw) / 2, b.y + (b.h - bh) / 2, bw, bh) } else { Rect::new(b.x + b.w - bw - 12, b.y - bh / 2, bw, bh) };
        fb.round_rect(Rect::new(pill.x - 6, pill.y - 6, pill.w + 12, pill.h + 12), (pill.h + 12) / 2, PAPER);
        fb.round_rect(pill, pill.h / 2, INK);
        f.draw(fb, Face::Display, size, pill.x + 40, pill.y + pill.h / 2 + (size * 0.36) as i32, PAPER, label);
    }
    if let Some(n) = btn.note {
        let r = status_rect();
        fb.fill(r, PAPER);
        f.draw(fb, Face::Display, 40.0, 72, r.y + 50, INK, &crate::apps::fit(f, Face::Display, 40.0, n, 1072 - 144));
    }
    for (r, (label, filled)) in [(undo_rect(), btn.left), (new_rect(), btn.right)] {
        if label.is_empty() {
            continue;
        }
        if filled {
            fb.round_rect(r, r.h / 2, INK);
        } else {
            fb.round_outline(r, r.h / 2, 4, INK, PAPER);
        }
        let size = if f.measure(Face::Strong, 38.0, label) > r.w - 40 { 30.0 } else { 38.0 };
        let w = f.measure(Face::Strong, size, label);
        f.draw(fb, Face::Strong, size, r.x + (r.w - w) / 2, r.y + r.h / 2 + (size * 0.36) as i32, if filled { PAPER } else { INK }, label);
    }
}

pub fn rotate_rect() -> Rect {
    Rect::new(1072 - 72 - 270, 76, 270, 84)
}

/// Girar a tela na vez das pretas (jogo local, Kindle deitado entre os dois).
pub fn rotate_enabled() -> bool {
    crate::home_dir().join("settings/xadrez_girar").exists()
}

pub fn set_rotate(on: bool) {
    let p = crate::home_dir().join("settings/xadrez_girar");
    if on {
        let _ = std::fs::write(p, "1");
    } else {
        let _ = std::fs::remove_file(p);
    }
}

/// Relógios: o de cima é de quem joga do lado de cima do tabuleiro.
pub fn top_clock_rect() -> Rect {
    Rect::new(1072 - 72 - 210, 368, 210, 64)
}
pub fn bottom_clock_rect() -> Rect {
    Rect::new(1072 - 72 - 210, 1292, 210, 64)
}

/// Desenha os dois relógios. O que está correndo fica preto; com menos de 10 s ganha borda grossa.
pub fn draw_clocks(fb: &mut Framebuffer, f: &Fonts, g: &Game) {
    if g.clock.is_none() {
        return;
    }
    let bottom_white = !g.flipped();
    for (r, white) in [(top_clock_rect(), !bottom_white), (bottom_clock_rect(), bottom_white)] {
        let ms = g.clock_left(white).unwrap_or(0);
        let active = g.clock_running() && g.pos.white_to_move == white;
        fb.fill(Rect::new(r.x - 6, r.y - 6, r.w + 12, r.h + 12), PAPER);
        if active {
            fb.round_rect(r, r.h / 2, INK);
        } else {
            fb.round_outline(r, r.h / 2, if ms < 10_000 { 7 } else { 3 }, INK, PAPER);
        }
        let t = clock_text(ms);
        let w = f.measure(Face::Display, 42.0, &t);
        f.draw(fb, Face::Display, 42.0, r.x + (r.w - w) / 2, r.y + 47, if active { PAPER } else { INK }, &t);
    }
}

pub fn status_rect() -> Rect {
    Rect::new(0, 1288, 1072, 72)
}

pub fn promo_rect(k: usize) -> Rect {
    let w = (1072 - 144 - 3 * 16) / 4;
    Rect::new(72 + k as i32 * (w + 16), 1286, w, 76)
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
            sp.draw_sized(fb, Piece { kind: *kind, white }, b.x + (b.w - 70) / 2, b.y + 3, 70);
        }
        return;
    }
    let s = g.status();
    let s = crate::apps::fit(f, Face::Display, 46.0, &s, 1072 - 144 - 240);
    f.draw(fb, Face::Display, 44.0, 72, r.y + 50, INK, &s);
    if g.clock.is_some() {
        draw_clocks(fb, f, g);
        return;
    }
    let n = g.history.len();
    let mat = g.pos.material();
    let side = match mat {
        0 => String::new(),
        m if m > 0 => format!("brancas +{m} · "),
        m => format!("pretas +{} · ", -m),
    };
    let moves = if n == 0 { "início".to_string() } else { format!("{side}{n} {}", if n == 1 { "lance" } else { "lances" }) };
    let w = f.measure(Face::Body, 28.0, &moves);
    f.draw(fb, Face::Body, 28.0, 1072 - 72 - w, r.y + 48, MUTED, &moves);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relogio_desconta_soma_acrescimo_e_cai_a_bandeira() {
        let mut g = Game { clock: Some(Clock::new(3)), ..Game::new() }; // 3+2
        assert_eq!(g.clock_left(true), Some(180_000));
        assert!(g.play_uci("e2e4").is_some());
        assert!(!g.clock_running(), "só corre depois do primeiro lance de cada lado");
        assert_eq!(g.clock_left(true), Some(182_000));
        assert!(g.play_uci("e7e5").is_some());
        assert!(g.clock_running());
        // Brancas sem tempo: a bandeira cai e as pretas vencem (têm material).
        g.clock.as_mut().unwrap().left[0] = 0;
        assert!(g.clock_tick());
        assert!(g.result().unwrap().contains("Vitória das pretas"));
        assert_eq!(clock_text(65_000), "1:05");
        assert_eq!(clock_text(9_450), "9.4");
    }

    #[test]
    fn fen_e_lances_legais() {
        let p = Pos::start();
        assert_eq!(p.fen(1), "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
        assert_eq!(p.legal_uci().len(), 20);
        assert!(p.legal_uci().contains(&"e2e4".to_string()));
    }
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

#[cfg(test)]
mod render {
    use super::*;
    #[test]
    #[ignore]
    fn xeque_em_imagem() {
        std::env::set_var("VERSO_INK_HOME", std::env::temp_dir().join("verso-ink-xadrez"));
        let out = std::path::PathBuf::from(std::env::var("VERSO_INK_SHOTS").expect("VERSO_INK_SHOTS"));
        let f = Fonts::new();
        let sp = Sprites::new();
        let btn = Buttons { left: ("Empate", false), right: ("Desistir", false), note: None };
        for (name, moves) in [("xeque", &["e2e4", "f7f6", "d1h5"][..]), ("mate", &["f2f3", "e7e5", "g2g4", "d8h4"][..])] {
            let mut g = Game::new();
            for m in moves {
                g.play_uci(m);
            }
            let mut fb = crate::fb::Framebuffer::memory();
            draw(&mut fb, &f, &g, &sp, &btn);
            fb.save_pgm(&out.join(format!("{name}.pgm")));
        }
    }
}
