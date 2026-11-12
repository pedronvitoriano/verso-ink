//! Damas (regras brasileiras): tabuleiro 8×8, captura obrigatória e pela maioria (quem pode
//! capturar mais peças tem de fazê-lo), peão captura para trás, dama anda e captura em diagonais
//! longas. Contra a CPU (fases) ou duas pessoas.

use crate::fases;
use crate::fb::{Framebuffer, Rect};
use crate::minigame::{self, MiniGame, Redraw, Rng, W};
use crate::text::{Face, Fonts};
use crate::ui::{self, INK, MUTED, PAPER};

/// 0 vazio; 1 peão claro (você/jogador 1, embaixo), 2 dama clara; 3 peão escuro, 4 dama escura.
type Board = [u8; 64];

fn is_light(p: u8) -> bool {
    p == 1 || p == 2
}
fn is_king(p: u8) -> bool {
    p == 2 || p == 4
}
fn owner(p: u8) -> u8 {
    match p {
        1 | 2 => 1,
        3 | 4 => 2,
        _ => 0,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Move {
    /// Casas visitadas: origem, (escalas), destino.
    pub path: Vec<usize>,
    pub captured: Vec<usize>,
}

const DIRS: [(i32, i32); 4] = [(-1, -1), (-1, 1), (1, -1), (1, 1)];

fn at(r: i32, c: i32) -> Option<usize> {
    ((0..8).contains(&r) && (0..8).contains(&c)).then(|| (r * 8 + c) as usize)
}

fn start_board() -> Board {
    let mut b = [0u8; 64];
    for i in 0..64 {
        let (r, c) = (i / 8, i % 8);
        if (r + c) % 2 == 1 {
            if r < 3 {
                b[i] = 3;
            } else if r > 4 {
                b[i] = 1;
            }
        }
    }
    b
}

/// Sequências de captura a partir de `from` (busca em profundidade; peças capturadas saem no fim).
fn captures_from(b: &Board, from: usize, piece: u8, path: &mut Vec<usize>, taken: &mut Vec<usize>, out: &mut Vec<Move>) {
    let me = owner(piece);
    let (r, c) = ((from / 8) as i32, (from % 8) as i32);
    let mut extended = false;
    for (dr, dc) in DIRS {
        if is_king(piece) {
            // Dama: anda livre até achar uma peça inimiga, pula e pode parar em qualquer casa livre depois.
            let mut k = 1;
            while let Some(i) = at(r + dr * k, c + dc * k) {
                if b[i] == 0 {
                    k += 1;
                    continue;
                }
                break;
            }
            let Some(enemy) = at(r + dr * k, c + dc * k) else { continue };
            if owner(b[enemy]) == me || owner(b[enemy]) == 0 || taken.contains(&enemy) {
                continue;
            }
            let mut j = k + 1;
            while let Some(land) = at(r + dr * j, c + dc * j) {
                if b[land] != 0 && land != path[0] {
                    break;
                }
                extended = true;
                path.push(land);
                taken.push(enemy);
                captures_from(b, land, piece, path, taken, out);
                taken.pop();
                path.pop();
                j += 1;
            }
        } else {
            let (Some(enemy), Some(land)) = (at(r + dr, c + dc), at(r + 2 * dr, c + 2 * dc)) else { continue };
            if owner(b[enemy]) == 0 || owner(b[enemy]) == me || taken.contains(&enemy) || (b[land] != 0 && land != path[0]) {
                continue;
            }
            extended = true;
            path.push(land);
            taken.push(enemy);
            captures_from(b, land, piece, path, taken, out);
            taken.pop();
            path.pop();
        }
    }
    if !extended && !taken.is_empty() {
        out.push(Move { path: path.clone(), captured: taken.clone() });
    }
}

/// Todos os lances legais de `side` (1 claro, 2 escuro), já aplicada a lei da maioria.
pub fn legal_moves(b: &Board, side: u8) -> Vec<Move> {
    let mut caps = Vec::new();
    for from in 0..64 {
        if owner(b[from]) != side {
            continue;
        }
        let mut board = *b;
        let piece = board[from];
        board[from] = 0;
        let mut path = vec![from];
        captures_from(&board, from, piece, &mut path, &mut Vec::new(), &mut caps);
    }
    if !caps.is_empty() {
        let best = caps.iter().map(|m| m.captured.len()).max().unwrap_or(0);
        caps.retain(|m| m.captured.len() == best);
        return caps;
    }
    let mut v = Vec::new();
    for from in 0..64 {
        let p = b[from];
        if owner(p) != side {
            continue;
        }
        let (r, c) = ((from / 8) as i32, (from % 8) as i32);
        for (dr, dc) in DIRS {
            if is_king(p) {
                let mut k = 1;
                while let Some(i) = at(r + dr * k, c + dc * k) {
                    if b[i] != 0 {
                        break;
                    }
                    v.push(Move { path: vec![from, i], captured: vec![] });
                    k += 1;
                }
            } else {
                // Peão só anda para a frente (claro sobe, escuro desce).
                let forward = if is_light(p) { -1 } else { 1 };
                if dr != forward {
                    continue;
                }
                if let Some(i) = at(r + dr, c + dc) {
                    if b[i] == 0 {
                        v.push(Move { path: vec![from, i], captured: vec![] });
                    }
                }
            }
        }
    }
    v
}

pub fn apply(b: &mut Board, m: &Move) {
    let from = m.path[0];
    let to = *m.path.last().unwrap();
    let p = b[from];
    b[from] = 0;
    for &c in &m.captured {
        b[c] = 0;
    }
    // Promove ao terminar na última linha.
    b[to] = match p {
        1 if to / 8 == 0 => 2,
        3 if to / 8 == 7 => 4,
        x => x,
    };
}

fn eval(b: &Board, side: u8) -> i32 {
    let mut s = 0;
    for (i, &p) in b.iter().enumerate() {
        let v = match p {
            1 => 100 + (7 - (i / 8) as i32) * 3,
            3 => 100 + (i / 8) as i32 * 3,
            2 | 4 => 320,
            _ => 0,
        };
        if owner(p) == side {
            s += v;
        } else if p != 0 {
            s -= v;
        }
    }
    s
}

fn negamax(b: &Board, side: u8, depth: u32, mut alpha: i32, beta: i32) -> i32 {
    let moves = legal_moves(b, side);
    if moves.is_empty() {
        return -100_000 - depth as i32;
    }
    if depth == 0 {
        return eval(b, side);
    }
    let mut best = i32::MIN + 1;
    for m in &moves {
        let mut nb = *b;
        apply(&mut nb, m);
        let v = -negamax(&nb, 3 - side, depth - 1, -beta, -alpha);
        best = best.max(v);
        alpha = alpha.max(v);
        if alpha >= beta {
            break;
        }
    }
    best
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Menu,
    Cpu,
    Local,
}

pub struct Damas {
    mode: Mode,
    board: Board,
    turn: u8,
    selected: Option<usize>,
    last: Option<Move>,
    winner: Option<u8>,
    /// Lances sem captura e sem peão andar (empate por repetição de damas).
    quiet: u32,
    rng: Rng,
    msg: String,
    fase: u32,
    practice: bool,
    level: u32,
    prog: fases::Progress,
}

const MENU_TREINO: [&str; 3] = ["Contra a CPU · Fácil", "Contra a CPU · Médio", "Contra a CPU · Difícil"];

impl Damas {
    pub fn new() -> Self {
        Damas {
            mode: Mode::Menu,
            board: start_board(),
            turn: 1,
            selected: None,
            last: None,
            winner: None,
            quiet: 0,
            rng: Rng::new(),
            msg: String::new(),
            fase: 0,
            practice: false,
            level: 1,
            prog: fases::Progress::load("damas"),
        }
    }

    fn reset(&mut self, mode: Mode, fase: u32, level: u32) {
        self.mode = mode;
        self.board = start_board();
        self.turn = 1;
        self.selected = None;
        self.last = None;
        self.winner = None;
        self.quiet = 0;
        self.practice = false;
        self.fase = fase;
        self.level = level;
        self.msg = "Toque numa peça sua e depois no destino. Capturar é obrigatório.".into();
    }

    /// Profundidade da CPU e chance de errar de propósito (%).
    fn strength(&self) -> (u32, u64) {
        if self.fase == 0 {
            return [(2, 30), (4, 5), (6, 0)][self.level.min(2) as usize];
        }
        let k = self.fase;
        match fases::kind(k) {
            fases::Kind::Normal => ((1 + k / 2).min(6), 35u64.saturating_sub(4 * k as u64)),
            fases::Kind::Hard => ((3 + k / 3).min(8), 0),
            fases::Kind::Challenge => (10, 0),
        }
    }

    fn geom() -> (i32, i32, i32) {
        minigame::grid_geom(8, 8, 300, 900)
    }

    fn cell(i: usize) -> Rect {
        let (x0, y0, s) = Self::geom();
        Rect::new(x0 + (i % 8) as i32 * s, y0 + (i / 8) as i32 * s, s, s)
    }

    fn moves(&self) -> Vec<Move> {
        legal_moves(&self.board, self.turn)
    }

    fn do_move(&mut self, m: Move) {
        let man = !is_king(self.board[m.path[0]]);
        apply(&mut self.board, &m);
        self.quiet = if m.captured.is_empty() && !man { self.quiet + 1 } else { 0 };
        self.last = Some(m);
        self.selected = None;
        self.turn = 3 - self.turn;
        if legal_moves(&self.board, self.turn).is_empty() {
            self.winner = Some(3 - self.turn);
            if self.fase > 0 && self.winner == Some(1) {
                self.prog.record_win(self.fase, fases::today());
            }
        } else if self.quiet >= 40 {
            self.winner = Some(0);
        }
    }

    fn name(&self, side: u8) -> &'static str {
        match (self.mode, side) {
            (Mode::Cpu, 1) => "Você",
            (Mode::Cpu, _) => "CPU",
            (_, 1) => "Claras",
            _ => "Escuras",
        }
    }

    fn status(&self) -> String {
        match self.winner {
            Some(0) => "Empate: 20 lances de damas sem captura.".into(),
            Some(w) => format!("{} {}!", self.name(w), if self.name(w) == "Você" { "venceu" } else { "vence" }),
            None => {
                let caps = self.moves().first().map_or(0, |m| m.captured.len());
                let must = if caps > 0 { format!(" · captura obrigatória ({caps})") } else { String::new() };
                format!("Vez: {}{must}", self.name(self.turn))
            }
        }
    }

    fn draw_piece(fb: &mut Framebuffer, r: Rect, p: u8) {
        // Peça desenhada (assets/sprites/peca-damas-*.png); sem o arquivo, o desenho geométrico.
        let name = match (is_light(p), is_king(p)) {
            (true, false) => "peca-damas-branca",
            (false, false) => "peca-damas-preta",
            (true, true) => "peca-damas-dama-branca",
            (false, true) => "peca-damas-dama-preta",
        };
        let m = r.w / 12;
        if crate::sprites::draw(fb, name, Rect::new(r.x + m, r.y + m, r.w - 2 * m, r.h - 2 * m)) {
            return;
        }
        let d = r.w * 3 / 4;
        let c = Rect::new(r.x + (r.w - d) / 2, r.y + (r.h - d) / 2, d, d);
        // Sombra de peça (espessura).
        fb.round_rect(Rect::new(c.x, c.y + 6, d, d), d / 2, 0x33);
        if is_light(p) {
            fb.round_outline(c, d / 2, 5, INK, PAPER);
            fb.round_outline(Rect::new(c.x + d / 5, c.y + d / 5, d * 3 / 5, d * 3 / 5), d * 3 / 10, 3, 0x99, PAPER);
        } else {
            fb.round_rect(c, d / 2, INK);
            fb.round_outline(Rect::new(c.x + d / 5, c.y + d / 5, d * 3 / 5, d * 3 / 5), d * 3 / 10, 3, 0x55, INK);
        }
        if is_king(p) {
            // Coroa no centro da dama.
            let ink = if is_light(p) { INK } else { PAPER };
            let (cx, cy, s) = (c.x as f32 + d as f32 / 2.0, c.y as f32 + d as f32 / 2.0, d as f32 * 0.42);
            let pts = [(-0.5, 0.35), (-0.5, -0.2), (-0.25, 0.08), (0.0, -0.4), (0.25, 0.08), (0.5, -0.2), (0.5, 0.35)];
            for w in pts.windows(2) {
                ui::stroke(fb, (cx + w[0].0 * s, cy + w[0].1 * s), (cx + w[1].0 * s, cy + w[1].1 * s), 5.0, ink);
            }
            ui::stroke(fb, (cx - 0.5 * s, cy + 0.35 * s), (cx + 0.5 * s, cy + 0.35 * s), 6.0, ink);
        }
    }
}

impl MiniGame for Damas {
    fn risky(&self, id: u32) -> bool {
        id == 803 && self.fase == 0 && self.winner.is_none()
    }

    fn at_menu(&self) -> bool {
        self.mode == Mode::Menu
    }

    fn summary(&self) -> String {
        if self.mode == Mode::Menu {
            return String::new();
        }
        let count = |s: u8| self.board.iter().filter(|&&p| owner(p) == s).count();
        format!("{} × {} peças · {}", count(1), count(2), self.status())
    }

    fn context(&self) -> String {
        if self.mode == Mode::Menu {
            return "No menu das Damas.".into();
        }
        let rows: Vec<String> = (0..8).map(|r| (0..8).map(|c| match self.board[r * 8 + c] { 1 => 'o', 2 => 'O', 3 => 'x', 4 => 'X', _ => '.' }).collect()).collect();
        format!(
            "Damas, regras brasileiras (captura obrigatória pela maioria, peão captura para trás, dama de longo alcance). o = peão do Pedro (sobe), O = dama dele; x/X = adversário. Linha de cima é a 8.\n{}\n{}",
            rows.join("\n"),
            self.status()
        )
    }

    fn draw(&self, fb: &mut Framebuffer, f: &Fonts) {
        fb.fill(fb.screen(), PAPER);
        ui::draw_back(fb, f, false);
        if self.mode == Mode::Menu && self.practice {
            minigame::draw_menu(fb, f, "Treino", "Escolha a força da CPU. Não conta para as fases.", &MENU_TREINO);
            return;
        }
        if self.mode == Mode::Menu {
            let main = format!("Fase {} · contra a CPU", self.prog.level);
            fases::draw_menu(fb, f, "Damas", "Regras brasileiras: capturar é obrigatório, pela maioria.", &self.prog, &main, &["Treino contra a CPU", "Duas pessoas"]);
            return;
        }
        let title = if self.fase > 0 { fases::title(self.fase) } else { "Damas".to_string() };
        minigame::header(fb, f, &title, &self.status());
        let (x0, y0, s) = Self::geom();
        fb.round_outline(Rect::new(x0 - 8, y0 - 8, s * 8 + 16, s * 8 + 16), 10, 5, INK, PAPER);
        let moves = if self.winner.is_none() { self.moves() } else { vec![] };
        let targets: Vec<usize> = self.selected.map(|sel| moves.iter().filter(|m| m.path[0] == sel).map(|m| *m.path.last().unwrap()).collect()).unwrap_or_default();
        let movable: Vec<usize> = moves.iter().map(|m| m.path[0]).collect();
        for i in 0..64 {
            let r = Self::cell(i);
            let dark = (i / 8 + i % 8) % 2 == 1;
            fb.fill(r, if dark { 0xA8 } else { 0xF2 });
            if self.last.as_ref().is_some_and(|m| m.path.contains(&i)) {
                fb.fill(Rect::new(r.x + 5, r.y + 5, r.w - 10, r.h - 10), 0x88);
            }
            if self.selected == Some(i) {
                fb.round_outline(Rect::new(r.x + 3, r.y + 3, r.w - 6, r.h - 6), 10, 7, INK, 0xA8);
            }
            let p = self.board[i];
            if p != 0 {
                Self::draw_piece(fb, r, p);
                // Peças que podem jogar (ajuda quando a captura é obrigatória).
                let my_turn = self.mode == Mode::Local || self.turn == 1;
                if my_turn && self.selected.is_none() && movable.contains(&i) && moves.first().is_some_and(|m| !m.captured.is_empty()) {
                    fb.round_outline(Rect::new(r.x + 2, r.y + 2, r.w - 4, r.h - 4), 8, 4, INK, 0xA8);
                    Self::draw_piece(fb, r, p);
                }
            }
            if targets.contains(&i) {
                let (cx, cy) = (r.x + r.w / 2, r.y + r.h / 2);
                fb.round_rect(Rect::new(cx - 16, cy - 16, 32, 32), 16, INK);
            }
        }
        if self.fase > 0 && self.winner.is_some() {
            if self.winner == Some(1) {
                return fases::draw_win(fb, f, &self.prog);
            }
            return fases::draw_retry(fb, f, self.fase);
        }
        minigame::hint(fb, f, &crate::apps::fit(f, Face::Body, 26.0, &self.msg, W - 144));
        let last = if self.fase > 0 { "Menu" } else { "Novo jogo" };
        minigame::pill(fb, f, minigame::bottom_button(0, 1), last, false);
        let _ = MUTED;
    }

    fn targets(&self) -> Vec<(u32, Rect)> {
        if self.mode == Mode::Menu && self.practice {
            return (0..3).map(|k| (910 + k as u32, minigame::menu_row(k))).collect();
        }
        if self.mode == Mode::Menu {
            return (0..3).map(|k| (900 + k as u32, minigame::menu_row(k))).collect();
        }
        if self.fase > 0 && self.winner.is_some() {
            return fases::win_targets();
        }
        let mut v = vec![(803u32, minigame::bottom_button(0, 1))];
        let my_turn = self.mode == Mode::Local || self.turn == 1;
        if my_turn && self.winner.is_none() {
            v.extend((0..64).filter(|i| (i / 8 + i % 8) % 2 == 1).map(|i| (i as u32, Self::cell(i))));
        }
        v
    }

    fn tap(&mut self, id: u32, _x: i32, _y: i32) -> Redraw {
        match id {
            900 | fases::WIN_NEXT => {
                let k = self.prog.level;
                self.reset(Mode::Cpu, k, 1);
                Redraw::Full
            }
            901 => {
                self.practice = true;
                Redraw::Full
            }
            902 => {
                self.reset(Mode::Local, 0, 0);
                Redraw::Full
            }
            910..=912 => {
                self.reset(Mode::Cpu, 0, id - 910);
                Redraw::Full
            }
            803 | fases::WIN_MENU => {
                self.mode = Mode::Menu;
                Redraw::Full
            }
            i if (i as usize) < 64 => {
                let i = i as usize;
                let moves = self.moves();
                if owner(self.board[i]) == self.turn {
                    if moves.iter().any(|m| m.path[0] == i) {
                        self.selected = Some(i);
                        self.msg.clear();
                    } else if moves.first().is_some_and(|m| !m.captured.is_empty()) {
                        self.msg = "Capturar é obrigatório: jogue com uma das peças marcadas.".into();
                    } else {
                        self.msg = "Essa peça não tem lance.".into();
                    }
                    return Redraw::Part(Rect::new(0, 170, W, 1448 - 170));
                }
                if let Some(sel) = self.selected {
                    if let Some(m) = moves.into_iter().find(|m| m.path[0] == sel && *m.path.last().unwrap() == i) {
                        let n = m.captured.len();
                        self.do_move(m);
                        self.msg = if n > 0 { format!("Capturou {n}.") } else { String::new() };
                        return Redraw::Part(Rect::new(0, 170, W, 1448 - 170));
                    }
                }
                Redraw::None
            }
            _ => Redraw::None,
        }
    }

    fn cpu_turn(&self) -> bool {
        self.mode == Mode::Cpu && self.turn == 2 && self.winner.is_none()
    }

    fn cpu_move(&mut self) {
        let moves = self.moves();
        if moves.is_empty() {
            return;
        }
        let (depth, random) = self.strength();
        let pick = if random > 0 && self.rng.next() % 100 < random {
            (self.rng.next() % moves.len() as u64) as usize
        } else {
            let mut best = (i32::MIN, 0);
            for (k, m) in moves.iter().enumerate() {
                let mut nb = self.board;
                apply(&mut nb, m);
                let v = -negamax(&nb, 1, depth.saturating_sub(1), -1_000_000, 1_000_000) + (self.rng.next() % 3) as i32;
                if v > best.0 {
                    best = (v, k);
                }
            }
            best.1
        };
        let m = moves[pick].clone();
        let n = m.captured.len();
        self.do_move(m);
        self.msg = if n > 0 { format!("A CPU capturou {n}.") } else { String::new() };
    }

    fn back(&mut self) -> bool {
        if self.mode == Mode::Menu && self.practice {
            self.practice = false;
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abertura_tem_sete_lances() {
        assert_eq!(legal_moves(&start_board(), 1).len(), 7);
    }

    #[test]
    fn captura_obrigatoria_e_pela_maioria() {
        let mut b = [0u8; 64];
        // Peão claro em f4 (r4,c5) pode capturar 1 à esquerda ou 2 em sequência à direita.
        b[4 * 8 + 5] = 1;
        b[3 * 8 + 4] = 3; // captura simples
        b[3 * 8 + 6] = 3; // primeira da dupla
        b[1 * 8 + 6] = 3; // segunda da dupla (depois de cair em r2,c7)
        b[7 * 8] = 1;
        let m = legal_moves(&b, 1);
        assert!(m.iter().all(|x| x.captured.len() == 2), "{m:?}");
    }

    #[test]
    fn peao_captura_para_tras_e_dama_voa() {
        let mut b = [0u8; 64];
        b[3 * 8 + 2] = 1;
        b[4 * 8 + 3] = 3; // atrás do peão claro
        assert!(legal_moves(&b, 1).iter().any(|m| m.captured == vec![4 * 8 + 3]));
        let mut b = [0u8; 64];
        b[7 * 8] = 2; // dama em a1
        b[3 * 8 + 4] = 3; // inimiga longe na diagonal
        let m = legal_moves(&b, 1);
        assert!(m.iter().all(|x| x.captured == vec![3 * 8 + 4]));
        assert!(m.len() >= 2, "pode parar em mais de uma casa depois");
    }

    #[test]
    fn promove_na_ultima_linha() {
        let mut b = [0u8; 64];
        b[1 * 8 + 2] = 1;
        let m = legal_moves(&b, 1).into_iter().next().unwrap();
        apply(&mut b, &m);
        assert!(b.contains(&2));
    }
}

#[cfg(test)]
mod tempo {
    use super::*;
    #[test]
    #[ignore]
    fn tempo_da_cpu() {
        for depth in [4u32, 6, 7, 8] {
            let b = start_board();
            let t = std::time::Instant::now();
            let _ = negamax(&b, 2, depth, -1_000_000, 1_000_000);
            eprintln!("profundidade {depth}: {:?}", t.elapsed());
        }
    }
}

#[cfg(test)]
mod render {
    use super::*;
    #[test]
    #[ignore]
    fn damas_em_imagem() {
        let out = std::path::PathBuf::from(std::env::var("VERSO_INK_SHOTS").expect("VERSO_INK_SHOTS"));
        let f = Fonts::new();
        let mut g = Damas { prog: fases::Progress::load_from(std::env::temp_dir().join("damas-r.txt")), ..Damas::new() };
        g.reset(Mode::Cpu, 3, 1);
        for _ in 0..3 {
            let m = g.moves()[0].clone();
            g.do_move(m);
            g.cpu_move();
        }
        g.board[2 * 8 + 1] = 2;
        g.selected = g.moves().first().map(|m| m.path[0]);
        let mut fb = crate::fb::Framebuffer::memory();
        g.draw(&mut fb, &f);
        fb.save_pgm(&out.join("damas.pgm"));
    }
}
